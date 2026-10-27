//! Issue #889: a Verify/Report runtime Bash refusal names the acceptable base
//! command (or the Write tool) while the accept/reject decision, the repeat
//! limit and the repeat key stay exactly as before.
//!
//! The command shapes are synthetic, derived from the L-129 tool-g / ambig-g and
//! L-127 cli-g runs; see `tests/corpus/apps/verify-refusal-base-command-hint`.
use std::path::Path;
use std::sync::{Arc, Mutex};

use clap::Parser;
use serde_json::{Value, json};

use super::recoverable_tool_feedback;
use crate::cli::Cli;
use crate::config::Config;
use crate::minimal_loop::loop_run::{
    RunSessionOptions, RunSessionStepKind, run_session_with_outcome_with_options,
};
use crate::minimal_loop::tool_feedback;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tui::NOOP_UI;

const TOOL_G: &str = "grep -q \"from './types'\" src/lib/storage.ts && echo \"ok\" || echo \"ng\"";
const TOOL_G_BASE: &str = "grep -q \"from './types'\" src/lib/storage.ts";
const AMBIG_G: &str = "grep -q 'style.css' index.html && echo \"pass\" || echo \"fail\"";
const AMBIG_G_BASE: &str = "grep -q 'style.css' index.html";
const CLI_G: &str = "echo \"name,score\" > test.csv";
const WRITE_HINT: &str = "Create fixture files with the Write tool";

#[derive(Clone)]
struct Scripted {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
}

impl Scripted {
    fn calling(commands: &[String]) -> Self {
        let mut replies = commands
            .iter()
            .map(|command| AssistantReply {
                content: String::new(),
                tool_calls: vec![ToolCall::new("Bash", json!({ "command": command }))],
                prompt_tokens: None,
                completion_tokens: None,
            })
            .collect::<Vec<_>>();
        replies.push(AssistantReply::text("Done."));
        replies.reverse();
        Self {
            replies: Arc::new(Mutex::new(replies)),
        }
    }
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
        _messages: &[ConversationMessage],
        _tools: &[crate::tools::registry::ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.replies
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("the scripted client ran out of replies"))
    }
}

struct Run {
    error: Option<String>,
    bash_results: Vec<String>,
    events: Vec<Value>,
}

fn run(step: RunSessionStepKind, commands: &[String]) -> Run {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.path().to_path_buf();
    config.profile = NEXTJS_PROFILE_ID.to_string();
    config.yes = true;
    config.max_iterations = 40;
    config.eval_events_path = Some(root.path().join(".anvil/runs/test/events.jsonl"));
    let mut client = Scripted::calling(commands);
    let mut session = SessionSnapshot::new();
    let result = run_session_with_outcome_with_options(
        &mut client,
        &mut session,
        "Verify the current step.",
        &[],
        &config,
        &NOOP_UI,
        RunSessionOptions::plan_step(step),
    );
    Run {
        error: result.err().map(|err| err.to_string()),
        bash_results: session
            .messages
            .iter()
            .filter(|message| message.role == "tool" && message.name.as_deref() == Some("Bash"))
            .map(|message| message.content.clone())
            .collect(),
        events: events(root.path()),
    }
}

fn events(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".anvil/runs/test/events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn first_refusal(command: &str) -> String {
    let run = run(RunSessionStepKind::Verify, &[command.to_string()]);
    assert_eq!(run.error, None, "{command}");
    assert_eq!(
        run.bash_results.len(),
        1,
        "{command}: {:?}",
        run.bash_results
    );
    let policy_errors = run
        .events
        .iter()
        .filter(|event| event["event"] == "tool_policy_error")
        .count();
    assert_eq!(policy_errors, 1, "{command} must still be refused");
    run.bash_results[0].clone()
}

fn named_base(refusal: &str, base: &str) -> bool {
    refusal.contains(&format!("Run the base command `{base}` as-is, once"))
}

#[test]
fn tool_g_shape_is_refused_and_names_the_base_command() {
    let refusal = first_refusal(TOOL_G);
    assert!(
        refusal.contains("success_failure_echo_stripped"),
        "{refusal}"
    );
    assert!(named_base(&refusal, TOOL_G_BASE), "{refusal}");
}

#[test]
fn ambig_g_shape_is_refused_and_names_the_base_command() {
    let refusal = first_refusal(AMBIG_G);
    assert!(
        refusal.contains("success_failure_echo_stripped"),
        "{refusal}"
    );
    assert!(named_base(&refusal, AMBIG_G_BASE), "{refusal}");
}

#[test]
fn fallback_true_and_exit_code_echo_shapes_name_the_base_command() {
    let refusal = first_refusal("grep -q 'style.css' index.html || true");
    assert!(named_base(&refusal, AMBIG_G_BASE), "{refusal}");
    let refusal = first_refusal("grep -q 'style.css' index.html; echo \"EXIT_CODE=$?\"");
    assert!(named_base(&refusal, AMBIG_G_BASE), "{refusal}");
}

