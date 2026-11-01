//! Issue #957: an opt-in closing notice for the last three model calls of a
//! generic direct run, and a guard that keeps unresolved failures from being
//! reported as a normal final after that notice.
//!
//! When `COMMANDAGENT_BUDGET_CLOSING=once` is set, a generic MinimalLoop run
//! without a step kind, recovery obligation, phase scope, or completion
//! contract appends one fixed notice ([`NOTICE`], prompt version 1) to the
//! request that is actually sent when the remaining calls, counting the
//! current one, reach [`CLOSING_REMAINING_CALLS`]. Existing feedback stays in
//! front of it; a fresh-session retry carries the notice alone and still
//! sends no earlier history. Tools, permissions, and every iteration, time,
//! and retry limit stay as they are.
//!
//! From run start the run also keeps a ledger of unresolved failures: a
//! non-substituted Bash call that the existing execution-progress
//! classification counts as not successful, and a Write/Edit call that ended
//! in a recoverable error or rejection. A Bash failure is resolved only by a
//! later success of the same command; a Write/Edit failure only by a later
//! success that mechanically matches the failed operation. Anything that
//! cannot be identified stays unresolved. After the notice, every normal
//! final that already passed every existing check is deferred while one
//! entry remains, however many finals came before; the budget then ends the
//! run through the existing exhaustion path. Before the notice, finals are
//! judged by the existing checks alone.
//!
//! Unset, empty, or `off` disables everything: no state, no event, and no
//! request change. Any other value is a configuration error that names the
//! variable but never the value. The ledger and the event carry counts and
//! fixed words only; no command, path, content, model text, tool output, or
//! secret leaves the run's memory.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::eval_events;
use crate::planner::profile_descriptor::GENERIC_PROFILE_ID;
use crate::state::ConversationMessage;
use crate::tools::path_guard::normalize_workspace_path;

use super::execution_progress::ExecutionProgress;
use super::loop_run::{RunSessionOptions, RunSessionScope};

/// The opt-in environment variable. It has no `ANVIL_*` legacy name.
pub(crate) const ENV_NAME: &str = "COMMANDAGENT_BUDGET_CLOSING";
/// Version of [`NOTICE`]; recorded in every event.
pub(crate) const PROMPT_VERSION: u32 = 1;
/// The notice goes into the request sent when this many calls remain,
/// counting the current one.
pub(crate) const CLOSING_REMAINING_CALLS: usize = 3;
/// Event name.
pub(crate) const EVENT: &str = "budget_closing";

/// The fixed closing notice (prompt version 1, Issue #957).
pub(crate) const NOTICE: &str = "残りの呼出しは現在の回を含めて3回です。新しい改善や作り直しは始めないでください。必要な変更が残っていれば最小限で行い、確認を1回実行してください。確認が成功したら tool を呼ばずに、変えた file・実行した確認・その結果を報告して終えてください。確認が失敗した、または実行できなかったときは成功と書かず、残っている問題を報告してください。";

/// The fixed feedback that replaces a deferred final.
pub(crate) const DEFERRAL_FEEDBACK: &str = "失敗した確認または書込みが残っています。直して同じ確認を通すか、未完了として報告してください。";

/// The blocking reason recorded when a final is deferred.
pub(crate) const DEFERRAL_BLOCKING_REASON: &str = "unresolved failure after budget closing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClosingMode {
    Off,
    Once,
}

/// Parses the raw environment value. The error never contains the value.
pub(crate) fn parse_mode(raw: Option<&OsStr>) -> anyhow::Result<ClosingMode> {
    let Some(raw) = raw else {
        return Ok(ClosingMode::Off);
    };
    match raw.to_str().map(str::trim) {
        Some("" | "off") => Ok(ClosingMode::Off),
        Some("once") => Ok(ClosingMode::Once),
        _ => anyhow::bail!("invalid {ENV_NAME}: expected unset, empty, `off`, or `once`"),
    }
}

