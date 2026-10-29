//! Issue #895: the last Verify/Report runtime Bash refusal before the loop
//! stops leads with a concrete next call, while the verdict, the repeat limit
//! and the stop stay exactly as before.
//!
//! The commands are recorded excerpts or synthetic shapes; see
//! `tests/corpus/apps/verify-refusal-repeat-guidance`.
use std::sync::{Arc, Mutex};

use clap::Parser;
use serde_json::{Value, json};

use super::{base_is_accepted, recoverable_tool_feedback, recoverable_tool_feedback_for_repeat};
use crate::cli::Cli;
use crate::config::Config;
use crate::minimal_loop::loop_run::{
    RunSessionOptions, RunSessionStepKind, run_session_with_outcome_with_options,
};
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID;
use crate::providers::xml_fallback::extract_tool_calls;
use crate::providers::{AssistantReply, ChatClient};
use crate::state::tool_history::history_content;
use crate::state::{ConversationMessage, SessionSnapshot, ToolCall};
use crate::tui::NOOP_UI;

/// L-135 ambig-g, recorded verbatim.
const AMBIG_G: &str = "grep -q \"addEventListener\" script.js && echo \"pass\" || echo \"fail\"";
const AMBIG_G_CALL: &str = "Repeated rejection (2/2). Next call: Bash {\"command\":\"grep -q \\\"addEventListener\\\" script.js\"}. Run it alone; a third refusal stops this step.";
/// L-135 cli-g, recorded verbatim.
const CLI_G: &str = "python3 -c \"import os; open('test.csv', 'w').write('a,b\\n1,2\\n3,4'); os.system('python3 src/app/main.py test.csv > out.txt'); os.remove('test.csv');\"\ntest -s out.txt";
const REPEAT_LEAD: &str = "Repeated rejection (2/2).";
const NAMED_CALL: &str = "Next call: Bash {";
const WRITE_CALL: &str =
    "Next calls: Write {\"path\":\"PATH\",\"content\":\"CONTENT\"} for each input file";
const POLICY_ERR: &str = "verify_command_policy_error: success_failure_echo_stripped: verifier exit status already records pass/fail; trailing echo branches mask the base command";

#[derive(Clone)]
struct Recording {
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
    native: bool,
}

impl Recording {
    /// One reply per batch; every call of a batch is sent in the same reply.
    fn new(batches: &[Vec<(&str, Value)>], native: bool) -> Self {
        let mut replies = batches
            .iter()
            .map(|batch| {
                if native {
                    AssistantReply {
                        content: String::new(),
                        tool_calls: batch
                            .iter()
                            .map(|(name, arguments)| ToolCall::new(*name, arguments.clone()))
                            .collect(),
                        prompt_tokens: None,
                        completion_tokens: None,
                    }
                } else {
                    AssistantReply::text(
                        batch
                            .iter()
                            .map(|(name, arguments)| {
                                format!(
                                    "<anvil_tool_call name=\"{name}\">{arguments}</anvil_tool_call>"
                                )
                            })
                            .collect::<String>(),
                    )
                }
            })
            .collect::<Vec<_>>();
        replies.push(AssistantReply::text("Done."));
        replies.reverse();
        Self {
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::new(Mutex::new(Vec::new())),
            native,
        }
    }
}

impl ChatClient for Recording {
    fn label(&self) -> &str {
        "recording"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }
    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[crate::tools::registry::ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        self.requests.lock().unwrap().push(messages.to_vec());
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop()
            .ok_or_else(|| anyhow::anyhow!("the recording client ran out of replies"))?;
        if self.native {
            return Ok(reply);
        }
        // A text-protocol provider parses its own XML reply, as Ollama does.
        let allowed = ["Bash".to_string(), "Write".to_string()];
        let (tool_calls, content) = extract_tool_calls(&reply.content, &allowed)?;
        Ok(AssistantReply {
            content,
            tool_calls,
            ..reply
        })
    }
}

