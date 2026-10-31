//! Issue #940: an experimental, opt-in completion obligation review.
//!
//! When `COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW=once` is set, a generic
//! MinimalLoop edit run without a completion contract defers its first normal
//! assistant final once and hands the model one fixed review request. The
//! review is advisory: the next normal final passes the existing checks again
//! and ends the run; nothing the model says about the review releases or
//! blocks it, no second request is sent, no iteration is added, and no
//! existing counter is reset.
//!
//! Unset, empty, or `off` disables the review completely: no state changes,
//! no extra provider call, and no event. Any other value is a configuration
//! error that names the variable but never the value.
//!
//! The host records, after the request, how many Write/Edit calls succeeded
//! and how many non-substituted Bash results the existing
//! [`ExecutionProgress`](super::execution_progress::ExecutionProgress)
//! classification counted as successful, and drops successful Bash
//! observations older than the latest successful edit. These counts say that
//! a command ran successfully, nothing more: they are not semantic coverage,
//! acceptance, or verification evidence, and the event carries no field that
//! claims otherwise. The event never copies the prompt, a command, model text,
//! tool output, or a secret.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::config::Config;
use crate::eval_events;
use crate::planner::profile_descriptor::GENERIC_PROFILE_ID;

/// The opt-in environment variable. It has no `ANVIL_*` legacy name.
pub(crate) const ENV_NAME: &str = "COMMANDAGENT_COMPLETION_OBLIGATION_REVIEW";
/// Version of [`REVIEW_PROMPT`]; recorded in every event.
pub(crate) const PROMPT_VERSION: u32 = 1;
/// The review is requested only when at least this many loop iterations
/// remain at the deferred final.
pub(crate) const MIN_REMAINING_ITERATIONS: usize = 3;

/// The fixed review request (prompt version 1, Issue #940).
pub(crate) const REVIEW_PROMPT: &str = "完了前に一度だけ確認してください。元の依頼に明記された義務を最大5項目で簡潔に整理し、これまでの実行で何を確認できたか対応づけてください。未確認のうち重要な境界・異常系・出力形式を1〜2件選び、実装を正解の根拠にせず依頼の仕様から期待結果を先に導いてください。既存の許可されたツールでその入力を実行し、観測結果と比較してください。矛盾を観測した場合だけ必要な修正を行い、修正後に再実行してください。既存試験や保護設定を弱めないでください。実行できない点や仕様が曖昧な点は確認済みとせず、残る不確実性を短く報告してください。";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReviewMode {
    Off,
    Once,
}

/// Parses the raw environment value. The error never contains the value.
pub(crate) fn parse_mode(raw: Option<&OsStr>) -> anyhow::Result<ReviewMode> {
    let Some(raw) = raw else {
        return Ok(ReviewMode::Off);
    };
    match raw.to_str().map(str::trim) {
        Some("" | "off") => Ok(ReviewMode::Off),
        Some("once") => Ok(ReviewMode::Once),
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

/// What the run is, decided once at run start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RunScope {
    minimal_loop: bool,
    generic_profile: bool,
    completion_contract: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Disabled,
    Armed,
    Prompted,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SkipReason {
    PlanRunStep,
    ProfileNotGeneric,
    CompletionContract,
    NoSuccessfulEdit,
    InsufficientRemainingIterations,
}

impl SkipReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::PlanRunStep => "plan_run_step",
            Self::ProfileNotGeneric => "profile_not_generic",
            Self::CompletionContract => "completion_contract",
            Self::NoSuccessfulEdit => "no_successful_edit",
            Self::InsufficientRemainingIterations => "insufficient_remaining_iterations",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseReason {
    NextFinal,
    ScopeExitBeforeNextFinal,
}

impl CloseReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::NextFinal => "next_final",
            Self::ScopeExitBeforeNextFinal => "scope_exit_before_next_final",
        }
    }
}

/// Host observations after the review request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Observations {
    edits_after_prompt: usize,
    bash_results_after_prompt: usize,
    successful_bash_after_prompt: usize,
    unsuccessful_bash_after_prompt: usize,
    successful_bash_since_latest_edit: usize,
}

/// Per-run review state. Dropping it while the review is pending records the
/// scope exit once; the drop never changes how the run ends.
#[derive(Debug)]
pub(crate) struct CompletionObligationReview {
    phase: Phase,
    events_path: Option<PathBuf>,
    scope: RunScope,
    edit_observed: bool,
    triggered_iteration: usize,
    observations: Observations,
}

impl CompletionObligationReview {
    /// Reads the environment once, at run start, and builds the run's state.
    pub(crate) fn start(
        config: &Config,
        minimal_loop: bool,
        completion_contract: bool,
    ) -> anyhow::Result<Self> {
        let mode = parse_mode(read_env().as_deref())?;
        let scope = RunScope {
            minimal_loop,
            generic_profile: config.profile == GENERIC_PROFILE_ID,
            completion_contract,
        };
        Ok(Self::with_mode(
            mode,
            config.eval_events_path.as_deref(),
            scope,
        ))
    }

