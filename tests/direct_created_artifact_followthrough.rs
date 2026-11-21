//! Issue #921: a direct prompt whose tool batch creates a missing required
//! path no longer ends with `required_artifacts_satisfied_after_tool`. The
//! loop sends the next provider request, so the model can run what it wrote,
//! and the model's final answer passes the existing gates. A leading `ls`, a
//! Bash `touch`, or a file that merely exists is not evidence of completion,
//! and the gates that blocked a run before still block it. Every scenario runs
//! with native tool calls and with the XML text protocol.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider, ToolProtocol,
};
use commandagent::minimal_loop::loop_run::{
    RunSessionOutcome, RunStopReason, run_session_with_outcome_with_ui,
};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::registry::ToolSpec;
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/direct-created-artifact-followthrough";
const AFTER_TOOL: &str = "required_artifacts_satisfied_after_tool";
const MISSING_TOOL_CALL: &str = "missing tool call for action prompt after feedback";
const SCRIPT: &str = "print(\"hello-from-script\")\n";
const NOTES: &str = "The release is planned for spring.\n";
const PROTOCOLS: [Protocol; 2] = [Protocol::Native, Protocol::Text];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Protocol {
    Native,
    Text,
}

/// One scripted step: tool calls, or a final text when `calls` is empty.
#[derive(Clone)]
struct Step {
    calls: Vec<(&'static str, Value)>,
    text: &'static str,
}

fn tools(calls: Vec<(&'static str, Value)>) -> Step {
    Step { calls, text: "" }
}

fn tool(name: &'static str, arguments: Value) -> Step {
    tools(vec![(name, arguments)])
}

fn bash(command: &str) -> Step {
    tool("Bash", json!({"command": command}))
}

fn write(path: &str, content: &str) -> Step {
    tool("Write", json!({"path": path, "content": content}))
}

fn final_text(text: &'static str) -> Step {
    Step {
        calls: Vec::new(),
        text,
    }
}

fn reply(protocol: Protocol, step: &Step) -> AssistantReply {
    if step.calls.is_empty() {
        return AssistantReply::text(step.text);
    }
    match protocol {
        Protocol::Native => AssistantReply {
            content: String::new(),
            tool_calls: step
                .calls
                .iter()
                .map(|(name, arguments)| ToolCall::new(*name, arguments.clone()))
                .collect(),
            prompt_tokens: None,
            completion_tokens: None,
        },
        Protocol::Text => AssistantReply::text(
            step.calls
                .iter()
                .map(|(name, arguments)| {
                    format!(r#"<anvil_tool_call name="{name}">{arguments}</anvil_tool_call>"#)
                })
                .collect::<String>(),
        ),
    }
}

#[derive(Clone)]
struct RecordingClient {
    native: bool,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Vec<ConversationMessage>>>>,
}

impl ChatClient for RecordingClient {
    fn label(&self) -> &str {
        "recording"
    }

    fn supports_native_tools(&self, _model: &str) -> bool {
        self.native
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
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

fn config(root: &Path, protocol: Protocol, max_iterations: usize) -> Config {
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
        tool_protocol: (protocol == Protocol::Text).then_some(ToolProtocol::Text),
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
        max_iterations,
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

struct Run {
    protocol: Protocol,
    dir: tempfile::TempDir,
    result: Result<RunSessionOutcome, String>,
    requests: Vec<Vec<ConversationMessage>>,
    remaining_replies: usize,
}

/// Runs a direct prompt (the minimal loop outside any plan step) in a fresh
/// workspace that holds `notes.txt`.
fn run_direct(protocol: Protocol, prompt: &str, steps: &[Step], max_iterations: usize) -> Run {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.txt"), NOTES).unwrap();
    let mut client = RecordingClient {
        native: protocol == Protocol::Native,
        replies: Arc::new(Mutex::new(
            steps.iter().map(|step| reply(protocol, step)).collect(),
        )),
        requests: Arc::new(Mutex::new(Vec::new())),
    };
    let mut session = SessionSnapshot::new();
    let result = run_session_with_outcome_with_ui(
        &mut client,
        &mut session,
        prompt,
        &[],
        &config(dir.path(), protocol, max_iterations),
        &NOOP_UI,
    )
    .map_err(|err| format!("{err:#}"));
    let requests = client.requests.lock().unwrap().clone();
    let remaining_replies = client.replies.lock().unwrap().len();
    Run {
        protocol,
        dir,
        result,
        requests,
        remaining_replies,
    }
}

impl Run {
    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.dir.path().join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect()
    }

    fn named(&self, name: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|event| event["event"] == name)
            .collect()
    }

    fn after_tool_stops(&self) -> usize {
        self.named("loop_stop")
            .iter()
            .filter(|event| event["reason"] == AFTER_TOOL)
            .count()
    }

    /// The direct scope and the paths the run treated as required.
    fn assert_direct_scope(&self, required: Value, initially_missing: Value) {
        let scopes = self.named("step_obligation_scope");
        assert_eq!(scopes.len(), 1, "{:?}: {scopes:#?}", self.protocol);
        assert_eq!(scopes[0]["session_scope"], "minimal-loop");
        assert_eq!(scopes[0]["step_kind"], "");
        assert_eq!(scopes[0]["effective_required_paths"], required);
        assert_eq!(scopes[0]["initially_missing_paths"], initially_missing);
    }

    /// The tool executions in order, as `name:status`.
    fn executions(&self) -> Vec<String> {
        self.named("tool_execute")
            .iter()
            .map(|event| {
                format!(
                    "{}:{}",
                    event["name"].as_str().unwrap_or_default(),
                    event["status"].as_str().unwrap_or_default()
                )
            })
            .collect()
    }

    /// Whether request `index` carries a tool result containing `needle`. The
    /// model's own messages (which hold the Write content) do not count.
    fn request_contains(&self, index: usize, needle: &str) -> bool {
        self.requests[index].iter().any(|message| {
            message.role != "assistant"
                && message.content.starts_with("outcome:")
                && message.content.contains(needle)
        })
    }

    fn outcome(&self) -> &RunSessionOutcome {
        self.result
            .as_ref()
            .unwrap_or_else(|err| panic!("{:?}: {err}", self.protocol))
    }

    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.path().join(path)).ok()
    }

    /// A shape both protocols must share: the outcome, the requests, and the
    /// executed tools.
    fn shape(&self) -> (Result<(RunStopReason, String), String>, usize, Vec<String>) {
        (
            self.result
                .as_ref()
                .map(|outcome| (outcome.stop_reason.clone(), outcome.final_text.clone()))
                .map_err(Clone::clone),
            self.requests.len(),
            self.executions(),
        )
    }
}

/// Runs a scenario under both protocols, checks each, and requires the same
/// control sequence from both.
fn both_protocols(prompt: &str, steps: &[Step], max_iterations: usize, check: impl Fn(&Run)) {
    let runs = PROTOCOLS.map(|protocol| run_direct(protocol, prompt, steps, max_iterations));
    for run in &runs {
        check(run);
    }
    assert_eq!(runs[0].shape(), runs[1].shape(), "native and text differ");
}

const CREATE_AND_RUN: &str = "Create `hello.py` that prints a greeting, then run it with python3.";

#[test]
fn first_write_of_a_new_file_continues_to_bash_and_final() {
    let answer = "hello.py を作成し、実行して出力を確認しました。";
    let steps = [
        write("hello.py", SCRIPT),
        bash("python3 hello.py"),
        final_text(answer),
    ];
    both_protocols(CREATE_AND_RUN, &steps, 6, |run| {
        let outcome = run.outcome();
        assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
        assert_eq!(outcome.final_text, answer);
        assert_eq!(outcome.changed_paths, vec!["hello.py".to_string()]);
        // The run did not stop after the Write: a second request followed and
        // the model's Bash ran, and its real output reached the final request.
        assert_eq!(run.after_tool_stops(), 0, "{:?}", run.protocol);
        assert!(run.named("loop_stop").is_empty(), "{:?}", run.protocol);
        assert_eq!(run.requests.len(), 3, "{:?}", run.protocol);
        assert_eq!(run.remaining_replies, 0, "{:?}", run.protocol);
        assert_eq!(run.executions(), ["Write:ok", "Bash:ok"]);
        assert!(
            run.request_contains(2, "hello-from-script"),
            "{:?}",
            run.protocol
        );
        run.assert_direct_scope(json!(["hello.py"]), json!(["hello.py"]));
        assert_eq!(run.read("hello.py").as_deref(), Some(SCRIPT));
    });
}

#[test]
fn leading_ls_and_read_are_not_completion_evidence() {
    let answer = "notes.txt を確認して hello.py を作成し、実行しました。";
    let steps = [
        bash("ls -F"),
        tool("Read", json!({"path": "notes.txt"})),
        write("hello.py", SCRIPT),
        bash("python3 hello.py"),
        final_text(answer),
    ];
    both_protocols(CREATE_AND_RUN, &steps, 8, |run| {
        let outcome = run.outcome();
        assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
        assert_eq!(outcome.final_text, answer);
        assert_eq!(run.after_tool_stops(), 0, "{:?}", run.protocol);
        // The Write was the third request; the fourth (Bash) and the fifth
        // (final) still came.
        assert_eq!(run.requests.len(), 5, "{:?}", run.protocol);
        assert_eq!(run.remaining_replies, 0);
        assert_eq!(
            run.executions(),
            ["Bash:ok", "Read:ok", "Write:ok", "Bash:ok"]
        );
        assert!(run.request_contains(4, "hello-from-script"));
        run.assert_direct_scope(json!(["hello.py"]), json!(["hello.py"]));
    });
}

#[test]
fn several_tools_in_one_reply_keep_their_order_and_results() {
    let answer = "作成して実行しました。";
    let steps = [
        tools(vec![
            ("Write", json!({"path": "hello.py", "content": SCRIPT})),
            ("Bash", json!({"command": "python3 hello.py"})),
        ]),
        final_text(answer),
    ];
    both_protocols(CREATE_AND_RUN, &steps, 6, |run| {
        let outcome = run.outcome();
        assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
        assert_eq!(outcome.final_text, answer);
        assert_eq!(run.after_tool_stops(), 0, "{:?}", run.protocol);
        assert_eq!(run.requests.len(), 2, "{:?}", run.protocol);
        assert_eq!(run.executions(), ["Write:ok", "Bash:ok"]);
        assert!(run.request_contains(1, "hello-from-script"));
    });
}

#[test]
fn a_path_created_by_bash_touch_does_not_end_the_run() {
    let steps = [
        bash("touch hello.py"),
        final_text("hello.py を用意しました。"),
        final_text("hello.py を用意しました。"),
    ];
    both_protocols(CREATE_AND_RUN, &steps, 6, |run| {
        // The empty file exists, but the run neither stopped after the Bash nor
        // counted it as the requested change: the existing action check held
        // both finals and the run failed.
        assert_eq!(run.read("hello.py").as_deref(), Some(""));
        assert_eq!(run.after_tool_stops(), 0, "{:?}", run.protocol);
        assert!(run.requests.len() >= 2, "{:?}", run.protocol);
        assert_eq!(run.executions(), ["Bash:ok"]);
        assert_eq!(
            run.result.as_ref().err().map(String::as_str),
            Some(MISSING_TOOL_CALL),
            "{:?}",
            run.protocol
        );
        assert_eq!(run.remaining_replies, 0);
        run.assert_direct_scope(json!(["hello.py"]), json!(["hello.py"]));
    });
}

#[test]
fn existing_gates_still_block_after_a_new_file() {
    // A refused follow-up command stays a hard error.
    both_protocols(
        CREATE_AND_RUN,
        &[
            write("hello.py", SCRIPT),
            bash("rm -rf /"),
            final_text("unused"),
        ],
        6,
        |run| {
            let err = run.result.as_ref().unwrap_err();
            assert!(err.contains("dangerous command blocked"), "{err}");
            assert_eq!(run.after_tool_stops(), 0);
            assert_eq!(run.remaining_replies, 1, "the final is never requested");
            assert_eq!(run.read("hello.py").as_deref(), Some(SCRIPT));
        },
    );

    // A required path that is still missing is not excused by a final.
    both_protocols(
        "Create `hello.py` and `bye.py`.",
        &[
            write("hello.py", SCRIPT),
            final_text("両方作成しました。"),
            final_text("両方作成しました。"),
            final_text("両方作成しました。"),
        ],
        6,
        |run| {
            assert!(run.result.is_err(), "{:?}: {:?}", run.protocol, run.shape());
            assert_eq!(run.after_tool_stops(), 0);
            assert!(run.read("bye.py").is_none());
            run.assert_direct_scope(json!(["hello.py", "bye.py"]), json!(["hello.py", "bye.py"]));
        },
    );

    // The extra replies use the existing iteration budget (max_iterations
    // plus the existing empty-response allowance); spending it on tools is
    // still a failure, not a success because the file exists.
    let mut budget_steps = vec![write("hello.py", SCRIPT)];
    budget_steps.extend((0..8).map(|_| bash("python3 hello.py")));
    budget_steps.push(final_text("unused"));
    both_protocols(CREATE_AND_RUN, &budget_steps, 2, |run| {
        assert!(run.result.is_err(), "{:?}: {:?}", run.protocol, run.shape());
        assert_eq!(run.after_tool_stops(), 0);
        assert!(run.requests.len() < budget_steps.len(), "{:?}", run.shape());
        assert!(run.remaining_replies >= 1, "the final is never requested");
    });
}

#[test]
fn preexisting_paths_and_documents_need_no_execution() {
    // A direct edit of a file that existed keeps continuing (Issue #913).
    let edited = "notes.txt を更新しました。";
    both_protocols(
        "Update `notes.txt` to say the release is planned for summer.",
        &[
            tool(
                "Edit",
                json!({"path": "notes.txt", "old_string": "spring", "new_string": "summer"}),
            ),
            final_text(edited),
        ],
        6,
        |run| {
            let outcome = run.outcome();
            assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
            assert_eq!(outcome.final_text, edited);
            assert_eq!(run.after_tool_stops(), 0);
            assert_eq!(run.requests.len(), 2);
            run.assert_direct_scope(json!(["notes.txt"]), json!([]));
        },
    );

    // Writing a document, or a script the prompt says not to run, ends with
    // the model's final; no Bash is demanded.
    for prompt in [
        "Write a short summary of `notes.txt` to `summary.md`.",
        "Create `hello.py` that prints a greeting. Do not run it.",
    ] {
        let path = if prompt.contains("summary.md") {
            "summary.md"
        } else {
            "hello.py"
        };
        let answer = "書きました。";
        both_protocols(
            prompt,
            &[write(path, SCRIPT), final_text(answer)],
            6,
            |run| {
                let outcome = run.outcome();
                assert_eq!(
                    outcome.stop_reason,
                    RunStopReason::AssistantFinal,
                    "{prompt}"
                );
                assert_eq!(outcome.final_text, answer, "{prompt}");
                assert_eq!(run.after_tool_stops(), 0, "{prompt}");
                assert_eq!(run.requests.len(), 2, "{prompt}");
                assert_eq!(run.executions(), ["Write:ok"], "{prompt}");
            },
        );
    }

    // A read-only answer stays a read and an answer.
    let answer = "リリースは春の予定です。";
    both_protocols(
        "Explain what `notes.txt` says.",
        &[
            tool("Read", json!({"path": "notes.txt"})),
            final_text(answer),
        ],
        6,
        |run| {
            let outcome = run.outcome();
            assert_eq!(outcome.stop_reason, RunStopReason::AssistantFinal);
            assert_eq!(outcome.final_text, answer);
            assert_eq!(run.after_tool_stops(), 0);
            assert_eq!(run.read("notes.txt").as_deref(), Some(NOTES));
        },
    );

    // A final alone does not satisfy a document that was never written.
    both_protocols(
        "Write a short summary of `notes.txt` to `summary.md`.",
        &[
            final_text("summary.md に書きました。"),
            final_text("summary.md に書きました。"),
            final_text("summary.md に書きました。"),
        ],
        6,
        |run| {
            assert!(run.result.is_err(), "{:?}: {:?}", run.protocol, run.shape());
            assert!(run.read("summary.md").is_none());
            assert_eq!(run.after_tool_stops(), 0);
        },
    );
}

fn corpus_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(CORPUS)
        .join(name)
}

fn read_json(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(corpus_path(name)).unwrap()).unwrap()
}