#[cfg(not(test))]
fn read_env() -> Option<OsString> {
    std::env::var_os(ENV_NAME)
}

#[cfg(test)]
thread_local! {
    /// Unit tests set the mode per thread so a parallel test never sees it.
    static TEST_ENV: std::cell::RefCell<Option<OsString>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn read_env() -> Option<OsString> {
    TEST_ENV.with(|value| value.borrow().clone())
}

/// Whether the run is a generic direct run without a completion contract.
pub(crate) fn in_scope(config: &Config, options: &RunSessionOptions, contract: bool) -> bool {
    options.scope == RunSessionScope::MinimalLoop
        && options.step_kind.is_none()
        && options.phase_scope.is_none()
        && options.recovery_obligation.is_none()
        && config.profile == GENERIC_PROFILE_ID
        && !contract
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Disabled,
    Armed,
    Noticed,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventPhase {
    Notice,
    FinalAccepted,
    FinalDeferred,
    ExhaustedAfterNotice,
}

impl EventPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Notice => "notice",
            Self::FinalAccepted => "final_accepted",
            Self::FinalDeferred => "final_deferred",
            Self::ExhaustedAfterNotice => "exhausted_after_notice",
        }
    }

    fn guard_reason(self) -> &'static str {
        match self {
            Self::Notice => "remaining_calls_reached",
            Self::FinalAccepted => "no_unresolved_failure",
            Self::FinalDeferred => "unresolved_failure",
            Self::ExhaustedAfterNotice => "iteration_budget_exhausted",
        }
    }
}

/// A failed Edit, identified by everything that made the operation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EditOperation {
    path: String,
    old: String,
    new: String,
    replace_all: bool,
}

/// A failed Write, identified by its target and a digest of its content.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WriteOperation {
    path: String,
    digest: [u8; 32],
}

/// Unresolved failures of the run. Entries are bounded by the run's tool
/// calls, which the existing iteration limit bounds.
#[derive(Debug, Default)]
struct Ledger {
    bash: BTreeSet<String>,
    unidentified_bash: usize,
    edits: Vec<EditOperation>,
    writes: Vec<WriteOperation>,
    unidentified_writes: usize,
    resolved: usize,
    rejected_bash: usize,
}

impl Ledger {
    fn unresolved_bash(&self) -> usize {
        self.bash.len() + self.unidentified_bash
    }

    fn unresolved_writes(&self) -> usize {
        self.edits.len() + self.writes.len() + self.unidentified_writes
    }

    fn unresolved(&self) -> usize {
        self.unresolved_bash() + self.unresolved_writes()
    }
}

/// Per-run closing state.
#[derive(Debug)]
pub(crate) struct BudgetClosing {
    phase: Phase,
    events_path: Option<PathBuf>,
    root: PathBuf,
    ledger: Ledger,
    /// `(iteration, remaining_calls, iteration_limit)` of a composed notice
    /// that is not yet known to be in a sent request.
    staged: Option<(usize, usize, usize)>,
    iteration_limit: usize,
    deferrals: usize,
}

impl BudgetClosing {
    /// Reads the environment once, at run start, and builds the run's state.
    pub(crate) fn start(
        config: &Config,
        options: &RunSessionOptions,
        completion_contract: bool,
    ) -> anyhow::Result<Self> {
        let mode = parse_mode(read_env().as_deref())?;
        let enabled = mode == ClosingMode::Once && in_scope(config, options, completion_contract);
        Ok(Self::new(
            enabled,
            config.eval_events_path.as_deref(),
            &config.workspace_root,
        ))
    }

    fn new(enabled: bool, events_path: Option<&Path>, root: &Path) -> Self {
        Self {
            phase: if enabled {
                Phase::Armed
            } else {
                Phase::Disabled
            },
            events_path: events_path.map(Path::to_path_buf),
            root: root.to_path_buf(),
            ledger: Ledger::default(),
            staged: None,
            iteration_limit: 0,
            deferrals: 0,
        }
    }