    fn with_mode(mode: ReviewMode, events_path: Option<&Path>, scope: RunScope) -> Self {
        Self {
            phase: match mode {
                ReviewMode::Off => Phase::Disabled,
                ReviewMode::Once => Phase::Armed,
            },
            events_path: events_path.map(Path::to_path_buf),
            scope,
            edit_observed: false,
            triggered_iteration: 0,
            observations: Observations::default(),
        }
    }

    /// A Write or Edit call succeeded. A later edit makes every successful
    /// Bash observation before it stale.
    pub(crate) fn note_successful_edit(&mut self) {
        if self.phase == Phase::Disabled {
            return;
        }
        self.edit_observed = true;
        if self.phase == Phase::Prompted {
            self.observations.edits_after_prompt += 1;
            self.observations.successful_bash_since_latest_edit = 0;
        }
    }

    /// A non-substituted Bash call produced a result; `successful` is the
    /// existing execution-progress classification, not the tool status.
    pub(crate) fn note_bash_result(&mut self, successful: bool) {
        if self.phase != Phase::Prompted {
            return;
        }
        self.observations.bash_results_after_prompt += 1;
        if successful {
            self.observations.successful_bash_after_prompt += 1;
            self.observations.successful_bash_since_latest_edit += 1;
        } else {
            self.observations.unsuccessful_bash_after_prompt += 1;
        }
    }

    /// Called at a normal assistant final that already passed every existing
    /// completion check. Returns the review request when the final is
    /// deferred; `None` lets the run return the final unchanged.
    pub(crate) fn before_normal_final(
        &mut self,
        iteration: usize,
        remaining_iterations: usize,
    ) -> Option<&'static str> {
        match self.phase {
            Phase::Disabled | Phase::Finished => None,
            Phase::Prompted => {
                self.close(
                    CloseReason::NextFinal,
                    Some((iteration, remaining_iterations)),
                );
                None
            }
            Phase::Armed => match self.skip_reason(remaining_iterations) {
                Some(reason) => {
                    self.phase = Phase::Finished;
                    self.emit(json!({
                        "event": "completion_obligation_review",
                        "phase": "skipped",
                        "reason": reason.as_str(),
                        "prompt_version": PROMPT_VERSION,
                        "iteration": iteration,
                        "remaining_iterations": remaining_iterations,
                        "min_remaining_iterations": MIN_REMAINING_ITERATIONS,
                    }));
                    None
                }
                None => {
                    self.phase = Phase::Prompted;
                    self.triggered_iteration = iteration;
                    self.observations = Observations::default();
                    self.emit(json!({
                        "event": "completion_obligation_review",
                        "phase": "triggered",
                        "reason": "normal_final_after_successful_edit",
                        "prompt_version": PROMPT_VERSION,
                        "iteration": iteration,
                        "remaining_iterations": remaining_iterations,
                        "min_remaining_iterations": MIN_REMAINING_ITERATIONS,
                    }));
                    Some(REVIEW_PROMPT)
                }
            },
        }
    }

    fn skip_reason(&self, remaining_iterations: usize) -> Option<SkipReason> {
        if !self.scope.minimal_loop {
            Some(SkipReason::PlanRunStep)
        } else if !self.scope.generic_profile {
            Some(SkipReason::ProfileNotGeneric)
        } else if self.scope.completion_contract {
            Some(SkipReason::CompletionContract)
        } else if !self.edit_observed {
            Some(SkipReason::NoSuccessfulEdit)
        } else if remaining_iterations < MIN_REMAINING_ITERATIONS {
            Some(SkipReason::InsufficientRemainingIterations)
        } else {
            None
        }
    }

    fn close(&mut self, reason: CloseReason, at: Option<(usize, usize)>) {
        self.phase = Phase::Finished;
        let observations = self.observations;
        self.emit(json!({
            "event": "completion_obligation_review",
            "phase": "closed",
            "reason": reason.as_str(),
            "prompt_version": PROMPT_VERSION,
            "triggered_iteration": self.triggered_iteration,
            "iteration": at.map(|(iteration, _)| iteration),
            "remaining_iterations": at.map(|(_, remaining)| remaining),
            "edits_after_prompt": observations.edits_after_prompt,
            "bash_results_after_prompt": observations.bash_results_after_prompt,
            "successful_bash_after_prompt": observations.successful_bash_after_prompt,
            "unsuccessful_bash_after_prompt": observations.unsuccessful_bash_after_prompt,
            "successful_bash_since_latest_edit": observations.successful_bash_since_latest_edit,
        }));
    }

    fn emit(&self, event: Value) {
        eval_events::emit(self.events_path.as_deref(), event);
    }
}