fn sha256_hex(path: &Path) -> String {
    Sha256::digest(std::fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn corpus_fixtures_match_the_manifest_digests() {
    let manifest = read_json("source-manifest.json");
    let digests = manifest["fixture_sha256"].as_object().unwrap();
    assert_eq!(digests.len(), 2);
    for (name, digest) in digests {
        let actual = sha256_hex(&corpus_path(&format!("fixtures/{name}")));
        assert_eq!(actual, digest.as_str().unwrap(), "{name}");
    }
}

#[test]
fn recorded_runs_stopped_right_after_the_creating_tool() {
    let records = std::fs::read_to_string(corpus_path("fixtures/recorded/runs.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 3);
    let mut shapes = Vec::new();
    for record in &records {
        let id = record["id"].as_str().unwrap();
        assert_eq!(record["kind"], "recorded", "{id}");
        assert_eq!(record["campaign"], "L-152", "{id}");
        assert_eq!(record["source_code"], "b2c3b372", "{id}");
        assert!(
            record["run_dir"].as_str().unwrap().starts_with("<bench>/"),
            "{id}"
        );
        assert_eq!(record["events_sha256"].as_str().unwrap().len(), 64, "{id}");
        assert_eq!(record["session_scope"], "minimal-loop", "{id}");
        assert_eq!(record["step_kind"], "", "{id}");
        assert!(
            !record["initially_missing_paths"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{id}"
        );
        // The rows end with a successful tool and the after-tool stop on the
        // very next line, with no provider request and no final in between.
        let rows = record["rows"].as_array().unwrap();
        let last = rows.last().unwrap();
        assert_eq!(last["event"], "loop_stop", "{id}");
        assert_eq!(last["reason"], AFTER_TOOL, "{id}");
        let before = &rows[rows.len() - 2];
        assert_eq!(before["event"], "tool_execute", "{id}");
        assert_eq!(before["status"], "ok", "{id}");
        assert_eq!(
            last["line"].as_u64().unwrap(),
            before["line"].as_u64().unwrap() + 1,
            "{id}"
        );
        let shape = record["shape"].as_str().unwrap();
        // Completed runs saved a session; the touch-only one was not checked.
        let session = if shape == "touch-only" {
            Value::Null
        } else {
            json!(true)
        };
        assert_eq!(record["session_saved"], session, "{id}");
        shapes.push(shape.to_string());
    }
    assert_eq!(shapes, ["write-only", "ls-read-write", "touch-only"]);
}

#[test]
fn synthetic_cases_are_separate_and_name_existing_tests() {
    let cases = read_json("fixtures/synthetic/cases.json");
    assert_eq!(cases["kind"], "synthetic");
    let sources = [
        "tests/direct_created_artifact_followthrough.rs",
        "src/minimal_loop/loop_run/after_tool_completion/tests.rs",
        "tests/prompt_fix_early_completion.rs",
    ]
    .map(|path| std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap());
    let cases = cases["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(case.get("events_sha256").is_none(), "{id} is synthetic");
        let test = case["test"].as_str().unwrap();
        assert!(
            sources
                .iter()
                .any(|source| source.contains(&format!("fn {test}("))),
            "{id}: {test}"
        );
    }
}

#[test]
fn corpus_carries_no_host_paths_secrets_or_model_names() {
    for name in [
        "expectations.toml",
        "source-manifest.json",
        "fixtures/recorded/runs.jsonl",
        "fixtures/synthetic/cases.json",
    ] {
        let text = std::fs::read_to_string(corpus_path(name)).unwrap();
        for forbidden in [
            "/Users/",
            "/home/",
            "/Volumes/",
            "OPENAI_API_KEY",
            "sk-",
            "Bearer ",
            "\"model\"",
            "gemma",
            "Prompt(",
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}