    /// The feedback for the request about to be built. Returns `pending`
    /// unchanged unless this request is the closing one, in which case the
    /// notice follows the existing feedback (or stands alone).
    pub(crate) fn request_feedback<'a>(
        &mut self,
        remaining_iterations: usize,
        iteration_limit: usize,
        pending: Option<&'a str>,
    ) -> Option<Cow<'a, str>> {
        let remaining_calls = remaining_iterations + 1;
        if self.phase != Phase::Armed
            || iteration_limit <= CLOSING_REMAINING_CALLS
            || remaining_calls != CLOSING_REMAINING_CALLS
        {
            return pending.map(Cow::Borrowed);
        }
        let iteration = iteration_limit - remaining_iterations;
        self.staged = Some((iteration, remaining_calls, iteration_limit));
        Some(Cow::Owned(match pending {
            Some(pending) => format!("{pending}\n\n{NOTICE}"),
            None => NOTICE.to_string(),
        }))
    }

    /// Called with the messages of the request about to be sent. A composed
    /// notice is recorded only when that request really carries it.
    pub(crate) fn note_request(&mut self, messages: &[ConversationMessage]) {
        let Some((iteration, remaining_calls, iteration_limit)) = self.staged.take() else {
            return;
        };
        if self.phase != Phase::Armed
            || !messages
                .iter()
                .any(|message| message.content.contains(NOTICE))
        {
            return;
        }
        self.phase = Phase::Noticed;
        self.iteration_limit = iteration_limit;
        self.emit(EventPhase::Notice, iteration, remaining_calls);
    }

    fn recording(&self) -> bool {
        matches!(self.phase, Phase::Armed | Phase::Noticed)
    }

    /// A non-substituted Bash call reached the execution-progress
    /// classification. `command` is the same recovered command string the
    /// classification uses.
    pub(crate) fn note_bash(&mut self, command: Option<&str>, progress: ExecutionProgress) {
        if !self.recording() {
            return;
        }
        let command = command.map(str::trim).filter(|command| !command.is_empty());
        match (progress, command) {
            (ExecutionProgress::NotSuccessful, Some(command)) => {
                self.ledger.bash.insert(command.to_string());
            }
            (ExecutionProgress::NotSuccessful, None) => self.ledger.unidentified_bash += 1,
            (ExecutionProgress::New | ExecutionProgress::Repeated, Some(command)) => {
                if self.ledger.bash.remove(command) {
                    self.ledger.resolved += 1;
                }
            }
            (ExecutionProgress::New | ExecutionProgress::Repeated, None) => {}
        }
    }

    /// A Bash call was refused or replaced by policy before it ran. It is
    /// neither a failure nor a success; only the count is kept.
    pub(crate) fn note_bash_rejected(&mut self, tool_name: &str) {
        if self.recording() && tool_name == "Bash" {
            self.ledger.rejected_bash += 1;
        }
    }

    /// A Write or Edit call ended in a recoverable error or was rejected.
    pub(crate) fn note_write_failure(&mut self, tool_name: &str, arguments: &Value) {
        if !self.recording() {
            return;
        }
        match tool_name {
            "Edit" => match self.edit_operation(arguments) {
                Some(operation) => self.ledger.edits.push(operation),
                None => self.ledger.unidentified_writes += 1,
            },
            "Write" => match self.write_operation(arguments) {
                Some((operation, _)) => self.ledger.writes.push(operation),
                None => self.ledger.unidentified_writes += 1,
            },
            _ => {}
        }
    }

    /// A Write or Edit call succeeded. Only entries the success mechanically
    /// matches are resolved.
    pub(crate) fn note_write_success(&mut self, tool_name: &str, arguments: &Value) {
        if !self.recording() {
            return;
        }
        let before = self.ledger.unresolved_writes();
        if tool_name == "Edit"
            && let Some(operation) = self.edit_operation(arguments)
        {
            self.ledger.edits.retain(|failed| *failed != operation);
        } else if tool_name == "Write"
            && let Some((operation, content)) = self.write_operation(arguments)
        {
            self.ledger.writes.retain(|failed| *failed != operation);
            self.ledger.edits.retain(|failed| {
                failed.path != operation.path || !edit_effect_present(failed, content)
            });
        }
        self.ledger.resolved += before - self.ledger.unresolved_writes();
    }

    /// Called at a normal assistant final that already passed every existing
    /// completion check. Returns the deferral feedback when the final must
    /// not be returned; `None` lets the run return it unchanged.
    pub(crate) fn before_normal_final(
        &mut self,
        iteration: usize,
        remaining_iterations: usize,
    ) -> Option<&'static str> {
        if self.phase != Phase::Noticed {
            return None;
        }
        let remaining_calls = remaining_iterations + 1;
        if self.ledger.unresolved() > 0 {
            self.deferrals += 1;
            self.emit(EventPhase::FinalDeferred, iteration, remaining_calls);
            return Some(DEFERRAL_FEEDBACK);
        }
        self.phase = Phase::Finished;
        self.emit(EventPhase::FinalAccepted, iteration, remaining_calls);
        None
    }

    /// The run left the loop because its iteration budget was spent.
    pub(crate) fn note_budget_exhausted(&mut self) {
        if self.phase != Phase::Noticed {
            return;
        }
        self.phase = Phase::Finished;
        self.emit(EventPhase::ExhaustedAfterNotice, self.iteration_limit, 0);
    }

    /// The target as the tools normalize it, with `.` components dropped.
    /// Paths that still differ are different targets.
    fn workspace_path(&self, arguments: &Value) -> Option<String> {
        let raw = arguments.get("path")?.as_str()?;
        let path = match normalize_workspace_path(&self.root, raw) {
            Ok(Some(normalization)) => normalization.relative,
            Ok(None) | Err(_) => raw.to_string(),
        };
        let path: PathBuf = Path::new(&path)
            .components()
            .filter(|component| *component != Component::CurDir)
            .collect();
        Some(path.to_string_lossy().into_owned())
    }

    fn edit_operation(&self, arguments: &Value) -> Option<EditOperation> {
        Some(EditOperation {
            path: self.workspace_path(arguments)?,
            old: arguments.get("old_string")?.as_str()?.to_string(),
            new: arguments.get("new_string")?.as_str()?.to_string(),
            replace_all: arguments
                .get("replace_all")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })
    }

    fn write_operation<'a>(&self, arguments: &'a Value) -> Option<(WriteOperation, &'a str)> {
        let content = arguments.get("content")?.as_str()?;
        let operation = WriteOperation {
            path: self.workspace_path(arguments)?,
            digest: Sha256::digest(content.as_bytes()).into(),
        };
        Some((operation, content))
    }

    fn emit(&self, phase: EventPhase, iteration: usize, remaining_calls: usize) {
        eval_events::emit(
            self.events_path.as_deref(),
            json!({
                "event": EVENT,
                "phase": phase.as_str(),
                "guard_reason": phase.guard_reason(),
                "prompt_version": PROMPT_VERSION,
                "iteration": iteration,
                "iteration_limit": self.iteration_limit,
                "remaining_calls": remaining_calls,
                "unresolved_bash_failures": self.ledger.unresolved_bash(),
                "unresolved_write_failures": self.ledger.unresolved_writes(),
                "resolved_failures": self.ledger.resolved,
                "policy_rejected_bash": self.ledger.rejected_bash,
                "final_deferrals": self.deferrals,
            }),
        );
    }
}

/// Whether a successful Write's whole content shows the failed Edit's effect:
/// the replacement is present and the replaced text is gone (or is part of
/// the replacement). An Edit without replaced text has no defined effect.
fn edit_effect_present(failed: &EditOperation, content: &str) -> bool {
    !failed.old.is_empty()
        && content.contains(&failed.new)
        && (failed.new.contains(&failed.old) || !content.contains(&failed.old))
}

#[cfg(test)]
mod tests;