impl Drop for CompletionObligationReview {
    fn drop(&mut self) {
        if self.phase == Phase::Prompted {
            self.close(CloseReason::ScopeExitBeforeNextFinal, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};

    use super::{
        CompletionObligationReview, ENV_NAME, MIN_REMAINING_ITERATIONS, REVIEW_PROMPT, ReviewMode,
        RunScope, TEST_ENV, parse_mode,
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
    };

    fn read_events(path: &Path) -> Vec<Value> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect()
    }

    fn new_review(
        mode: ReviewMode,
        scope: RunScope,
    ) -> (tempfile::TempDir, CompletionObligationReview) {
        let dir = tempfile::tempdir().unwrap();
        let review =
            CompletionObligationReview::with_mode(mode, Some(&dir.path().join("e.jsonl")), scope);
        (dir, review)
    }

    #[test]
    fn mode_accepts_only_unset_empty_off_and_once() {
        for (raw, expected) in [
            (None, ReviewMode::Off),
            (Some(""), ReviewMode::Off),
            (Some("  "), ReviewMode::Off),
            (Some("off"), ReviewMode::Off),
            (Some(" off\n"), ReviewMode::Off),
            (Some("once"), ReviewMode::Once),
            (Some(" once "), ReviewMode::Once),
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
            "sk-test-940-NOT-A-KEY",
        ] {
            let error = parse_mode(Some(OsStr::new(value))).unwrap_err().to_string();
            // One fixed text for every value: nothing of the value is echoed.
            assert_eq!(
                error,
                format!("invalid {ENV_NAME}: expected unset, empty, `off`, or `once`")
            );
        }
        let secret = "sk-test-940-NOT-A-KEY";
        let error = parse_mode(Some(OsStr::new(secret)))
            .unwrap_err()
            .to_string();
        assert!(!error.contains(secret), "{error}");
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let raw = OsStr::from_bytes(&[0x6f, 0x6e, 0xff]);
            let error = parse_mode(Some(raw)).unwrap_err().to_string();
            assert!(error.contains(ENV_NAME), "{error}");
        }
    }

    #[test]
    fn off_never_prompts_observes_or_emits() {
        let (dir, mut review) = new_review(ReviewMode::Off, ELIGIBLE);
        review.note_successful_edit();
        review.note_bash_result(true);
        for iteration in 1..6 {
            assert_eq!(review.before_normal_final(iteration, 10), None);
        }
        drop(review);
        assert!(read_events(&dir.path().join("e.jsonl")).is_empty());
    }