#[test]
fn redirect_shape_is_refused_and_points_to_the_write_tool() {
    let refusal = first_refusal(CLI_G);
    assert!(refusal.contains(WRITE_HINT), "{refusal}");
    assert!(!refusal.contains("Run the base command"), "{refusal}");
}

#[test]
fn a_base_that_is_itself_refused_is_not_named() {
    let command = "node -e \"process.exit(0)\" && echo ok";
    let refusal = first_refusal(command);
    assert!(!refusal.contains("Run the base command"), "{refusal}");
    assert!(!refusal.contains(WRITE_HINT), "{refusal}");
    let nested = "node -e \"process.exit(0)\" && echo \"ok\" || echo \"ng\"";
    let refusal = first_refusal(nested);
    assert!(!refusal.contains("Run the base command"), "{refusal}");
}

#[test]
fn report_step_refusal_also_names_the_base_command() {
    let run = run(RunSessionStepKind::Report, &[TOOL_G.to_string()]);
    assert_eq!(run.error, None);
    assert!(
        run.events
            .iter()
            .any(|event| event["event"] == "tool_policy_error"),
        "a Report step must still refuse the echo shape"
    );
    assert!(
        named_base(&run.bash_results[0], TOOL_G_BASE),
        "{:?}",
        run.bash_results
    );
}

#[test]
fn other_refusals_and_accepted_commands_keep_their_exact_feedback() {
    let root = tempfile::tempdir().unwrap();
    for err_text in [
        "verify_command_policy_error: verify command starts a dev server",
        "verify_command_policy_error: Verify step runtime Bash must not update the workspace",
        "tool_args_invalid: missing command",
    ] {
        let err = anyhow::anyhow!("{err_text}");
        for command in ["npm run dev", "cargo test", "test -f package.json"] {
            assert_eq!(
                recoverable_tool_feedback("Bash", &err, command, root.path()),
                tool_feedback::recoverable_tool_feedback("Bash", &err, None),
                "{err_text} / {command}"
            );
        }
    }
    // A non-policy error never gains a hint, even for an echo-shaped command.
    let err = anyhow::anyhow!("tool_args_invalid: missing command");
    assert_eq!(
        recoverable_tool_feedback("Bash", &err, TOOL_G, root.path()),
        tool_feedback::recoverable_tool_feedback("Bash", &err, None)
    );
    // An accepted command is not refused, so no hint can reach it.
    let run = run(
        RunSessionStepKind::Verify,
        &["test -f package.json".to_string()],
    );
    assert!(
        run.bash_results
            .iter()
            .all(|result| !result.contains("Run the base command")),
        "{:?}",
        run.bash_results
    );
    assert!(
        run.events
            .iter()
            .all(|event| event["event"] != "tool_policy_error")
    );
}

#[test]
fn repeated_g_shape_keeps_the_verdict_and_limit_and_only_the_text_gains_the_base() {
    let run = run(RunSessionStepKind::Verify, &vec![TOOL_G.to_string(); 3]);
    assert_eq!(
        run.error.as_deref(),
        Some("recoverable tool error repeated: verify_command_policy_error")
    );
    let policy_repeats = run
        .events
        .iter()
        .filter(|event| event["event"] == "tool_policy_error")
        .map(|event| {
            (
                event["policy_error_kind"].as_str().unwrap().to_string(),
                event["verify_command_violation_kind"]
                    .as_str()
                    .unwrap()
                    .to_string(),
                event["repeat_count"].as_u64().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let expected = (1..=3)
        .map(|n| {
            (
                "verify_command_policy_error".to_string(),
                "success_failure_echo_stripped".to_string(),
                n,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(policy_repeats, expected);
    assert_eq!(run.bash_results.len(), 2, "{:?}", run.bash_results);
    for result in &run.bash_results {
        assert!(named_base(result, TOOL_G_BASE), "{result}");
    }
    // The pre-change sentence (what `tool_feedback` alone builds) never carried it.
    let err = anyhow::anyhow!(
        "verify_command_policy_error: success_failure_echo_stripped: verifier exit status already records pass/fail; trailing echo branches mask the base command"
    );
    let before = tool_feedback::recoverable_tool_feedback("Bash", &err, None);
    assert!(!before.contains(TOOL_G_BASE), "{before}");
}

#[test]
fn corpus_cases_match_the_refusal_sentences() {
    let cases = std::fs::read_to_string(
        "tests/corpus/apps/verify-refusal-base-command-hint/fixtures/cases.txt",
    )
    .unwrap();
    let mut checked = 0;
    for line in cases.lines().filter(|line| !line.starts_with('#')) {
        let parts = line.split(" | ").collect::<Vec<_>>();
        assert_eq!(parts.len(), 3, "{line}");
        let refusal = first_refusal(parts[1]);
        if parts[2] == "(no base named)" {
            assert!(
                !refusal.contains("Run the base command"),
                "{line}: {refusal}"
            );
        } else {
            assert!(refusal.contains(parts[2]), "{line}: {refusal}");
        }
        checked += 1;
    }
    assert_eq!(checked, 6);
}