struct Run {
    root: tempfile::TempDir,
    error: Option<String>,
    bash_results: Vec<String>,
    requests: Vec<Vec<ConversationMessage>>,
    events: Vec<Value>,
}

impl Run {
    fn named(&self, event: &str) -> Vec<&Value> {
        self.events
            .iter()
            .filter(|value| value["event"] == event)
            .collect()
    }

    fn queued(&self) -> Vec<(u64, String)> {
        self.named("verify_refusal_feedback_queued")
            .into_iter()
            .map(|event| {
                (
                    event["repeat_count"].as_u64().unwrap(),
                    event["guidance_kind"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn queued_text(&self, index: usize) -> String {
        self.named("verify_refusal_feedback_queued")[index]["queued_feedback"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// Every message content of the request sent after the `n`-th reply.
    fn request_text(&self, n: usize) -> String {
        self.requests[n]
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn policy_repeats(&self) -> Vec<u64> {
        self.named("tool_policy_error")
            .into_iter()
            .map(|event| event["repeat_count"].as_u64().unwrap())
            .collect()
    }
}

fn bash(command: &str) -> (&'static str, Value) {
    ("Bash", json!({ "command": command }))
}

fn run_calls(step: RunSessionStepKind, calls: &[(&str, Value)], native: bool) -> Run {
    let batches = calls
        .iter()
        .map(|call| vec![call.clone()])
        .collect::<Vec<_>>();
    run_batches(step, &batches, native)
}

fn run_batches(step: RunSessionStepKind, batches: &[Vec<(&str, Value)>], native: bool) -> Run {
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::from_cli(Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
    config.workspace_root = root.path().to_path_buf();
    config.profile = NEXTJS_PROFILE_ID.to_string();
    config.yes = true;
    config.max_iterations = 40;
    config.eval_events_path = Some(root.path().join(".anvil/runs/test/events.jsonl"));
    let mut client = Recording::new(batches, native);
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
    let events = std::fs::read_to_string(root.path().join(".anvil/runs/test/events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let requests = client.requests.lock().unwrap().clone();
    Run {
        error: result.err().map(|err| err.to_string()),
        bash_results: session
            .messages
            .iter()
            .filter(|message| message.role == "tool" && message.name.as_deref() == Some("Bash"))
            .map(|message| message.content.clone())
            .collect(),
        requests,
        events,
        root,
    }
}

fn run(commands: &[&str]) -> Run {
    let calls = commands
        .iter()
        .map(|command| bash(command))
        .collect::<Vec<_>>();
    run_calls(RunSessionStepKind::Verify, &calls, true)
}

fn assert_stopped_after_two_results(run: &Run) {
    assert_eq!(
        run.error.as_deref(),
        Some("recoverable tool error repeated: verify_command_policy_error"),
        "{:?}",
        run.bash_results
    );
    assert_eq!(run.policy_repeats(), vec![1, 2, 3]);
    assert_eq!(run.bash_results.len(), 2, "{:?}", run.bash_results);
    let stops = run.named("loop_stop");
    assert_eq!(stops.len(), 1);
    assert_eq!(stops[0]["reason"], "recoverable_tool_error_repeated");
    assert_eq!(stops[0]["repeat_count"], 2);
    assert!(run.named("verify_command_substituted").is_empty());
}

#[test]
fn only_the_second_refusal_leads_with_the_complete_base_call_and_the_next_request_carries_it() {
    let base = "grep -q \"addEventListener\" script.js";
    let run = run(&[AMBIG_G, AMBIG_G, base]);
    assert_eq!(run.error, None);
    assert_eq!(run.policy_repeats(), vec![1, 2]);
    assert_eq!(
        run.queued(),
        vec![(1, "base_command".into()), (2, "repeat_base_call".into())]
    );
    let (first, second) = (&run.bash_results[0], &run.bash_results[1]);
    // The first refusal is byte for byte what Issue #889 sends; the history
    // bounds both before they are stored.
    let err = anyhow::anyhow!("{POLICY_ERR}");
    let raw_first = recoverable_tool_feedback("Bash", &err, AMBIG_G, run.root.path());
    assert_eq!(first, &history_content(Some("Bash"), &raw_first));
    assert!(!first.contains(REPEAT_LEAD), "{first}");
    // Only the second one changes, and only by its lead.
    let raw_second =
        recoverable_tool_feedback_for_repeat("Bash", &err, AMBIG_G, run.root.path(), 2, None);
    assert_eq!(raw_second, format!("{AMBIG_G_CALL} {raw_first}"));
    assert_eq!(second, &history_content(Some("Bash"), &raw_second));
    // What the client received in the next request holds the whole call.
    assert!(!run.request_text(1).contains(REPEAT_LEAD));
    let sent = run.requests[2]
        .iter()
        .rev()
        .find(|message| message.role == "tool")
        .unwrap()
        .content
        .clone();
    assert!(sent.starts_with(AMBIG_G_CALL), "{sent}");
    assert_eq!(sent, run.queued_text(1));
    assert_eq!(history_content(Some("Bash"), second), sent);
    // The base then runs as-is and its failure (no script.js) stays a failure.
    let third = &run.bash_results[2];
    assert!(!third.contains("rejected"), "{third}");
    assert!(third.contains("status: exit status: "), "{third}");
    assert!(!third.contains("status: exit status: 0"), "{third}");
}

#[test]
fn the_same_refusal_every_time_still_stops_on_the_third_with_two_tool_results() {
    // L-129 tool-g, recorded: only the echoed words change.
    let commands = [
        "grep -q \"from './types'\" src/lib/storage.ts && echo \"Verification successful\" || echo \"Verification failed\"",
        "grep -q \"from './types'\" src/lib/storage.ts && echo \"Pass\" || echo \"Fail\"",
        "grep -q \"from './types'\" src/lib/storage.ts && echo \"Verification passed\" || echo \"Verification failed\"",
    ];
    let run = run(&commands);
    assert_stopped_after_two_results(&run);
    assert_eq!(
        run.queued(),
        vec![(1, "base_command".into()), (2, "repeat_base_call".into())]
    );
    assert!(!run.bash_results[0].contains(REPEAT_LEAD));
    assert!(run.bash_results[1].starts_with(
        "Repeated rejection (2/2). Next call: Bash {\"command\":\"grep -q \\\"from './types'\\\" src/lib/storage.ts\"}. Run it alone; a third refusal stops this step. Run the base command"
    ));
    // The lead's own limit agrees with the loop's: the third refusal stops
    // before any feedback, so no third text is queued.
    assert_eq!(run.requests.len(), 3);
}

#[test]
fn the_refused_body_never_runs_and_a_failing_verifier_after_write_stays_failing() {
    let marker = "python3 -c \"open('ran.txt', 'w').write('x')\"";
    let run = run(&[marker, marker, marker]);
    assert_stopped_after_two_results(&run);
    assert!(!run.root.path().join("ran.txt").exists());
    assert_eq!(
        run.queued(),
        vec![(1, "none".into()), (2, "repeat_write_then_verifier".into())]
    );

    // A Write ends the step's tool observation, so the Write and the verifier
    // come in one reply, as a model following the lead would send them.
    let batches = [
        vec![bash(CLI_G)],
        vec![bash(CLI_G)],
        vec![
            (
                "Write",
                json!({ "path": "input.csv", "content": "a,b\n1,2\n" }),
            ),
            bash("test -s out.txt"),
        ],
    ];
    let run = run_batches(RunSessionStepKind::Verify, &batches, true);
    assert_eq!(run.error, None);
    assert_eq!(
        run.queued(),
        vec![(1, "none".into()), (2, "repeat_write_then_verifier".into())]
    );
    let second = &run.bash_results[1];
    assert!(second.starts_with(REPEAT_LEAD), "{second}");
    assert!(second.contains(WRITE_CALL), "{second}");
    assert!(history_content(Some("Bash"), second).contains(WRITE_CALL));
    // The lead takes nothing from the refused body and offers no weaker check.
    let lead = &second[..second.find("Tool call").unwrap()];
    for absent in ["a,b", "test.csv", "out.txt", "test -f", "compileall"] {
        assert!(!lead.contains(absent), "{absent}: {lead}");
    }
    assert!(!run.root.path().join("test.csv").exists());
    assert!(!run.root.path().join("out.txt").exists());
    let verifier = &run.bash_results[2];
    assert!(!verifier.contains("rejected"), "{verifier}");
    assert!(!verifier.contains("status: exit status: 0"), "{verifier}");
}

#[test]
fn redirect_shape_leads_with_write_on_the_second_refusal() {
    let command = "echo \"name,score\" > test.csv";
    let run = run(&[command, command, command]);
    assert_stopped_after_two_results(&run);
    assert_eq!(
        run.queued(),
        vec![
            (1, "write_tool".into()),
            (2, "repeat_write_then_verifier".into())
        ]
    );
    assert!(run.bash_results[1].starts_with(REPEAT_LEAD));
    assert!(run.bash_results[1].contains(WRITE_CALL));
    assert!(!run.bash_results[1][..200].contains("name,score"));
}

/// `grep -q -e token0 … -e token<count - 1> notes.txt`.
fn grep_base(count: usize) -> String {
    format!(
        "grep -q {} notes.txt",
        (0..count)
            .map(|n| format!("-e token{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

#[test]
fn a_base_that_cannot_be_named_whole_gets_no_command_to_run() {
    let long_base = grep_base(450);
    // Longer than the 4,000-byte Bash history budget (Issue #916).
    assert!(long_base.len() > 4_000);
    // The long base itself is admitted; only its length keeps it unnamed.
    let root = tempfile::tempdir().unwrap();
    assert!(base_is_accepted(&long_base, root.path()));
    let long = format!("{long_base} && echo ok || echo ng");
    for command in [
        "node -e \"process.exit(0)\" && echo ok || echo ng",
        "grep -q \"foo  bar\" f && echo ok || echo ng",
        long.as_str(),
    ] {
        let run = run(&[command, command, command]);
        assert_stopped_after_two_results(&run);
        assert_eq!(run.queued()[1].1, "repeat_verifier_alone", "{command}");
        for text in [&run.bash_results[1], &run.queued_text(1)] {
            assert!(text.starts_with(REPEAT_LEAD), "{text}");
            assert!(!text.contains(NAMED_CALL), "{text}");
        }
    }
}

#[test]
fn a_base_past_500_characters_within_the_bash_budget_is_named_whole() {
    // Issue #916: a base the old 500-character history cut is now kept whole,
    // so the lead names it.
    let base = grep_base(70);
    assert!(base.len() > 500);
    let root = tempfile::tempdir().unwrap();
    assert!(base_is_accepted(&base, root.path()));
    let command = format!("{base} && echo ok || echo ng");
    let command = command.as_str();
    let run = run(&[command, command, command]);
    assert_stopped_after_two_results(&run);
    assert_eq!(run.queued()[1].1, "repeat_base_call");
    let call = serde_json::to_string(&json!({ "command": base })).unwrap();
    let lead = format!("{REPEAT_LEAD} Next call: Bash {call}.");
    for text in [&run.bash_results[1], &run.queued_text(1)] {
        assert!(text.starts_with(&lead), "{text}");
        assert!(text.len() <= 4_000, "{text}");
    }
}

#[test]
fn xml_fallback_requests_carry_the_same_second_lead() {
    let calls = [bash(AMBIG_G), bash(AMBIG_G), bash(AMBIG_G)];
    let run = run_calls(RunSessionStepKind::Verify, &calls, false);
    assert!(run.request_text(0).contains("<anvil_tool_call"));
    assert_stopped_after_two_results(&run);
    assert!(!run.request_text(1).contains(REPEAT_LEAD));
    assert!(run.request_text(2).contains(AMBIG_G_CALL));
}

#[test]
fn report_step_second_refusal_also_leads_with_the_base_call() {
    let calls = [bash(AMBIG_G), bash(AMBIG_G), bash(AMBIG_G)];
    let run = run_calls(RunSessionStepKind::Report, &calls, true);
    assert_stopped_after_two_results(&run);
    assert!(run.bash_results[1].starts_with(AMBIG_G_CALL));
}

#[test]
fn the_queued_event_holds_only_the_scrubbed_bounded_text() {
    let command = "grep -q api_key=abc123 config.txt && echo ok || echo ng";
    let run = run(&[command, command, command]);
    assert_stopped_after_two_results(&run);
    for index in 0..2 {
        let queued = run.queued_text(index);
        assert!(!queued.contains("api_key=abc123"), "{queued}");
        // The field copies what the model is sent, so it follows the Bash
        // history budget (Issue #916).
        assert!(queued.len() <= 4_000, "{queued}");
    }
    // A scrubbed command cannot be shown verbatim, so none is offered.
    assert_eq!(run.queued()[1].1, "repeat_verifier_alone");
    assert!(!run.request_text(2).contains("api_key=abc123"));
    let event = run.named("verify_refusal_feedback_queued")[1];
    let mut keys = event
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.retain(|key| key != "schema_version" && !key.starts_with("ts"));
    for key in [
        "event",
        "name",
        "repeat_count",
        "guidance_kind",
        "queued_feedback",
    ] {
        assert!(keys.iter().any(|k| k == key), "{key}: {keys:?}");
    }
}

#[test]
fn other_errors_and_other_repeats_keep_the_first_text_and_queue_nothing() {
    let root = tempfile::tempdir().unwrap();
    let events = root.path().join("events.jsonl");
    let err = anyhow::anyhow!("tool_args_invalid: missing command");
    assert_eq!(
        recoverable_tool_feedback_for_repeat("Bash", &err, AMBIG_G, root.path(), 2, Some(&events)),
        recoverable_tool_feedback("Bash", &err, AMBIG_G, root.path())
    );
    assert!(!events.exists());
    let err = anyhow::anyhow!("{POLICY_ERR}");
    let first = recoverable_tool_feedback("Bash", &err, AMBIG_G, root.path());
    for repeats in [0, 1, 3] {
        assert_eq!(
            recoverable_tool_feedback_for_repeat("Bash", &err, AMBIG_G, root.path(), repeats, None),
            first
        );
    }
    let err = anyhow::anyhow!("verify_command_policy_error: verify command starts a dev server");
    assert_eq!(
        recoverable_tool_feedback_for_repeat("Bash", &err, "npm run dev", root.path(), 2, None),
        recoverable_tool_feedback("Bash", &err, "npm run dev", root.path())
    );
}

#[test]
fn corpus_sequences_match_the_second_refusal_guidance() {
    let text = std::fs::read_to_string(
        "tests/corpus/apps/verify-refusal-repeat-guidance/fixtures/cases.json",
    )
    .unwrap();
    let corpus: Value = serde_json::from_str(&text).unwrap();
    let mut checked = 0;
    for case in corpus["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let commands = case["commands"]
            .as_array()
            .unwrap()
            .iter()
            .map(|command| command.as_str().unwrap())
            .collect::<Vec<_>>();
        let run = run(&commands);
        assert_stopped_after_two_results(&run);
        let kinds = run
            .queued()
            .into_iter()
            .map(|(_, kind)| kind)
            .collect::<Vec<_>>();
        let expected = case["guidance_kinds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|kind| kind.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(kinds, expected, "{id}");
        let second = &run.bash_results[1];
        let lead = case["second_lead"].as_str().unwrap();
        assert!(second.starts_with(lead), "{id}: {second}");
        assert!(run.request_text(2).contains(lead), "{id}");
        assert!(!run.bash_results[0].contains(REPEAT_LEAD), "{id}");
        checked += 1;
    }
    assert_eq!(checked, 6);
}