    #[test]
    fn once_prompts_a_single_time_and_closes_at_the_next_final() {
        let (dir, mut review) = new_review(ReviewMode::Once, ELIGIBLE);
        review.note_successful_edit();
        assert_eq!(review.before_normal_final(2, 7), Some(REVIEW_PROMPT));
        assert_eq!(review.before_normal_final(3, 6), None);
        review.note_successful_edit();
        assert_eq!(review.before_normal_final(4, 5), None);
        drop(review);
        let events = read_events(&dir.path().join("e.jsonl"));
        let phases = events
            .iter()
            .map(|event| (event["phase"].clone(), event["reason"].clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            phases,
            [
                (
                    json!("triggered"),
                    json!("normal_final_after_successful_edit")
                ),
                (json!("closed"), json!("next_final")),
            ]
        );
        assert_eq!(events[1]["iteration"], 3);
        assert_eq!(events[1]["triggered_iteration"], 2);
    }

    #[test]
    fn remaining_iteration_boundary_and_scope_exclusions_skip_once() {
        let cases = [
            (
                ELIGIBLE,
                true,
                MIN_REMAINING_ITERATIONS - 1,
                Some("insufficient_remaining_iterations"),
            ),
            (ELIGIBLE, true, MIN_REMAINING_ITERATIONS, None),
            (ELIGIBLE, false, 9, Some("no_successful_edit")),
            (
                RunScope {
                    minimal_loop: false,
                    ..ELIGIBLE
                },
                true,
                9,
                Some("plan_run_step"),
            ),
            (
                RunScope {
                    generic_profile: false,
                    ..ELIGIBLE
                },
                true,
                9,
                Some("profile_not_generic"),
            ),
            (
                RunScope {
                    completion_contract: true,
                    ..ELIGIBLE
                },
                true,
                9,
                Some("completion_contract"),
            ),
        ];
        for (scope, edited, remaining, skipped) in cases {
            let (dir, mut review) = new_review(ReviewMode::Once, scope);
            if edited {
                review.note_successful_edit();
            }
            let prompt = review.before_normal_final(2, remaining);
            assert_eq!(prompt.is_some(), skipped.is_none(), "{scope:?} {remaining}");
            // A skipped review stays finished: a later final never prompts.
            if skipped.is_some() {
                assert_eq!(review.before_normal_final(3, 9), None);
            }
            drop(review);
            let events = read_events(&dir.path().join("e.jsonl"));
            match skipped {
                Some(reason) => {
                    assert_eq!(events.len(), 1, "{events:?}");
                    assert_eq!(events[0]["phase"], "skipped");
                    assert_eq!(events[0]["reason"], reason);
                    assert_eq!(events[0]["remaining_iterations"], remaining);
                }
                None => assert_eq!(events[0]["phase"], "triggered"),
            }
        }
    }

    #[test]
    fn a_later_edit_invalidates_older_successful_bash_observations() {
        let (dir, mut review) = new_review(ReviewMode::Once, ELIGIBLE);
        review.note_successful_edit();
        // Observations before the request do not count.
        review.note_bash_result(true);
        assert!(review.before_normal_final(2, 7).is_some());
        review.note_bash_result(true);
        review.note_bash_result(false);
        review.note_successful_edit();
        review.note_bash_result(false);
        assert_eq!(review.before_normal_final(6, 3), None);
        let closed = read_events(&dir.path().join("e.jsonl")).pop().unwrap();
        assert_eq!(closed["edits_after_prompt"], 1);
        assert_eq!(closed["bash_results_after_prompt"], 3);
        assert_eq!(closed["successful_bash_after_prompt"], 1);
        assert_eq!(closed["unsuccessful_bash_after_prompt"], 2);
        assert_eq!(closed["successful_bash_since_latest_edit"], 0);
    }

    #[test]
    fn scope_guard_records_an_exit_once_and_only_while_pending() {
        let (dir, mut review) = new_review(ReviewMode::Once, ELIGIBLE);
        review.note_successful_edit();
        assert!(review.before_normal_final(2, 7).is_some());
        review.note_bash_result(true);
        drop(review);
        let events = read_events(&dir.path().join("e.jsonl"));
        assert_eq!(events.len(), 2, "{events:?}");
        assert_eq!(events[1]["phase"], "closed");
        assert_eq!(events[1]["reason"], "scope_exit_before_next_final");
        assert_eq!(events[1]["iteration"], Value::Null);
        assert_eq!(events[1]["successful_bash_since_latest_edit"], 1);

        // Armed but never prompted, or already closed: the drop is silent.
        let (dir, mut review) = new_review(ReviewMode::Once, ELIGIBLE);
        review.note_successful_edit();
        drop(review);
        assert!(read_events(&dir.path().join("e.jsonl")).is_empty());
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

    fn reply(content: &str, tool_calls: Vec<ToolCall>) -> AssistantReply {
        AssistantReply {
            content: content.to_string(),
            tool_calls,
            prompt_tokens: None,
            completion_tokens: None,
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

    /// Runs one write-then-final script with the mode set for this thread.
    fn run_with_options(setting: Option<&str>, options: RunSessionOptions) -> (usize, Vec<Value>) {
        TEST_ENV.with(|value| *value.borrow_mut() = setting.map(OsString::from));
        let dir = tempfile::tempdir().unwrap();
        let mut client = Scripted {
            replies: Arc::new(Mutex::new(vec![
                reply(
                    "",
                    vec![ToolCall {
                        id: "call-0".to_string(),
                        name: "Write".to_string(),
                        arguments: json!({"path": "greeting.txt", "content": "hello\n"}),
                    }],
                ),
                reply("greeting.txt を作成しました。", Vec::new()),
                reply("Done.", Vec::new()),
            ])),
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        let result = run_session_with_outcome_with_options(
            &mut client,
            &mut SessionSnapshot::new(),
            "Write a short greeting file for the project and report what you did.",
            &[],
            &config(dir.path()),
            &NOOP_UI,
            options,
        );
        TEST_ENV.with(|value| *value.borrow_mut() = None);
        let outcome = result.unwrap();
        assert_eq!(outcome.final_text, "greeting.txt を作成しました。");
        let requests = client.requests.lock().unwrap().len();
        let review = read_events(&dir.path().join("events.jsonl"))
            .into_iter()
            .filter(|event| event["event"] == "completion_obligation_review")
            .collect();
        (requests, review)
    }

    #[test]
    fn plan_run_step_is_never_deferred() {
        let plan_step = || RunSessionOptions::plan_step(RunSessionStepKind::Unknown);
        let (off_requests, off_review) = run_with_options(None, plan_step());
        let (requests, review) = run_with_options(Some("once"), plan_step());
        assert_eq!(requests, off_requests);
        assert!(off_review.is_empty());
        assert_eq!(review.len(), 1, "{review:?}");
        assert_eq!(review[0]["phase"], "skipped");
        assert_eq!(review[0]["reason"], "plan_run_step");
    }
}
