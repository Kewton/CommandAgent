//! Issue #945: an experimental, opt-in requirement plan for generic runs.
//!
//! When `COMMANDAGENT_REQUIREMENT_PLAN=once` is set, the first request of a new
//! generic MinimalLoop run without a completion contract carries one fixed
//! guidance message. It asks the model to open its first reply with a short
//! `<requirement_plan>` JSON block: the explicit obligations of the request
//! and at most two small check examples whose expected outcome is decided from
//! the request before any implementation.
//!
//! The host takes that block out of the first reply before any XML tool
//! extraction or tool execution, validates its shape and bounds, and keeps the
//! normalized note in run memory only. Later requests of the same run carry the
//! note exactly once, as assistant-side context with a fixed header that says
//! it is the model's own unverified memo and that the original request and any
//! later user correction take precedence. A missing or invalid block is never
//! asked for again; the run simply continues without a note.
//!
//! The note is a model hypothesis. Nothing here reads it to raise verification,
//! completion, acceptance, or assurance, and no budget, counter, or stop path
//! changes because of it. Unset, empty, or `off` disables everything: no state
//! changes, no request change, and no event. Any other value is a configuration
//! error that names the variable but never the value.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::loop_run::{RunSessionOptions, RunSessionScope};
use crate::config::Config;
use crate::eval_events;
use crate::planner::profile_descriptor::GENERIC_PROFILE_ID;
use crate::providers::AssistantReply;
use crate::state::ConversationMessage;

/// The opt-in environment variable. It has no `ANVIL_*` legacy name.
pub(crate) const ENV_NAME: &str = "COMMANDAGENT_REQUIREMENT_PLAN";
/// The Issue #940 review variable; both experiments cannot run together.
const REVIEW_ENV_NAME: &str = super::completion_obligation_review::ENV_NAME;
/// Version of [`GUIDANCE`], [`NOTE_HEADER`], and [`CONTINUE_AFTER_NOTE`].
pub(crate) const PROMPT_VERSION: u32 = 1;
/// The plan is requested only when at least this many loop iterations remain
/// after the first request.
pub(crate) const MIN_REMAINING_ITERATIONS: usize = 4;
/// Upper bound of the raw block between the tags, in UTF-8 bytes.
pub(crate) const MAX_BLOCK_BYTES: usize = 8 * 1024;
/// Upper bound of the normalized note, in Unicode scalar values.
pub(crate) const MAX_NOTE_CHARS: usize = 2048;
pub(crate) const MAX_OBLIGATIONS: usize = 5;
pub(crate) const MAX_EXAMPLES: usize = 2;

const OPEN_TAG: &str = "<requirement_plan>";
const CLOSE_TAG: &str = "</requirement_plan>";

/// The fixed guidance appended once to the first request (prompt version 1).
pub(crate) const GUIDANCE: &str = "作業を始める前に一度だけ、この返答の冒頭に要求計画ブロックを次の形式で書いてください: <requirement_plan>{\"obligations\":[{\"id\":\"O1\",\"stated_requirement\":\"依頼に明記された義務\",\"expected_behavior\":\"依頼から導いた期待結果\",\"assumption\":\"曖昧な点の仮定・未確定事項（無ければ空文字）\"}],\"examples\":[{\"obligation_id\":\"O1\",\"input_outline\":\"確認に使う入力の概要\",\"expected_outcome\":\"依頼から先に決めた期待結果\",\"reason_from_request\":\"その期待結果を依頼のどこから導いたか\"}]}</requirement_plan>\n元の依頼に明記された義務を最大5件（目安3〜5件。実際に少なければ水増ししない）、重要な確認例を最大2件にしてください。ノートは依頼だけから作り、期待結果は実装やライブラリの既定動作に合わせず先に決めてください。曖昧な点は仮定・未確定として assumption に書き、確認できない例は発明しないでください。明示された義務の無い説明などの依頼ではブロックを省略して構いません。ブロックの後は同じ返答で通常どおり作業を始めて構いません。実装後は選んだ例の観測結果を先に書いた期待結果と比べ、矛盾したら元の依頼と期待結果の両方を見直してください。試験や仕様を実装に合わせて弱めないでください。このノートは確認済みの証拠ではありません。";

/// The fixed header in front of the retained note.
pub(crate) const NOTE_HEADER: &str = "[requirement plan note v1: an unverified memo the model wrote before implementing. The original request and any later user correction take precedence over it.]";

/// The fixed user turn that follows the note when no assistant turn follows
/// the original request in the outgoing request.
pub(crate) const CONTINUE_AFTER_NOTE: &str = "Continue the original request. The requirement plan note above is your own unverified memo, not a requirement.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlanMode {
    Off,
    Once,
}

const INVALID_VALUE: &str = "expected unset, empty, `off`, or `once`";

/// Parses the raw environment value. The error never contains the value.
pub(crate) fn parse_mode(raw: Option<&std::ffi::OsStr>) -> anyhow::Result<PlanMode> {
    let Some(raw) = raw else {
        return Ok(PlanMode::Off);
    };
    match raw.to_str().map(str::trim) {
        Some("" | "off") => Ok(PlanMode::Off),
        Some("once") => Ok(PlanMode::Once),
        _ => anyhow::bail!("invalid {ENV_NAME}: {INVALID_VALUE}"),
    }
}

