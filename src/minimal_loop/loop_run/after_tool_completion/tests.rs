use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::super::{
    RunSessionOptions, RunSessionScope, RunSessionStepKind, RunStopReason,
    run_session_with_outcome_with_options,
};
use super::{
    after_tool_stop_allowed, looks_like_action_prompt, required_paths_satisfied_after_tool, stop,
};
use crate::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tools::registry::ToolSpec;
use crate::tui::NOOP_UI;

const STEP_KINDS: [RunSessionStepKind; 6] = [
    RunSessionStepKind::Inspect,
    RunSessionStepKind::Setup,
    RunSessionStepKind::Implement,
    RunSessionStepKind::Verify,
    RunSessionStepKind::Report,
    RunSessionStepKind::Unknown,
];

fn paths(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

/// The decision the loop takes after a tool batch: the paths are satisfied
/// and the stop is allowed.
fn ends_after_tool(
    root: &Path,
    options: &RunSessionOptions,
    required: &[String],
    initially_missing: &[String],
    progress_seen: bool,
) -> bool {
    required_paths_satisfied_after_tool(root, required, initially_missing, progress_seen)
        && after_tool_stop_allowed(options)
}

#[test]
fn direct_prompt_does_not_end_after_tool_for_preexisting_or_created_paths() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dates.py"), "broken\n").unwrap();
    std::fs::write(dir.path().join("created.md"), "new\n").unwrap();
    let direct = RunSessionOptions::default();
    assert_eq!(direct.scope, RunSessionScope::MinimalLoop);
    assert_eq!(direct.step_kind, None);
    assert!(!after_tool_stop_allowed(&direct));
    // (required, initially missing, progress seen, paths satisfied)
    let table: [(&[&str], &[&str], bool, bool); 8] = [
        // Every required path existed at the start (Issue #913).
        (&["dates.py"], &[], true, true),
        (&["dates.py"], &[], false, false),
        // A path missing at the start and present now, alone or next to a
        // path that already existed, with or without progress (Issue #921):
        // the paths are satisfied, but a direct prompt does not stop on that.
        (&["created.md"], &["created.md"], false, true),
        (&["created.md"], &["created.md"], true, true),
        (&["dates.py", "created.md"], &["created.md"], false, true),
        (&["dates.py", "created.md"], &["created.md"], true, true),
        // No required paths, or a path still missing: never satisfied.
        (&[], &[], true, false),
        (&["dates.py", "absent.py"], &["absent.py"], true, false),
    ];
    for (required, initially_missing, progress, satisfied) in table {
        let required = paths(required);
        let initially_missing = paths(initially_missing);
        assert_eq!(
            required_paths_satisfied_after_tool(
                dir.path(),
                &required,
                &initially_missing,
                progress
            ),
            satisfied,
            "{required:?} {initially_missing:?} progress={progress}"
        );
        assert!(
            !ends_after_tool(dir.path(), &direct, &required, &initially_missing, progress),
            "{required:?} {initially_missing:?} progress={progress}"
        );
    }
}

#[test]
fn plan_steps_keep_the_after_tool_stop_for_preexisting_paths() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dates.py"), "broken\n").unwrap();
    std::fs::write(dir.path().join("created.md"), "new\n").unwrap();
    let required = paths(&["dates.py"]);
    let created = paths(&["dates.py", "created.md"]);
    for kind in STEP_KINDS {
        for options in [
            RunSessionOptions::plan_step(kind),
            RunSessionOptions {
                scope: RunSessionScope::MinimalLoop,
                ..RunSessionOptions::plan_step(kind)
            },
            RunSessionOptions {
                step_kind: None,
                ..RunSessionOptions::plan_step(kind)
            },
        ] {
            assert!(
                ends_after_tool(dir.path(), &options, &required, &[], true),
                "{kind:?} {:?} {:?}",
                options.scope,
                options.step_kind
            );
            // A plan step that created a missing path keeps the stop too.
            assert!(
                ends_after_tool(
                    dir.path(),
                    &options,
                    &created,
                    &paths(&["created.md"]),
                    false
                ),
                "{kind:?} {:?} {:?}",
                options.scope,
                options.step_kind
            );
        }
    }
    let final_repair = RunSessionOptions::final_acceptance_repair();
    assert!(ends_after_tool(
        dir.path(),
        &final_repair,
        &required,
        &[],
        true
    ));
}

#[test]
fn stop_emits_the_unchanged_event_only_when_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let events = dir.path().join("events.jsonl");
    let required = paths(&["dates.py", "tests/test_dates.py"]);
    let direct = RunSessionOptions::default();
    // A direct prompt emits nothing, whether the paths existed or were created.
    assert!(stop(Some(&events), &direct, &required, &[]).is_none());
    assert!(stop(Some(&events), &direct, &required, &paths(&["dates.py"])).is_none());
    assert!(!events.exists());

    let implement = RunSessionOptions::plan_step(RunSessionStepKind::Implement);
    let outcome = stop(Some(&events), &implement, &required, &paths(&["dates.py"]))
        .unwrap()
        .into_outcome(paths(&["dates.py"]), 3, 2, 0, None, None);
    assert_eq!(
        outcome.final_text,
        "required artifacts satisfied: dates.py, tests/test_dates.py"
    );
    assert_eq!(
        outcome.stop_reason,
        RunStopReason::RequiredArtifactsSatisfiedAfterTool
    );
    assert_eq!(outcome.changed_paths, paths(&["dates.py"]));
    assert_eq!((outcome.iterations, outcome.tool_calls), (3, 2));
    let lines = std::fs::read_to_string(&events).unwrap();
    let values = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["event"], "loop_stop");
    assert_eq!(
        values[0]["reason"],
        "required_artifacts_satisfied_after_tool"
    );
    assert_eq!(values[0]["required_paths"], json!(required));

    assert!(stop(None, &implement, &required, &[]).is_some());
}

const REPAIR_REQUESTS: [&str; 14] = [
    "dates.py を直して",
    "dates.py を直してください",
    "dates.py を修正して",
    "dates.py のバグを直す",
    "fix dates.py",
    "repair dates.py",
    "dates.py を直してから説明して",
    "バグを直す",
    "dates.py を書き直して",
    "dates.py を作り直して",
    "dates.py の日付の順を直すこと。",
    "Repair dates.py.",
    "Please repair it.",
    "repair the parser",
];

const READ_ONLY_REQUESTS: [&str; 17] = [
    "直すのではなく、説明だけして",
    "dates.py は直してほしくない",
    "dates.py を直してもらう必要はない",
    "dates.py を直す方法を教えて",
    "dates.py の直すべき点を挙げて",
    "Please do not try to repair dates.py; only explain it.",
    "never attempt to repair it",
    "`repair_notes.md` を読んで説明して",
    "Explain correctness of dates.py.",
    "Explain unresolved items in dates.py.",
    "dates.py の見直し結果を説明して",
    "Do not repair dates.py; only explain it.",
    "dates.py の中身を説明して",
    "dates.py を見直して説明して",
    "dates.py は直すな。説明だけして",
    "Explain what \"repair\" means in dates.py.",
    "Read repair_notes.md and summarize it.",
];

#[test]
fn direct_prompt_counts_repair_requests_as_actions() {
    let direct = RunSessionOptions::default();
    for prompt in REPAIR_REQUESTS {
        assert!(looks_like_action_prompt(&direct, prompt), "{prompt}");
    }
    for prompt in READ_ONLY_REQUESTS {
        assert!(!looks_like_action_prompt(&direct, prompt), "{prompt}");
    }
}

#[test]
fn plan_steps_keep_the_shared_action_vocabulary() {
    // (prompt, action in every plan step)
    let table = [
        ("dates.py を直して", false),
        ("dates.py を直してください", false),
        ("dates.py のバグを直す", false),
        ("repair dates.py", false),
        ("dates.py を修正して", true),
        ("fix dates.py", true),
        ("create notes.md", true),
        ("dates.py の中身を説明して", false),
    ];
    for kind in STEP_KINDS {
        let options = RunSessionOptions::plan_step(kind);
        for (prompt, expected) in table {
            assert_eq!(
                looks_like_action_prompt(&options, prompt),
                expected,
                "{kind:?} {prompt}"
            );
        }
    }
}

#[derive(Clone)]
struct Scripted {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
}

impl Scripted {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies)),
        }
    }

    fn remaining(&self) -> usize {
        self.replies.lock().unwrap().len()
    }
}

impl ChatClient for Scripted {
    fn label(&self) -> &str {
        "scripted"
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        true
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }

    fn chat(
        &mut self,
        _model: &str,
        _messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn tool_reply(name: &str, arguments: Value) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: vec![ToolCall::new(name, arguments)],
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
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 6,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 30,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 1,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: false,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: false,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

#[test]
fn plan_implement_step_still_ends_after_a_successful_bash_on_existing_paths() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("dates.py"), "broken\n").unwrap();
    let mut client = Scripted::new(vec![
        tool_reply("Bash", json!({"command": "ls -la"})),
        AssistantReply::text("unused final"),
    ]);
    let mut session = SessionSnapshot::new();
    let outcome = run_session_with_outcome_with_options(
        &mut client,
        &mut session,
        "dates.py を直してください",
        &paths(&["dates.py"]),
        &config(dir.path()),
        &NOOP_UI,
        RunSessionOptions::plan_step(RunSessionStepKind::Implement),
    )
    .unwrap();

    assert_eq!(
        outcome.stop_reason,
        RunStopReason::RequiredArtifactsSatisfiedAfterTool
    );
    assert_eq!(outcome.final_text, "required artifacts satisfied: dates.py");
    assert_eq!(client.remaining(), 1, "the final reply is not consumed");
}

#[test]
fn plan_implement_step_still_ends_after_creating_a_missing_path() {
    let dir = tempfile::tempdir().unwrap();
    let mut client = Scripted::new(vec![
        tool_reply(
            "Write",
            json!({"path": "summary.md", "content": "# Summary\n"}),
        ),
        AssistantReply::text("unused final"),
    ]);
    let mut session = SessionSnapshot::new();
    let outcome = run_session_with_outcome_with_options(
        &mut client,
        &mut session,
        "create summary.md",
        &paths(&["summary.md"]),
        &config(dir.path()),
        &NOOP_UI,
        RunSessionOptions::plan_step(RunSessionStepKind::Implement),
    )
    .unwrap();

    assert_eq!(
        outcome.stop_reason,
        RunStopReason::RequiredArtifactsSatisfiedAfterTool
    );
    assert_eq!(
        outcome.final_text,
        "required artifacts satisfied: summary.md"
    );
    assert_eq!(outcome.changed_paths, paths(&["summary.md"]));
    assert_eq!(client.remaining(), 1, "the final reply is not consumed");
    let events = std::fs::read_to_string(dir.path().join("events.jsonl")).unwrap();
    let stops = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|event| event["event"] == "loop_stop")
        .collect::<Vec<_>>();
    assert_eq!(stops.len(), 1, "{stops:#?}");
    assert_eq!(
        stops[0]["reason"],
        "required_artifacts_satisfied_after_tool"
    );
    assert_eq!(stops[0]["required_paths"], json!(["summary.md"]));
}