#[cfg(not(test))]
fn read_env(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

#[cfg(test)]
thread_local! {
    /// Unit tests set the variables per thread so a parallel test never sees them.
    static TEST_ENV: std::cell::RefCell<Vec<(&'static str, OsString)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn read_env(name: &str) -> Option<OsString> {
    TEST_ENV.with(|values| {
        values
            .borrow()
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reason {
    PlanRunStep,
    ProfileNotGeneric,
    CompletionContract,
    ResumedConversation,
    InsufficientRemainingIterations,
    NoProviderRequest,
    FirstRequest,
    ValidBlock,
    Missing,
    Invalid,
    Oversize,
    UserUpdate,
    RunCompleted,
    RunInterrupted,
    BudgetExhausted,
    RunFailed,
}

impl Reason {
    fn as_str(self) -> &'static str {
        match self {
            Self::PlanRunStep => "plan_run_step",
            Self::ProfileNotGeneric => "profile_not_generic",
            Self::CompletionContract => "completion_contract",
            Self::ResumedConversation => "resumed_conversation",
            Self::InsufficientRemainingIterations => "insufficient_remaining_iterations",
            Self::NoProviderRequest => "no_provider_request",
            Self::FirstRequest => "first_request",
            Self::ValidBlock => "valid_block",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Oversize => "oversize",
            Self::UserUpdate => "user_update",
            Self::RunCompleted => "run_completed",
            Self::RunInterrupted => "run_interrupted",
            Self::BudgetExhausted => "budget_exhausted",
            Self::RunFailed => "run_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventPhase {
    Requested,
    Captured,
    Skipped,
    Released,
}

impl EventPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Captured => "captured",
            Self::Skipped => "skipped",
            Self::Released => "released",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Obligation {
    id: String,
    stated_requirement: String,
    expected_behavior: String,
    assumption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Example {
    obligation_id: String,
    input_outline: String,
    expected_outcome: String,
    reason_from_request: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteBody {
    obligations: Vec<Obligation>,
    #[serde(default)]
    examples: Vec<Example>,
}

/// A validated, normalized, and scrubbed note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Note {
    text: String,
    obligations: usize,
    examples: usize,
}

impl Note {
    fn chars(&self) -> usize {
        self.text.chars().count()
    }
}

/// Why a block could not be kept. The variants never carry block content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockRejection {
    Missing,
    Invalid,
    Oversize,
}

/// The first reply after the block was taken out.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Extraction {
    pub note: Result<Note, BlockRejection>,
    /// The reply content without the block (and without the unterminated
    /// rest of an unclosed block).
    pub content: String,
}

/// Takes the `<requirement_plan>` block out of `content` and validates it.
///
/// A block must open the reply (after leading whitespace). A tag anywhere else
/// makes the block invalid, and an unclosed block drops the rest of the reply
/// so nothing inside it ever reaches the XML tool parser.
pub(crate) fn extract_block(content: &str, scrub: impl Fn(&str) -> String) -> Extraction {
    let Some(open) = content.find(OPEN_TAG) else {
        return Extraction {
            note: Err(BlockRejection::Missing),
            content: content.to_string(),
        };
    };
    let leading = content[..open].trim().is_empty();
    let body_start = open + OPEN_TAG.len();
    let Some(close_rel) = content[body_start..].find(CLOSE_TAG) else {
        return Extraction {
            note: Err(BlockRejection::Invalid),
            content: content[..open].trim_end().to_string(),
        };
    };
    let body_end = body_start + close_rel;
    let after = &content[body_end + CLOSE_TAG.len()..];
    let rest = if leading {
        after.trim_start().to_string()
    } else {
        format!("{}{}", &content[..open], after)
    };
    let note = if !leading {
        Err(BlockRejection::Invalid)
    } else {
        validate_body(&content[body_start..body_end], scrub)
    };
    Extraction {
        note,
        content: rest,
    }
}

fn validate_body(raw: &str, scrub: impl Fn(&str) -> String) -> Result<Note, BlockRejection> {
    if raw.len() > MAX_BLOCK_BYTES {
        return Err(BlockRejection::Oversize);
    }
    let mut body: NoteBody =
        serde_json::from_str(raw.trim()).map_err(|_| BlockRejection::Invalid)?;
    if body.obligations.is_empty()
        || body.obligations.len() > MAX_OBLIGATIONS
        || body.examples.len() > MAX_EXAMPLES
    {
        return Err(BlockRejection::Invalid);
    }
    let mut ids: Vec<String> = Vec::new();
    for obligation in &mut body.obligations {
        for field in [
            &mut obligation.id,
            &mut obligation.stated_requirement,
            &mut obligation.expected_behavior,
            &mut obligation.assumption,
        ] {
            *field = field.trim().to_string();
        }
        if obligation.id.is_empty()
            || obligation.stated_requirement.is_empty()
            || obligation.expected_behavior.is_empty()
            || ids.contains(&obligation.id)
        {
            return Err(BlockRejection::Invalid);
        }
        ids.push(obligation.id.clone());
    }
    for example in &mut body.examples {
        for field in [
            &mut example.obligation_id,
            &mut example.input_outline,
            &mut example.expected_outcome,
            &mut example.reason_from_request,
        ] {
            *field = field.trim().to_string();
        }
        if !ids.contains(&example.obligation_id)
            || example.input_outline.is_empty()
            || example.expected_outcome.is_empty()
            || example.reason_from_request.is_empty()
        {
            return Err(BlockRejection::Invalid);
        }
    }
    let normalized = serde_json::to_string(&body).map_err(|_| BlockRejection::Invalid)?;
    let text = scrub(&normalized);
    if text.chars().count() > MAX_NOTE_CHARS {
        return Err(BlockRejection::Oversize);
    }
    Ok(Note {
        text,
        obligations: body.obligations.len(),
        examples: body.examples.len(),
    })
}

/// The assistant-side context that carries the note in a request.
pub(crate) fn note_context(note: &str) -> String {
    format!("{NOTE_HEADER}\n{note}")
}

/// Inserts the note once after the original request (the first user message):
/// into the assistant turn that follows it, or as a new assistant turn plus the
/// fixed continuation when no assistant turn follows. Nothing is replaced.
pub(crate) fn insert_note(messages: &mut Vec<ConversationMessage>, note: &str) {
    let Some(prompt) = messages.iter().position(|message| message.role == "user") else {
        return;
    };
    let context = note_context(note);
    match messages.get_mut(prompt + 1) {
        Some(next) if next.role == "assistant" => {
            next.content = if next.content.is_empty() {
                context
            } else {
                format!("{context}\n\n{}", next.content)
            };
        }
        _ => {
            messages.insert(
                prompt + 1,
                ConversationMessage::assistant(context, Vec::new()),
            );
            messages.insert(prompt + 2, ConversationMessage::user(CONTINUE_AFTER_NOTE));
        }
    }
}

/// What the run is, decided once at run start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RunScope {
    minimal_loop: bool,
    generic_profile: bool,
    completion_contract: bool,
    new_conversation: bool,
}

impl RunScope {
    fn skip_reason(self) -> Option<Reason> {
        if !self.minimal_loop {
            Some(Reason::PlanRunStep)
        } else if !self.generic_profile {
            Some(Reason::ProfileNotGeneric)
        } else if self.completion_contract {
            Some(Reason::CompletionContract)
        } else if !self.new_conversation {
            Some(Reason::ResumedConversation)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Disabled,
    Armed,
    Requested,
    Holding(Note),
    Done,
}

/// Per-run requirement plan state. It lives only for one run: nothing is
/// written to the session, to a file, or to the runtime namespace.
#[derive(Debug)]
pub(crate) struct RequirementPlan {
    state: State,
    events_path: Option<PathBuf>,
    /// Loop iterations that reached a request (1-based, as the loop counts).
    iteration: usize,
    /// Remaining iterations at the latest request, to tell a budget end
    /// from another failure when the run returns an error.
    last_remaining: Option<usize>,
}

impl Default for RequirementPlan {
    fn default() -> Self {
        Self {
            state: State::Disabled,
            events_path: None,
            iteration: 0,
            last_remaining: None,
        }
    }
}

impl RequirementPlan {
    /// Reads the environment once, at run start, before any provider call.
    pub(crate) fn start(
        &mut self,
        config: &Config,
        options: &RunSessionOptions,
        completion_contract: bool,
        history: &[ConversationMessage],
    ) -> anyhow::Result<()> {
        let mode = parse_mode(read_env(ENV_NAME).as_deref())?;
        if mode == PlanMode::Once
            && super::completion_obligation_review::parse_mode(read_env(REVIEW_ENV_NAME).as_deref())
                .is_ok_and(|review| review == super::completion_obligation_review::ReviewMode::Once)
        {
            anyhow::bail!("{ENV_NAME} and {REVIEW_ENV_NAME} cannot both be enabled");
        }
        let scope = RunScope {
            minimal_loop: options.scope == RunSessionScope::MinimalLoop,
            generic_profile: config.profile == GENERIC_PROFILE_ID,
            completion_contract,
            new_conversation: history.is_empty(),
        };
        *self = Self::with_mode(mode, config.eval_events_path.as_deref(), scope);
        Ok(())
    }

    fn with_mode(mode: PlanMode, events_path: Option<&Path>, scope: RunScope) -> Self {
        let mut plan = Self {
            state: match mode {
                PlanMode::Off => State::Disabled,
                PlanMode::Once => State::Armed,
            },
            events_path: events_path.map(Path::to_path_buf),
            iteration: 0,
            last_remaining: None,
        };
        if plan.state == State::Armed
            && let Some(reason) = scope.skip_reason()
        {
            plan.terminal(EventPhase::Skipped, reason, None, None);
        }
        plan
    }

    /// Adjusts the outgoing request of one loop iteration: the guidance on the
    /// first request, the retained note on later ones. `history` is the run's
    /// conversation; the loop calls this exactly once per iteration.
    pub(crate) fn prepare_request(
        &mut self,
        messages: &mut Vec<ConversationMessage>,
        history: &[ConversationMessage],
        remaining_iterations: usize,
    ) {
        self.iteration += 1;
        self.last_remaining = Some(remaining_iterations);
        let at = Some((self.iteration, remaining_iterations));
        match &self.state {
            State::Armed if remaining_iterations < MIN_REMAINING_ITERATIONS => {
                self.terminal(
                    EventPhase::Skipped,
                    Reason::InsufficientRemainingIterations,
                    at,
                    None,
                );
            }
            State::Armed => {
                messages.push(ConversationMessage::user(GUIDANCE));
                self.state = State::Requested;
                self.emit(EventPhase::Requested, Reason::FirstRequest, at, None, false);
            }
            State::Holding(_) if user_update_after_request(history) => {
                self.terminal(EventPhase::Released, Reason::UserUpdate, at, None);
            }
            State::Holding(note) => insert_note(messages, &note.text),
            State::Disabled | State::Requested | State::Done => {}
        }
    }

    /// Takes the block out of the first reply received after the guidance,
    /// before any XML tool extraction or tool execution. Returns true when the
    /// reply was only a valid block: a planning turn the loop consumes once.
    pub(crate) fn capture_reply(&mut self, reply: &mut AssistantReply) -> bool {
        if self.state != State::Requested {
            return false;
        }
        let extraction = extract_block(&reply.content, |text| {
            crate::sensitive_data::active_for(self.events_path.as_deref())
                .map(|context| context.scrub_text(text))
                .unwrap_or_else(|| text.to_string())
        });
        reply.content = extraction.content;
        let at = self
            .last_remaining
            .map(|remaining| (self.iteration, remaining));
        match extraction.note {
            Ok(note) => {
                let plan_only = reply.tool_calls.is_empty() && reply.content.trim().is_empty();
                self.emit(
                    EventPhase::Captured,
                    Reason::ValidBlock,
                    at,
                    Some(&note),
                    plan_only,
                );
                self.state = State::Holding(note);
                plan_only
            }
            Err(rejection) => {
                let reason = match rejection {
                    BlockRejection::Missing => Reason::Missing,
                    BlockRejection::Invalid => Reason::Invalid,
                    BlockRejection::Oversize => Reason::Oversize,
                };
                self.terminal(EventPhase::Released, reason, at, None);
                false
            }
        }
    }

    /// Records how the run ended and drops the note.
    pub(crate) fn finish(&mut self, completed: bool, interrupted: bool) {
        let reason = match self.state {
            State::Disabled | State::Done => return,
            State::Armed => {
                self.terminal(EventPhase::Skipped, Reason::NoProviderRequest, None, None);
                return;
            }
            State::Requested | State::Holding(_) if completed => Reason::RunCompleted,
            State::Requested | State::Holding(_) if interrupted => Reason::RunInterrupted,
            State::Requested | State::Holding(_) if self.last_remaining == Some(0) => {
                Reason::BudgetExhausted
            }
            State::Requested | State::Holding(_) => Reason::RunFailed,
        };
        self.terminal(EventPhase::Released, reason, None, None);
    }

    fn terminal(
        &mut self,
        phase: EventPhase,
        reason: Reason,
        at: Option<(usize, usize)>,
        note: Option<&Note>,
    ) {
        self.state = State::Done;
        self.emit(phase, reason, at, note, false);
    }

    fn emit(
        &self,
        phase: EventPhase,
        reason: Reason,
        at: Option<(usize, usize)>,
        note: Option<&Note>,
        plan_only_turn: bool,
    ) {
        eval_events::emit(
            self.events_path.as_deref(),
            json!({
                "event": "requirement_plan",
                "phase": phase.as_str(),
                "reason": reason.as_str(),
                "prompt_version": PROMPT_VERSION,
                "iteration": at.map(|(iteration, _)| iteration),
                "remaining_iterations": at.map(|(_, remaining)| remaining),
                "obligation_count": note.map_or(0, |note| note.obligations),
                "example_count": note.map_or(0, |note| note.examples),
                "note_chars": note.map_or(0, Note::chars),
                "plan_only_turn": plan_only_turn,
            }),
        );
    }
}

/// True when the run's history holds a user message after the original
/// request: a later user correction makes the retained note stale.
fn user_update_after_request(history: &[ConversationMessage]) -> bool {
    history
        .iter()
        .skip_while(|message| message.role != "user")
        .skip(1)
        .any(|message| message.role == "user")
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};

    use super::{
        BlockRejection, CONTINUE_AFTER_NOTE, ENV_NAME, GUIDANCE, MAX_BLOCK_BYTES, MAX_EXAMPLES,
        MAX_NOTE_CHARS, MAX_OBLIGATIONS, MIN_REMAINING_ITERATIONS, NOTE_HEADER, PlanMode,
        REVIEW_ENV_NAME, RequirementPlan, RunScope, TEST_ENV, extract_block, insert_note,
        parse_mode,
    };
    use crate::config::Config;
    use crate::minimal_loop::loop_run::{
        RunSessionOptions, RunSessionStepKind, run_session_with_outcome_with_options,
    };
    use crate::providers::{AssistantReply, ChatClient};
    use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
    use crate::tools::registry::ToolSpec;
    use crate::tui::NOOP_UI;

    const ELIGIBLE: RunScope = RunScope {
        minimal_loop: true,
        generic_profile: true,
        completion_contract: false,
        new_conversation: true,
    };

    fn plain(text: &str) -> String {
        text.to_string()
    }

    fn obligation(id: &str) -> Value {
        json!({
            "id": id,
            "stated_requirement": "write the summary file",
            "expected_behavior": "summary.txt holds one line",
            "assumption": "",
        })
    }

    fn example(obligation_id: &str) -> Value {
        json!({
            "obligation_id": obligation_id,
            "input_outline": "read summary.txt",
            "expected_outcome": "one line",
            "reason_from_request": "the request asks for one line",
        })
    }

    fn block(body: &Value) -> String {
        format!("<requirement_plan>{body}</requirement_plan>")
    }

    fn valid_body() -> Value {
        json!({"obligations": [obligation("O1")], "examples": [example("O1")]})
    }

    fn rejection(content: &str) -> Option<BlockRejection> {
        extract_block(content, plain).note.err()
    }

    fn read_events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect()
    }

    fn new_plan(mode: PlanMode, scope: RunScope) -> (tempfile::TempDir, RequirementPlan) {
        let dir = tempfile::tempdir().unwrap();
        let plan = RequirementPlan::with_mode(mode, Some(&dir.path().join("e.jsonl")), scope);
        (dir, plan)
    }

    fn phases(dir: &tempfile::TempDir) -> Vec<(String, String)> {
        read_events(&dir.path().join("e.jsonl"))
            .iter()
            .map(|event| {
                (
                    event["phase"].as_str().unwrap().to_string(),
                    event["reason"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn pair(phase: &str, reason: &str) -> (String, String) {
        (phase.to_string(), reason.to_string())
    }

    fn base_request() -> Vec<ConversationMessage> {
        vec![
            ConversationMessage::system("system"),
            ConversationMessage::user("the original request"),
        ]
    }

    fn count_in(messages: &[ConversationMessage], needle: &str) -> usize {
        messages
            .iter()
            .map(|message| message.content.matches(needle).count())
            .sum()
    }

    #[test]
    fn mode_accepts_only_unset_empty_off_and_once() {
        for (raw, expected) in [
            (None, PlanMode::Off),
            (Some(""), PlanMode::Off),
            (Some("  "), PlanMode::Off),
            (Some("off"), PlanMode::Off),
            (Some(" off\n"), PlanMode::Off),
            (Some("once"), PlanMode::Once),
            (Some(" once "), PlanMode::Once),
        ] {
            assert_eq!(
                parse_mode(raw.map(OsStr::new)).unwrap(),
                expected,
                "{raw:?}"
            );
        }
        for value in [
            "ONCE",
            "Off",
            "on",
            "true",
            "1",
            "twice",
            "sk-test-945-NOT-A-KEY",
        ] {
            let error = parse_mode(Some(OsStr::new(value))).unwrap_err().to_string();
            assert_eq!(
                error,
                format!("invalid {ENV_NAME}: expected unset, empty, `off`, or `once`")
            );
            assert!(!error.contains("sk-test"), "{error}");
        }
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let raw = OsStr::from_bytes(&[0x6f, 0x6e, 0x63, 0x65, 0xff]);
            let error = parse_mode(Some(raw)).unwrap_err().to_string();
            assert_eq!(
                error,
                format!("invalid {ENV_NAME}: expected unset, empty, `off`, or `once`")
            );
        }
    }

    #[test]
    fn a_valid_leading_block_is_taken_out_and_normalized() {
        let content = format!("  {}\nI will start now.", block(&valid_body()));
        let extraction = extract_block(&content, plain);
        let note = extraction.note.unwrap();
        assert_eq!(extraction.content, "I will start now.");
        assert_eq!((note.obligations, note.examples), (1, 1));
        assert_eq!(
            serde_json::from_str::<Value>(&note.text).unwrap(),
            valid_body()
        );
        assert!(!note.text.contains(": "), "compact: {}", note.text);
        assert!(!note.text.contains("<requirement_plan>"));

        // Strings are trimmed; `examples` may be absent.
        let body = json!({"obligations": [{
            "id": " O1 ", "stated_requirement": " a ", "expected_behavior": " b ",
            "assumption": " c ",
        }]});
        let note = extract_block(&block(&body), plain).note.unwrap();
        assert_eq!(
            note.text,
            r#"{"obligations":[{"id":"O1","stated_requirement":"a","expected_behavior":"b","assumption":"c"}],"examples":[]}"#
        );
        assert_eq!(note.examples, 0);
    }

    #[test]
    fn missing_misplaced_unclosed_and_broken_blocks_are_rejected_whole() {
        assert_eq!(rejection("plain answer"), Some(BlockRejection::Missing));
        assert_eq!(
            rejection(&format!("Plan first:\n{}", block(&valid_body()))),
            Some(BlockRejection::Invalid)
        );
        assert_eq!(
            rejection("<requirement_plan></requirement_plan>"),
            Some(BlockRejection::Invalid)
        );
        assert_eq!(
            rejection("<requirement_plan>   </requirement_plan>"),
            Some(BlockRejection::Invalid)
        );
        let full = valid_body().to_string();
        let truncated = &full[..full.len() / 2];
        assert_eq!(
            rejection(&format!("<requirement_plan>{truncated}</requirement_plan>")),
            Some(BlockRejection::Invalid)
        );
        assert_eq!(
            rejection(&format!("<requirement_plan>{full}")),
            Some(BlockRejection::Invalid)
        );
        assert_eq!(
            rejection("<requirement_plan>[1, 2]</requirement_plan>"),
            Some(BlockRejection::Invalid)
        );
    }

    #[test]
    fn tool_like_tags_inside_a_block_never_reach_the_reply() {
        let tag =
            r#"<anvil_tool_call name="Write">{"path":"leak.txt","content":"x"}</anvil_tool_call>"#;
        // Unclosed: the rest of the reply is dropped with it.
        let unclosed = extract_block(
            &format!("<requirement_plan>{{\"obligations\": [{tag}"),
            plain,
        );
        assert_eq!(unclosed.note, Err(BlockRejection::Invalid));
        assert_eq!(unclosed.content, "");
        // Closed but not JSON: the block is taken out, the tag with it.
        let closed = extract_block(
            &format!("<requirement_plan>{tag}</requirement_plan>\nnext"),
            plain,
        );
        assert_eq!(closed.note, Err(BlockRejection::Invalid));
        assert_eq!(closed.content, "next");
        // Valid JSON whose strings hold a tag: kept as data, not in the reply.
        let mut body = valid_body();
        body["obligations"][0]["assumption"] = json!(tag);
        let valid = extract_block(&block(&body), plain);
        assert!(valid.note.is_ok());
        assert!(!valid.content.contains("anvil_tool_call"));
        // Text before an unclosed misplaced block stays; the block does not.
        let misplaced = extract_block(&format!("intro <requirement_plan>{tag}"), plain);
        assert_eq!(misplaced.content, "intro");
    }

    #[test]
    fn counts_fields_ids_and_references_are_bounded() {
        let ids = (1..=MAX_OBLIGATIONS + 1)
            .map(|index| format!("O{index}"))
            .collect::<Vec<_>>();
        let obligations = |count: usize| {
            ids[..count]
                .iter()
                .map(|id| obligation(id))
                .collect::<Vec<_>>()
        };
        let five = json!({"obligations": obligations(MAX_OBLIGATIONS), "examples": [example("O1"), example("O5")]});
        let note = extract_block(&block(&five), plain).note.unwrap();
        assert_eq!(
            (note.obligations, note.examples),
            (MAX_OBLIGATIONS, MAX_EXAMPLES)
        );

        let cases = [
            json!({"obligations": obligations(MAX_OBLIGATIONS + 1)}),
            json!({"obligations": []}),
            json!({"examples": [example("O1")]}),
            json!({"obligations": obligations(1), "examples": [example("O1"), example("O1"), example("O1")]}),
            json!({"obligations": [obligation("O1"), obligation("O1")]}),
            json!({"obligations": obligations(1), "examples": [example("O9")]}),
            json!({"obligations": obligations(1), "notes": "x"}),
            json!({"obligations": [{"id": "O1", "stated_requirement": "a", "expected_behavior": "b", "assumption": "", "priority": 1}]}),
            json!({"obligations": obligations(1), "examples": [{"obligation_id": "O1", "input_outline": "a", "expected_outcome": "b", "reason_from_request": "c", "verified": true}]}),
            json!({"obligations": [{"id": "O1", "stated_requirement": "a", "expected_behavior": "b"}]}),
            json!({"obligations": [{"id": 1, "stated_requirement": "a", "expected_behavior": "b", "assumption": ""}]}),
            json!({"obligations": [{"id": "O1", "stated_requirement": " ", "expected_behavior": "b", "assumption": ""}]}),
            json!({"obligations": obligations(1), "examples": [{"obligation_id": "O1", "input_outline": "", "expected_outcome": "b", "reason_from_request": "c"}]}),
        ];
        for body in cases {
            assert_eq!(
                rejection(&block(&body)),
                Some(BlockRejection::Invalid),
                "{body}"
            );
        }
        // A repeated JSON key is a parse failure, not a last-wins merge.
        let repeated = r#"{"obligations": [], "obligations": [{"id": "O1", "stated_requirement": "a", "expected_behavior": "b", "assumption": ""}]}"#;
        assert_eq!(
            rejection(&format!("<requirement_plan>{repeated}</requirement_plan>")),
            Some(BlockRejection::Invalid)
        );
    }

    /// A note whose normalized form is exactly `chars` characters long, built
    /// from multi-byte characters so the bound is on characters, not bytes.
    fn note_of_chars(chars: usize) -> String {
        let mut body = valid_body();
        body["obligations"][0]["assumption"] = json!("");
        let base = body.to_string().chars().count();
        body["obligations"][0]["assumption"] = json!("あ".repeat(chars - base));
        let text = body.to_string();
        assert_eq!(text.chars().count(), chars);
        text
    }

    #[test]
    fn size_bounds_are_bytes_for_the_block_and_characters_for_the_note() {
        let at_limit = note_of_chars(MAX_NOTE_CHARS);
        assert!(at_limit.len() > MAX_NOTE_CHARS, "multi-byte note");
        let note = extract_block(
            &format!("<requirement_plan>{at_limit}</requirement_plan>"),
            plain,
        )
        .note
        .unwrap();
        assert_eq!(note.chars(), MAX_NOTE_CHARS);
        assert_eq!(
            rejection(&format!(
                "<requirement_plan>{}</requirement_plan>",
                note_of_chars(MAX_NOTE_CHARS + 1)
            )),
            Some(BlockRejection::Oversize)
        );
        // Whitespace counts toward the raw block bound.
        let padded = format!("{}{}", valid_body(), " ".repeat(MAX_BLOCK_BYTES));
        assert_eq!(
            rejection(&format!("<requirement_plan>{padded}</requirement_plan>")),
            Some(BlockRejection::Oversize)
        );
        let fits = format!(
            "{}{}",
            valid_body(),
            " ".repeat(MAX_BLOCK_BYTES - valid_body().to_string().len())
        );
        assert!(
            extract_block(
                &format!("<requirement_plan>{fits}</requirement_plan>"),
                plain
            )
            .note
            .is_ok()
        );
    }

    #[test]
    fn the_scrub_runs_before_the_note_is_kept() {
        let mut body = valid_body();
        body["obligations"][0]["assumption"] = json!("token sk-test-945-SECRET here");
        let note = extract_block(&block(&body), |text| {
            text.replace("sk-test-945-SECRET", "<redacted>")
        })
        .note
        .unwrap();
        assert!(!note.text.contains("sk-test-945-SECRET"));
        assert!(note.text.contains("<redacted>"));
    }

    #[test]
    fn the_note_goes_once_into_the_assistant_turn_after_the_request() {
        // An assistant turn follows: the note joins it, nothing is replaced.
        let mut messages = base_request();
        messages.push(ConversationMessage::assistant(
            "",
            vec![ToolCall::new("Read", json!({"path": "a"}))],
        ));
        messages.push(ConversationMessage::tool("Read", "a"));
        messages.push(ConversationMessage::user("pending feedback"));
        let before = messages.clone();
        insert_note(&mut messages, "{\"n\":1}");
        assert_eq!(messages.len(), before.len());
        assert_eq!(messages[2].content, format!("{NOTE_HEADER}\n{{\"n\":1}}"));
        assert_eq!(messages[2].tool_calls, before[2].tool_calls);
        for index in [0, 1, 3, 4] {
            assert_eq!(messages[index], before[index]);
        }
        assert_eq!(count_in(&messages, NOTE_HEADER), 1);
        assert!(
            messages
                .iter()
                .all(|message| message.role != "system" || !message.content.contains(NOTE_HEADER))
        );

        // A kept preamble stays after the note.
        let mut messages = base_request();
        messages.push(ConversationMessage::assistant("Done.", Vec::new()));
        insert_note(&mut messages, "N");
        assert_eq!(messages[2].content, format!("{NOTE_HEADER}\nN\n\nDone."));

        // Nothing follows (fresh retry, planning turn): note plus continuation.
        let mut messages = base_request();
        insert_note(&mut messages, "N");
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[3].role, "user");
        assert_eq!(messages[3].content, CONTINUE_AFTER_NOTE);
        assert_eq!(messages[1].content, "the original request");

        // A compacted history whose next turn is not an assistant turn.
        let mut messages = base_request();
        messages.push(ConversationMessage::tool("Read", "a"));
        insert_note(&mut messages, "N");
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[3].content, CONTINUE_AFTER_NOTE);
        assert_eq!(messages[4].role, "tool");
    }

    #[test]
    fn off_never_changes_a_request_or_a_reply_and_never_emits() {
        let (dir, mut plan) = new_plan(PlanMode::Off, ELIGIBLE);
        let history = vec![ConversationMessage::user("the original request")];
        for remaining in [8, 7, 6] {
            let mut messages = base_request();
            plan.prepare_request(&mut messages, &history, remaining);
            assert_eq!(messages, base_request());
            let content = block(&valid_body());
            let mut reply = AssistantReply::text(content.clone());
            assert!(!plan.capture_reply(&mut reply));
            assert_eq!(reply.content, content);
        }
        plan.finish(true, false);
        assert!(read_events(&dir.path().join("e.jsonl")).is_empty());
    }

    #[test]
    fn once_requests_a_single_time_and_holds_the_note_for_later_requests() {
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        let history = vec![ConversationMessage::user("the original request")];
        let mut first = base_request();
        plan.prepare_request(&mut first, &history, 8);
        assert_eq!(first.last().unwrap().content, GUIDANCE);
        assert_eq!(first.last().unwrap().role, "user");
        assert_eq!(&first[..2], &base_request()[..]);
        let mut reply = AssistantReply {
            content: format!("{}\nWriting it.", block(&valid_body())),
            tool_calls: vec![ToolCall::new("Write", json!({"path": "a"}))],
            prompt_tokens: None,
            completion_tokens: None,
        };
        assert!(!plan.capture_reply(&mut reply), "a reply with a tool call");
        assert_eq!(reply.content, "Writing it.");
        for remaining in [7, 6] {
            let mut later = base_request();
            later.push(ConversationMessage::assistant("", reply.tool_calls.clone()));
            plan.prepare_request(&mut later, &history, remaining);
            assert_eq!(count_in(&later, GUIDANCE), 0);
            assert_eq!(count_in(&later, NOTE_HEADER), 1);
            assert_eq!(count_in(&later, "write the summary file"), 1);
            // A later reply with a block is not captured again.
            let mut again = AssistantReply::text(block(&valid_body()));
            assert!(!plan.capture_reply(&mut again));
            assert_eq!(again.content, block(&valid_body()));
        }
        plan.finish(true, false);
        plan.finish(false, true);
        assert_eq!(
            phases(&dir),
            [
                pair("requested", "first_request"),
                pair("captured", "valid_block"),
                pair("released", "run_completed"),
            ]
        );
        let events = read_events(&dir.path().join("e.jsonl"));
        assert_eq!(events[1]["obligation_count"], 1);
        assert_eq!(events[1]["example_count"], 1);
        assert_eq!(
            events[1]["note_chars"],
            valid_body().to_string().chars().count()
        );
        assert_eq!(events[1]["plan_only_turn"], false);
        assert_eq!(events[1]["iteration"], 1);
        assert_eq!(events[1]["remaining_iterations"], 8);
        assert_eq!(events[2]["iteration"], Value::Null);
    }

    #[test]
    fn a_block_only_reply_is_a_planning_turn() {
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        let history = vec![ConversationMessage::user("the original request")];
        plan.prepare_request(&mut base_request(), &history, 8);
        let mut reply = AssistantReply::text(format!("{}\n  ", block(&valid_body())));
        assert!(plan.capture_reply(&mut reply));
        assert_eq!(reply.content, "");
        let mut next = base_request();
        plan.prepare_request(&mut next, &history, 7);
        assert_eq!(next.len(), 4);
        assert_eq!(next[3].content, CONTINUE_AFTER_NOTE);
        let events = read_events(&dir.path().join("e.jsonl"));
        assert_eq!(events[1]["plan_only_turn"], true);
    }

    #[test]
    fn missing_invalid_and_oversize_release_without_asking_again() {
        let oversize = format!(
            "<requirement_plan>{}</requirement_plan>",
            note_of_chars(MAX_NOTE_CHARS + 1)
        );
        for (content, reason) in [
            ("I will write it.".to_string(), "missing"),
            ("<requirement_plan>{".to_string(), "invalid"),
            (oversize, "oversize"),
        ] {
            let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
            let history = vec![ConversationMessage::user("the original request")];
            plan.prepare_request(&mut base_request(), &history, 8);
            let mut reply = AssistantReply::text(content);
            assert!(!plan.capture_reply(&mut reply), "{reason}");
            for remaining in [7, 6] {
                let mut later = base_request();
                plan.prepare_request(&mut later, &history, remaining);
                assert_eq!(later, base_request(), "{reason}: nothing re-requested");
            }
            plan.finish(true, false);
            assert_eq!(
                phases(&dir),
                [pair("requested", "first_request"), pair("released", reason)],
                "{reason}"
            );
        }
    }

    #[test]
    fn a_later_user_message_releases_the_note() {
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        let mut history = vec![ConversationMessage::user("the original request")];
        plan.prepare_request(&mut base_request(), &history, 8);
        plan.capture_reply(&mut AssistantReply::text(block(&valid_body())));
        history.push(ConversationMessage::assistant("", Vec::new()));
        history.push(ConversationMessage::user("Actually, use two lines."));
        let mut later = base_request();
        plan.prepare_request(&mut later, &history, 7);
        assert_eq!(later, base_request());
        let mut again = base_request();
        plan.prepare_request(&mut again, &history, 6);
        assert_eq!(again, base_request());
        plan.finish(true, false);
        assert_eq!(
            phases(&dir),
            [
                pair("requested", "first_request"),
                pair("captured", "valid_block"),
                pair("released", "user_update"),
            ]
        );
    }

    #[test]
    fn scope_exclusions_and_the_remaining_iteration_boundary_skip_once() {
        let history = vec![ConversationMessage::user("the original request")];
        let cases = [
            (
                RunScope {
                    minimal_loop: false,
                    ..ELIGIBLE
                },
                8,
                Some("plan_run_step"),
            ),
            (
                RunScope {
                    generic_profile: false,
                    ..ELIGIBLE
                },
                8,
                Some("profile_not_generic"),
            ),
            (
                RunScope {
                    completion_contract: true,
                    ..ELIGIBLE
                },
                8,
                Some("completion_contract"),
            ),
            (
                RunScope {
                    new_conversation: false,
                    ..ELIGIBLE
                },
                8,
                Some("resumed_conversation"),
            ),
            (
                ELIGIBLE,
                MIN_REMAINING_ITERATIONS - 1,
                Some("insufficient_remaining_iterations"),
            ),
            (ELIGIBLE, MIN_REMAINING_ITERATIONS, None),
        ];
        for (scope, remaining, skipped) in cases {
            let (dir, mut plan) = new_plan(PlanMode::Once, scope);
            let mut first = base_request();
            plan.prepare_request(&mut first, &history, remaining);
            assert_eq!(first.len() == 3, skipped.is_none(), "{scope:?} {remaining}");
            let mut second = base_request();
            plan.prepare_request(&mut second, &history, remaining);
            assert_eq!(second, base_request(), "never on a later request");
            plan.finish(true, false);
            match skipped {
                Some(reason) => assert_eq!(phases(&dir), [pair("skipped", reason)]),
                None => assert_eq!(phases(&dir)[0], pair("requested", "first_request")),
            }
        }
    }

    #[test]
    fn run_end_reasons_are_distinct_and_terminal_once() {
        let history = vec![ConversationMessage::user("the original request")];
        for (completed, interrupted, remaining, reason) in [
            (true, false, 5, "run_completed"),
            (false, true, 5, "run_interrupted"),
            (false, false, 0, "budget_exhausted"),
            (false, false, 3, "run_failed"),
        ] {
            let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
            plan.prepare_request(&mut base_request(), &history, 8);
            plan.prepare_request(&mut base_request(), &history, remaining);
            plan.finish(completed, interrupted);
            plan.finish(true, false);
            assert_eq!(
                phases(&dir),
                [pair("requested", "first_request"), pair("released", reason)]
            );
        }
        // Armed but never asked: one skip at the end.
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        plan.finish(true, false);
        assert_eq!(phases(&dir), [pair("skipped", "no_provider_request")]);
    }

    fn with_env<T>(values: &[(&'static str, &str)], body: impl FnOnce() -> T) -> T {
        TEST_ENV.with(|env| {
            *env.borrow_mut() = values
                .iter()
                .map(|(key, value)| (*key, OsString::from(value)))
                .collect();
        });
        let result = body();
        TEST_ENV.with(|env| env.borrow_mut().clear());
        result
    }

    fn start(config: &Config) -> anyhow::Result<RequirementPlan> {
        let mut plan = RequirementPlan::default();
        plan.start(config, &RunSessionOptions::default(), false, &[])
            .map(|()| plan)
    }

    #[test]
    fn once_with_the_940_review_once_is_refused_by_name_only() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(dir.path());
        let error = with_env(&[(ENV_NAME, "once"), (REVIEW_ENV_NAME, " once ")], || {
            start(&config).unwrap_err().to_string()
        });
        assert_eq!(
            error,
            format!("{ENV_NAME} and {REVIEW_ENV_NAME} cannot both be enabled")
        );
        for values in [
            vec![(ENV_NAME, "off"), (REVIEW_ENV_NAME, "once")],
            vec![(REVIEW_ENV_NAME, "once")],
            vec![(ENV_NAME, "once"), (REVIEW_ENV_NAME, "off")],
            vec![(ENV_NAME, "once")],
        ] {
            with_env(&values, || assert!(start(&config).is_ok(), "{values:?}"));
        }
        assert!(read_events(&dir.path().join("events.jsonl")).is_empty());
    }

    /// Every phase, reason, and field the plan emits beyond the shared
    /// `event`/`schema_version`/`phase`/`reason` keys and the `"1"` version.
    const EMITTED_FIXED_WORDS: [&str; 30] = [
        "requested",
        "captured",
        "skipped",
        "released",
        "plan_run_step",
        "profile_not_generic",
        "completion_contract",
        "resumed_conversation",
        "insufficient_remaining_iterations",
        "no_provider_request",
        "first_request",
        "valid_block",
        "missing",
        "invalid",
        "oversize",
        "user_update",
        "run_completed",
        "run_interrupted",
        "budget_exhausted",
        "run_failed",
        "prompt_version",
        "iteration",
        "remaining_iterations",
        "obligation_count",
        "example_count",
        "note_chars",
        "plan_only_turn",
        "requirement_plan",
        ENV_NAME,
        "once",
    ];

    #[test]
    fn every_emitted_phase_reason_and_field_is_a_fixed_identifier() {
        let history = vec![ConversationMessage::user("the original request")];
        let mut events = Vec::new();
        let mut collect = |dir: tempfile::TempDir| {
            events.extend(read_events(&dir.path().join("e.jsonl")));
        };
        for scope in [
            RunScope {
                minimal_loop: false,
                ..ELIGIBLE
            },
            RunScope {
                generic_profile: false,
                ..ELIGIBLE
            },
            RunScope {
                completion_contract: true,
                ..ELIGIBLE
            },
            RunScope {
                new_conversation: false,
                ..ELIGIBLE
            },
        ] {
            let (dir, _plan) = new_plan(PlanMode::Once, scope);
            collect(dir);
        }
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        plan.prepare_request(&mut base_request(), &history, 1);
        collect(dir);
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        plan.finish(true, false);
        collect(dir);
        for content in ["none", "<requirement_plan>{"] {
            let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
            plan.prepare_request(&mut base_request(), &history, 8);
            plan.capture_reply(&mut AssistantReply::text(content));
            collect(dir);
        }
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        plan.prepare_request(&mut base_request(), &history, 8);
        plan.capture_reply(&mut AssistantReply::text(format!(
            "<requirement_plan>{}</requirement_plan>",
            note_of_chars(MAX_NOTE_CHARS + 1)
        )));
        collect(dir);
        for (completed, interrupted, remaining) in [
            (true, false, 5),
            (false, true, 5),
            (false, false, 0),
            (false, false, 3),
        ] {
            let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
            plan.prepare_request(&mut base_request(), &history, 8);
            plan.capture_reply(&mut AssistantReply::text(block(&valid_body())));
            plan.prepare_request(&mut base_request(), &history, remaining);
            plan.finish(completed, interrupted);
            collect(dir);
        }
        let (dir, mut plan) = new_plan(PlanMode::Once, ELIGIBLE);
        plan.prepare_request(&mut base_request(), &history, 8);
        plan.capture_reply(&mut AssistantReply::text(block(&valid_body())));
        let updated = vec![
            ConversationMessage::user("the original request"),
            ConversationMessage::user("a correction"),
        ];
        plan.prepare_request(&mut base_request(), &updated, 7);
        collect(dir);

        let mut words = std::collections::BTreeSet::new();
        for event in &events {
            assert_eq!(event["event"], "requirement_plan");
            for (key, value) in event.as_object().unwrap() {
                if !matches!(
                    key.as_str(),
                    "event" | "schema_version" | "phase" | "reason"
                ) {
                    words.insert(key.clone());
                }
                if let Some(text) = value.as_str()
                    && !matches!(key.as_str(), "event" | "schema_version")
                {
                    words.insert(text.to_string());
                }
            }
        }
        let emitted = EMITTED_FIXED_WORDS[..27]
            .iter()
            .map(|word| word.to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(words, emitted);
        for word in EMITTED_FIXED_WORDS {
            if word == "once" {
                continue;
            }
            assert!(
                crate::sensitive_data::is_fixed_identifier(word),
                "{word} is not reserved in fixed_identifiers"
            );
            assert!(crate::sensitive_data::collides_with_fixed_identifier(word));
        }
    }

    #[derive(Clone)]
    struct Scripted {
        replies: Arc<Mutex<Vec<AssistantReply>>>,
        requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    }

    impl ChatClient for Scripted {
        fn label(&self) -> &str {
            "scripted"
        }
        fn boxed_clone(&self) -> Box<dyn ChatClient> {
            Box::new(self.clone())
        }
        fn supports_native_tools(&self, _model: &str) -> bool {
            true
        }
        fn chat(
            &mut self,
            _model: &str,
            messages: &[ConversationMessage],
            _tools: &[ToolSpec],
            _native_tools_enabled: bool,
        ) -> anyhow::Result<AssistantReply> {
            self.requests.lock().unwrap().push(messages.to_vec());
            let mut replies = self.replies.lock().unwrap();
            if replies.is_empty() {
                anyhow::bail!("scripted client exhausted");
            }
            Ok(replies.remove(0))
        }
    }

    fn config(root: &Path) -> Config {
        Config {
            workspace_root: root.to_path_buf(),
            state_dir: root.join("state"),
            eval_events_path: Some(root.join("events.jsonl")),
            completion_contract_path: None,
            yes: true,
            offline: false,
            context_budget: 100_000,
            model: "m".to_string(),
            provider: crate::config::Provider::Ollama,
            tool_protocol: None,
            openai_api: crate::config::OpenAiApi::ChatCompletions,
            prompt_layout: crate::config::PromptLayout::Stable,
            plan_preset: crate::config::PlanPreset::None,
            intent_override: None,
            planner_model: "m".to_string(),
            planner_provider: crate::config::Provider::Ollama,
            planner_think: Some(crate::config::OllamaThink::False),
            classifier_model: "m".to_string(),
            classifier_provider: crate::config::Provider::Ollama,
            openai_compatible: None,
            ollama_host: "http://localhost:11434".to_string(),
            ollama_think: None,
            lm_studio_host: "http://localhost:1234".to_string(),
            num_predict: 100,
            max_iterations: 6,
            recovery_plan_auto_runs: 0,
            chat_timeout_secs: 30,
            chat_timeout_source: "override:test".to_string(),
            field_sources: crate::config::ConfigFieldSources::default(),
            chat_retries: 1,
            stream: false,
            resume: None,
            fresh_session: false,
            no_footer: false,
            narration: crate::config::NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: crate::config::Action::Repl,
        }
    }

    /// Runs one block-then-write-then-final script as a plan step.
    fn run_plan_step(setting: Option<&str>) -> (Vec<String>, Vec<Value>) {
        let dir = tempfile::tempdir().unwrap();
        let mut client = Scripted {
            replies: Arc::new(Mutex::new(vec![
                AssistantReply {
                    content: block(&valid_body()),
                    tool_calls: vec![ToolCall {
                        id: "call-0".to_string(),
                        name: "Write".to_string(),
                        arguments: json!({"path": "greeting.txt", "content": "hello\n"}),
                    }],
                    prompt_tokens: None,
                    completion_tokens: None,
                },
                AssistantReply::text("greeting.txt を作成しました。"),
            ])),
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        let values = setting
            .map(|value| vec![(ENV_NAME, value)])
            .unwrap_or_default();
        let result = with_env(&values, || {
            run_session_with_outcome_with_options(
                &mut client,
                &mut SessionSnapshot::new(),
                "Write a short greeting file for the project and report what you did.",
                &[],
                &config(dir.path()),
                &NOOP_UI,
                RunSessionOptions::plan_step(RunSessionStepKind::Unknown),
            )
        });
        result.unwrap();
        let root = dir.path().display().to_string();
        let requests = client
            .requests
            .lock()
            .unwrap()
            .iter()
            .map(|messages| {
                serde_json::to_string(messages)
                    .unwrap()
                    .replace(&root, "<ROOT>")
            })
            .collect();
        (requests, read_events(&dir.path().join("events.jsonl")))
    }

    #[test]
    fn plan_run_step_is_never_planned() {
        let (off_requests, off_events) = run_plan_step(None);
        let (requests, events) = run_plan_step(Some("once"));
        assert_eq!(requests, off_requests);
        let plan_events = events
            .iter()
            .filter(|event| event["event"] == "requirement_plan")
            .collect::<Vec<_>>();
        assert!(
            off_events
                .iter()
                .all(|event| event["event"] != "requirement_plan")
        );
        assert_eq!(plan_events.len(), 1, "{plan_events:?}");
        assert_eq!(plan_events[0]["phase"], "skipped");
        assert_eq!(plan_events[0]["reason"], "plan_run_step");
    }
}
