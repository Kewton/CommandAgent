//! Issue #997: a profile tool's schema refusal reaches the model through the 500-character
//! history snippet with its stage, at least one whole violation, and the counts.
//!
//! Every run drives the real Chat Completions client against a disposable fake on
//! 127.0.0.1, which answers the first request with a call the schema refuses. What is
//! judged is the tool message of the second request: the bytes the provider received.
//! No model, real key, external provider, or Jev call is used.

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::providers::client_from_config;
use commandagent::sensitive_data::{SecretCatalog, install_scope, reset_scopes_for_tests};
use commandagent::state::{ConversationMessage, SessionSnapshot};
use commandagent::tools::profile_tools::{
    ProfileToolReadExecutor, ReadToolOutput, register_read_executor,
};
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
const PROFILE: &str = "schema-feedback";
const TOOL_ID: &str = "feedback-submit";
const TOOL: &str = "SubmitFeedback";
const RUNTIME: &str = "schema-feedback-runtime";
/// A pinned process tool: on macOS it can start, so a refusal must keep it from starting.
const PROCESS_PROFILE: &str = "schema-feedback-process";
const PROCESS_TOOL_ID: &str = "feedback-propose";
#[cfg(target_os = "macos")]
const PROCESS_TOOL: &str = "ProposeFeedback";
const DUMMY_SECRET: &str = "fake-dummy-credential-997-not-a-real-key";
const SNIPPET_LIMIT: usize = 500;
/// What the model was left with before #997, for a refusal too long for the snippet.
const LOST: &str = "profile_tool_refused: schema_violation:";

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_env(name: &str, value: Option<&str>) {
    // SAFETY: every env mutation in this binary holds `LOCK`.
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// ---------------------------------------------------------------------------
// The fake provider: 127.0.0.1, one scripted reply per request.
// ---------------------------------------------------------------------------

struct FakeHttp {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeHttp {
    fn start(replies: Vec<String>) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
        let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (requests, stop) = (requests.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => serve(stream, &requests, &replies),
                        Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
                    }
                }
            })
        };
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.requests.lock().unwrap().clone()
    }
}

fn serve(
    mut stream: std::net::TcpStream,
    requests: &Mutex<Vec<String>>,
    replies: &Mutex<VecDeque<String>>,
) {
    stream.set_nonblocking(false).unwrap();
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    let body_start = loop {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            return;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
    };
    let head = String::from_utf8_lossy(&bytes[..body_start]).to_ascii_lowercase();
    let length = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    while bytes.len() < body_start + length {
        let read = stream.read(&mut chunk).unwrap_or(0);
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    requests
        .lock()
        .unwrap()
        .push(String::from_utf8_lossy(&bytes[body_start..]).to_string());
    let reply = replies
        .lock()
        .unwrap()
        .pop_front()
        .unwrap_or_else(|| text_reply("script exhausted"));
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn text_reply(content: &str) -> String {
    json!({"choices": [{"message": {"role": "assistant", "content": content}}]}).to_string()
}

fn tool_reply(tool: &str, raw: &str) -> String {
    json!({"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [{
        "id": "call-0",
        "type": "function",
        "function": {"name": tool, "arguments": raw},
    }]}}]})
    .to_string()
}

// ---------------------------------------------------------------------------
// The operator: a recording executor, one profile, one descriptor.
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct RecordingReader {
    calls: Mutex<Vec<Value>>,
}

impl ProfileToolReadExecutor for RecordingReader {
    fn read(&self, _workspace_root: &Path, arguments: &Value) -> anyhow::Result<ReadToolOutput> {
        self.calls.lock().unwrap().push(arguments.clone());
        Ok(ReadToolOutput {
            text: "accepted by the fake".to_string(),
            usage: None,
        })
    }
}

fn schema() -> Value {
    json!({
        "protocol": "profile-tool.schema/v1",
        "root": {
            "type": "object",
            "additionalProperties": false,
            "required": ["mode"],
            "properties": {
                "mode": {"type": "string", "enum": ["fast", "safe"]},
                "limits": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {"retries": {"type": "integer", "minimum": 1, "maximum": 3}},
                },
                "targets": {"type": "array", "maxItems": 1, "items": {"type": "string"}},
                "name": {"type": "string", "maxLength": 3},
            },
        },
    })
}

fn extension() {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        for (profile, tool_id, effect) in [
            (PROFILE, TOOL_ID, "read"),
            (PROCESS_PROFILE, PROCESS_TOOL_ID, "propose-artifacts"),
        ] {
            let directory = root.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            let manifest = format!(
                "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Schema feedback {profile}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [\"Read\"]\ntools = [\n  {{ id = \"{tool_id}\", effect = \"{effect}\" }},\n]\n"
            );
            std::fs::write(directory.join("manifest.toml"), manifest).unwrap();
        }
        commandagent::planner::extension_profiles::register(&root).unwrap();
        root
    });
}

fn base_config(root: &Path, evidence: &Path, profile: &str, url: &str) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: evidence.join("state"),
        eval_events_path: Some(evidence.join("events.jsonl")),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 100_000,
        model: "m".to_string(),
        provider: Provider::LmStudio,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "m".to_string(),
        planner_provider: Provider::LmStudio,
        planner_think: Some(OllamaThink::False),
        classifier_model: "m".to_string(),
        classifier_provider: Provider::LmStudio,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: url.to_string(),
        num_predict: 100,
        max_iterations: 8,
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
        profile: profile.to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}

struct Env {
    _workspace: tempfile::TempDir,
    _evidence: tempfile::TempDir,
    _operator: tempfile::TempDir,
    root: PathBuf,
    evidence: PathBuf,
    operator: PathBuf,
    profile: &'static str,
    tool: &'static str,
}

impl Env {
    /// Sets the registry variable; the caller holds `LOCK`.
    fn new() -> Self {
        extension();
        let workspace = tempfile::tempdir().unwrap();
        let evidence = tempfile::tempdir().unwrap();
        let operator = tempfile::tempdir().unwrap();
        let operator_path = std::fs::canonicalize(operator.path()).unwrap();
        let schema = schema();
        let descriptor = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": TOOL_ID,
            "name": TOOL,
            "version": "1.0.0",
            "description": "Submit one object for a synthetic fixture.",
            "effect": "read",
            "parameters": [],
            "schema": schema,
            "schema_sha256": sha256(serde_json::to_string(&schema).unwrap().as_bytes()),
            "runtime": {"kind": "in_process", "id": RUNTIME},
            "requires": [],
            "limits": {"max_input_bytes": 1 << 20, "max_output_bytes": 4096},
        });
        let env = Self {
            root: std::fs::canonicalize(workspace.path()).unwrap(),
            evidence: std::fs::canonicalize(evidence.path()).unwrap(),
            operator: operator_path,
            _workspace: workspace,
            _evidence: evidence,
            _operator: operator,
            profile: PROFILE,
            tool: TOOL,
        };
        env.write_descriptor(TOOL_ID, &descriptor);
        set_env(REGISTRY_ENV, env.operator.join("registry").to_str());
        env
    }

    fn write_descriptor(&self, tool_id: &str, descriptor: &Value) {
        let directory = self.operator.join("registry/profile-tools");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join(format!("{tool_id}.json")),
            descriptor.to_string(),
        )
        .unwrap();
    }

    fn config(&self) -> Config {
        base_config(&self.root, &self.evidence, self.profile, "http://unused")
    }

    fn events(&self) -> Vec<Value> {
        std::fs::read_to_string(self.evidence.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

struct Run {
    executed: Vec<Value>,
    requests: Vec<String>,
    result: Result<String, String>,
}

impl Run {
    /// The tool message of the second request, as the provider received it.
    fn feedback(&self) -> String {
        assert_eq!(self.requests.len(), 2, "{:?}", self.result);
        let body: Value = serde_json::from_str(&self.requests[1]).unwrap();
        let last = body["messages"].as_array().unwrap().last().unwrap().clone();
        assert_eq!(last["role"], "tool", "{last}");
        last["content"].as_str().unwrap().to_string()
    }
}

/// One refused call and then `done`. `secret` installs a dummy credential first.
fn run(env: &Env, raw: &str, secret: bool) -> Run {
    let reader = Arc::new(RecordingReader::default());
    register_read_executor(RUNTIME, reader.clone());
    let http = FakeHttp::start(vec![tool_reply(env.tool, raw), text_reply("done")]);
    let mut config = env.config();
    config.lm_studio_host = http.url.clone();
    if secret {
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
            .unwrap();
        install_scope(catalog, Some(&env.root), config.eval_events_path.as_deref());
    }
    let mut client = client_from_config(&config, false).unwrap();
    let result = run_session_with_outcome_with_ui(
        client.as_mut(),
        &mut SessionSnapshot::new(),
        "Submit the object.",
        &[],
        &config,
        &NOOP_UI,
    )
    .map(|outcome| outcome.final_text)
    .map_err(|error| format!("{error:#}"));
    let requests = http.finish();
    let executed = reader.calls.lock().unwrap().clone();
    Run {
        executed,
        requests,
        result,
    }
}

/// The summary's own part, before the refusal's text that follows it.
fn summary_part(feedback: &str) -> &str {
    let start = feedback
        .find("schema_violation stage=")
        .unwrap_or_else(|| panic!("no summary: {feedback}"));
    let end = feedback
        .find("extension_tool_rejected:")
        .unwrap_or(feedback.len());
    &feedback[start..end]
}

/// The shown violations: `code@pointer` tokens after the counts.
fn shown(feedback: &str) -> Vec<(String, String)> {
    let part = summary_part(feedback);
    let (_, items) = part.split_once(": ").unwrap();
    items
        .split_whitespace()
        .map(|token| {
            let (code, pointer) = token
                .split_once('@')
                .unwrap_or_else(|| panic!("`{token}` is not code@pointer: {feedback}"));
            (code.to_string(), pointer.to_string())
        })
        .collect()
}

/// What every refusal keeps: nothing ran, the run went on, the history stayed bounded,
/// and the same history policy run again changes nothing.
fn assert_bounded_refusal(run: &Run, label: &str) -> String {
    assert_eq!(
        run.executed,
        Vec::<Value>::new(),
        "{label}: the executor ran"
    );
    assert_eq!(run.result.as_deref(), Ok("done"), "{label}");
    let feedback = run.feedback();
    assert!(
        feedback.chars().count() <= SNIPPET_LIMIT,
        "{label}: {} chars: {feedback}",
        feedback.chars().count()
    );
    let again = ConversationMessage::tool(TOOL, feedback.clone()).content;
    assert_eq!(
        again, feedback,
        "{label}: a second pass changed the history"
    );
    feedback
}

/// Eight violations, the validator's most: the refusal's JSON alone is over the limit.
const MANY: &str = r#"{"mode":"slow","limits":{"retries":9,"extra_limit_key":1},"targets":[1,2],"name":"toolong","alpha_extra_key":1,"bravo_extra_key":2}"#;

#[test]
fn a_long_refusal_reaches_the_provider_with_its_stage_a_whole_violation_and_the_counts() {
    let _guard = lock();
    let env = Env::new();
    let run = run(&env, MANY, false);
    let feedback = assert_bounded_refusal(&run, "many");
    // Before #997 the snippet ended right here, with no stage, code or pointer.
    assert!(
        feedback.contains("stage=received_native"),
        "the stage did not reach the provider: {feedback}"
    );
    assert!(
        feedback.contains("violations=8 shown=3 omitted=5:"),
        "{feedback}"
    );
    // The first three, in the validator's order (its object walk is by sorted key).
    assert_eq!(
        shown(&feedback),
        [
            ("unknown_key", "/alpha_extra_key"),
            ("unknown_key", "/bravo_extra_key"),
            ("unknown_key", "/limits/extra_limit_key"),
        ]
        .map(|(code, pointer)| (code.to_string(), pointer.to_string())),
        "{feedback}"
    );
    assert!(!feedback.contains("\"violations\""), "{feedback}");
    assert!(
        feedback.ends_with(LOST),
        "the long JSON was expected to stay cut: {feedback}"
    );
    set_env(REGISTRY_ENV, None);
}

/// The feedback before #997, built here the way base `fea0a54e` wrote it: the refusal's
/// data is the validator's JSON of `(code, path)` pairs at `received_native`.
fn today(tool: &str, violations: &[(&str, &str)]) -> String {
    let violations = violations
        .iter()
        .map(|(code, path)| json!({"code": code, "path": path}))
        .collect::<Vec<_>>();
    let data = json!({"stage": "received_native", "violations": violations});
    format!(
        "Tool call `{tool}` was rejected with a recoverable validation error: extension_tool_rejected: `{tool}`: profile_tool_refused: schema_violation: {data}. Retry with the same tool or another available tool using a valid JSON object that matches the tool schema."
    )
}

/// Two unknown keys, `a…` and `b…`, the second sized so that today's feedback has exactly
/// `total` characters. The keys sort in this order, which is the validator's walk.
fn sized_keys(total: usize) -> (String, String, String) {
    let first = format!("a{}", "x".repeat(59));
    let fixed = today(
        TOOL,
        &[("unknown_key", &format!("/{first}")), ("unknown_key", "/b")],
    )
    .chars()
    .count();
    let second = format!("b{}", "y".repeat(total - fixed));
    let expected = today(
        TOOL,
        &[
            ("unknown_key", &format!("/{first}")),
            ("unknown_key", &format!("/{second}")),
        ],
    );
    assert_eq!(expected.chars().count(), total);
    (first, second, expected)
}

#[test]
fn a_short_refusal_is_today_s_feedback_byte_for_byte() {
    let _guard = lock();
    let env = Env::new();
    let run = run(&env, r#"{"mode":"slow"}"#, false);
    let feedback = assert_bounded_refusal(&run, "short");
    assert_eq!(feedback, today(TOOL, &[("enum_mismatch", "/mode")]));
    set_env(REGISTRY_ENV, None);
}

/// The refusal that failed `tests/profile_tool_schema.rs:683` when a summary went first:
/// four violations fit today, and a summary in front would push the JSON out.
#[test]
fn a_four_violation_refusal_that_fits_today_is_today_s_feedback_byte_for_byte() {
    let _guard = lock();
    let env = Env::new();
    let raw = r#"{"mode":"slow","limits":{"retries":9},"targets":[1],"name":"toolong"}"#;
    let run = run(&env, raw, false);
    let feedback = assert_bounded_refusal(&run, "four");
    let expected = today(
        TOOL,
        &[
            ("above_maximum", "/limits/retries"),
            ("enum_mismatch", "/mode"),
            ("string_too_long", "/name"),
            ("wrong_type", "/targets/0"),
        ],
    );
    let length = expected.chars().count();
    assert!(length <= SNIPPET_LIMIT, "{length}");
    assert!(
        length > SNIPPET_LIMIT - 150,
        "{length}: too short to need this case"
    );
    assert_eq!(feedback, expected);
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_refusal_of_exactly_the_snippet_limit_is_today_s_feedback_byte_for_byte() {
    let _guard = lock();
    let env = Env::new();
    let (first, second, expected) = sized_keys(SNIPPET_LIMIT);
    let raw = json!({"mode": "fast", first: 1, second: 2}).to_string();
    let run = run(&env, &raw, false);
    let feedback = assert_bounded_refusal(&run, "limit");
    assert_eq!(feedback.chars().count(), SNIPPET_LIMIT);
    assert_eq!(feedback, expected);
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_refusal_one_character_over_the_limit_leads_with_the_summary() {
    let _guard = lock();
    let env = Env::new();
    let (first, second, expected) = sized_keys(SNIPPET_LIMIT + 1);
    assert_eq!(expected.chars().count(), SNIPPET_LIMIT + 1);
    let raw = json!({"mode": "fast", first.as_str(): 1, second.as_str(): 2}).to_string();
    let run = run(&env, &raw, false);
    let feedback = assert_bounded_refusal(&run, "limit + 1");
    assert!(
        feedback.contains("schema_violation stage=received_native violations=2 shown=2 omitted=0:"),
        "{feedback}"
    );
    assert_eq!(
        shown(&feedback),
        vec![
            ("unknown_key".to_string(), format!("/{first}")),
            ("unknown_key".to_string(), format!("/{second}")),
        ],
        "{feedback}"
    );
    set_env(REGISTRY_ENV, None);
}

/// A secret in a refusal that fits: the decision is made on the scrubbed snippet, so the
/// feedback is today's, scrubbed as today.
#[test]
fn a_fitting_refusal_with_a_secret_is_today_s_scrubbed_feedback() {
    let _guard = lock();
    for key in [DUMMY_SECRET, "my_api_key_field"] {
        let env = Env::new();
        let raw = json!({"mode": "fast", key: 1}).to_string();
        let run = run(&env, &raw, true);
        let expected = today(TOOL, &[("unknown_key", &format!("/{key}"))]);
        let scrubbed = commandagent::eval_events::body_snippet_whole_tokens(&expected);
        reset_scopes_for_tests();
        let feedback = assert_bounded_refusal(&run, key);
        assert_ne!(scrubbed, expected, "{key}: the scrub did nothing");
        assert_eq!(feedback, scrubbed, "{key}");
        assert!(!feedback.contains(key), "{key}: {feedback}");
        assert!(!feedback.contains("schema_violation stage="), "{feedback}");
        set_env(REGISTRY_ENV, None);
    }
}

#[test]
fn a_long_pointer_is_cut_to_one_bounded_token() {
    let _guard = lock();
    let env = Env::new();
    let keys = ["a", "b", "c"].map(|first| format!("{first}{}", "k".repeat(119)));
    let raw = format!(
        r#"{{"mode":"fast","{}":1,"{}":2,"{}":3}}"#,
        keys[0], keys[1], keys[2]
    );
    let run = run(&env, &raw, false);
    let feedback = assert_bounded_refusal(&run, "long pointer");
    assert!(
        feedback.contains("violations=3 shown=3 omitted=0:"),
        "{feedback}"
    );
    let shown = shown(&feedback);
    assert_eq!(shown.len(), 3, "{feedback}");
    for (code, pointer) in &shown {
        assert_eq!(code, "unknown_key", "{feedback}");
        assert_eq!(pointer.chars().count(), 64, "{pointer}: {feedback}");
        assert!(pointer.ends_with("..."), "{pointer}: {feedback}");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn control_whitespace_and_invisible_characters_never_reach_the_summary() {
    let _guard = lock();
    let env = Env::new();
    let key = "a\tb\u{7}c\u{202e}d\u{fffd}e f";
    // Two long keys after it make the refusal too long for the snippet.
    let raw = json!({"mode": "fast", key: 1, "x".repeat(120): 2, "y".repeat(120): 3}).to_string();
    let run = run(&env, &raw, false);
    let feedback = assert_bounded_refusal(&run, "characters");
    let shown = shown(&feedback);
    assert_eq!(shown.len(), 3, "{feedback}");
    assert_eq!(
        shown[0],
        ("unknown_key".to_string(), "/a?b?c?d?e?f".to_string()),
        "{feedback}"
    );
    assert!(!feedback.chars().any(char::is_control), "{feedback:?}");
    let summary = summary_part(&feedback);
    for invisible in ['\u{202e}', '\u{fffd}'] {
        assert!(!summary.contains(invisible), "{summary:?}");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_secret_in_a_key_is_scrubbed_before_the_pointer_is_cut() {
    let _guard = lock();
    let env = Env::new();
    // The cut lands inside the secret: a cut before the scrub would leave its head.
    let padded = format!("{}{DUMMY_SECRET}", "p".repeat(40));
    // A third, long key makes the refusal too long for the snippet. No key holds
    // `api_key`: that heuristic would replace the whole JSON token and make it short.
    let long = "c".repeat(100);
    let raw = format!(r#"{{"mode":"fast","{padded}":1,"{long}":2,"{DUMMY_SECRET}":3}}"#);
    let run = run(&env, &raw, true);
    reset_scopes_for_tests();
    let feedback = assert_bounded_refusal(&run, "secret");
    assert!(
        feedback.contains("violations=3 shown=3 omitted=0:"),
        "{feedback}"
    );
    for fragment in [DUMMY_SECRET, &DUMMY_SECRET[..12]] {
        assert!(!feedback.contains(fragment), "`{fragment}`: {feedback}");
        assert!(
            !run.requests[1].contains(fragment),
            "`{fragment}` in the request"
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// The refusal is the same refusal: the event records the error, not the feedback, and no
/// other tool event appears.
#[test]
fn the_refusal_and_its_events_are_unchanged() {
    let _guard = lock();
    let env = Env::new();
    let run = run(&env, MANY, false);
    assert_bounded_refusal(&run, "events");
    let events = env.events();
    let names = events
        .iter()
        .filter_map(|event| event["event"].as_str())
        .filter(|name| name.starts_with("extension_tool") || name.starts_with("profile_tool"))
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["extension_tool_call", "extension_tool_rejected"],
        "{events:?}"
    );
    let rejected = events
        .iter()
        .find(|event| event["event"] == "extension_tool_rejected")
        .unwrap();
    let violations = [
        ("unknown_key", "/alpha_extra_key"),
        ("unknown_key", "/bravo_extra_key"),
        ("unknown_key", "/limits/extra_limit_key"),
        ("above_maximum", "/limits/retries"),
        ("enum_mismatch", "/mode"),
        ("string_too_long", "/name"),
        ("too_many_items", "/targets"),
        ("wrong_type", "/targets/0"),
    ]
    .map(|(code, path)| json!({"code": code, "path": path}));
    let data = json!({"stage": "received_native", "violations": violations});
    assert_eq!(
        *rejected,
        json!({
            "access": "read_only",
            "arguments": {"argument_keys": [], "argument_summaries": {}, "arguments_type": "object"},
            "event": "extension_tool_rejected",
            "name": TOOL,
            "policy": "workspace",
            "reason": format!("profile_tool_refused: schema_violation: {data}"),
            "schema_version": "1",
            "transport": "profile_tool",
        }),
    );
    set_env(REGISTRY_ENV, None);
}

/// A pinned process starts only on macOS. There a refused argument still starts nothing:
/// the same tool commits a valid call, so a missing commit is the refusal's, and the
/// summary reaches the model as it does for a read tool.
#[cfg(target_os = "macos")]
mod process {
    use commandagent::tools::profile_tools::directory_pin;
    use serde_json::{Value, json};

    use super::{
        Env, MANY, PROCESS_PROFILE, PROCESS_TOOL, PROCESS_TOOL_ID, REGISTRY_ENV, run, schema,
        set_env, sha256, shown,
    };

    const OUTPUT: &str = "config/settings.json";
    const CANDIDATE: &[u8] = b"{\"mode\":\"fast\"}\n";

    fn encode_base64(bytes: &[u8]) -> String {
        const SYMBOLS: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut text = String::new();
        for chunk in bytes.chunks(3) {
            let mut word = [0u8; 3];
            word[..chunk.len()].copy_from_slice(chunk);
            let word = u32::from(word[0]) << 16 | u32::from(word[1]) << 8 | u32::from(word[2]);
            for index in 0..4 {
                if index <= chunk.len() {
                    text.push(SYMBOLS[(word >> (18 - 6 * index) & 63) as usize] as char);
                } else {
                    text.push('=');
                }
            }
        }
        text
    }

    /// A fixed script for `/bin/bash`: it repeats the request's pins and proposes
    /// `CANDIDATE`. No part of it comes from a model.
    fn script() -> String {
        format!(
            r#"IFS= read -r request
take() {{ local pattern="\"$1\":\"([^\"]+)\""; [[ $request =~ $pattern ]] && printf -v "$2" '%s' "${{BASH_REMATCH[1]}}"; }}
take request_id rid; take tool_id tid; take runtime_sha256 run; take input_revision rev; take schema_sha256 sch
printf '%s' "{{\"protocol\":\"profile-tool.result/v1\",\"request_id\":\"$rid\",\"tool_id\":\"$tid\",\"runtime_sha256\":\"$run\",\"input_revision\":\"$rev\",\"schema_sha256\":\"$sch\",\"checks\":[{{\"id\":\"shape\",\"status\":\"passed\"}}],\"outputs\":[{{\"id\":\"artifact\",\"sha256\":\"{digest}\",\"bytes_base64\":\"{encoded}\"}}],\"diagnostics\":[]}}"
"#,
            digest = sha256(CANDIDATE),
            encoded = encode_base64(CANDIDATE),
        )
    }

    fn process_env() -> Env {
        let mut env = Env::new();
        let directory = env.operator.join("tools").join(PROCESS_TOOL_ID);
        std::fs::create_dir_all(&directory).unwrap();
        let script_path = directory.join("tool.sh");
        std::fs::write(&script_path, script()).unwrap();
        let shell = std::fs::canonicalize("/bin/bash").unwrap();
        let schema = schema();
        let descriptor = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": PROCESS_TOOL_ID,
            "name": PROCESS_TOOL,
            "version": "1.0.0",
            "description": "Propose one artifact for a synthetic fixture.",
            "effect": "propose-artifacts",
            "parameters": [],
            "schema_sha256": sha256(serde_json::to_string(&schema).unwrap().as_bytes()),
            "schema": schema,
            "runtime": {"kind": "pinned_process", "id": "fixed-script"},
            "limits": {"max_input_bytes": 1 << 20, "max_output_bytes": 64 * 1024},
            "process": {
                "protocol": "profile-tool.process/v1",
                "executable": {
                    "path": shell,
                    "sha256": sha256(&std::fs::read(&shell).unwrap()),
                },
                "dependency_directories": [{
                    "path": directory,
                    "sha256": directory_pin(&directory).unwrap(),
                }],
                "argv": [script_path],
                "cwd": directory,
                "env": {"TOOL_MODE": "fixture"},
                "limits": {"timeout_ms": 10_000, "max_result_bytes": 64 * 1024},
                "outputs": [{"id": "artifact", "path": OUTPUT}],
            },
        });
        env.write_descriptor(PROCESS_TOOL_ID, &descriptor);
        env.profile = PROCESS_PROFILE;
        env.tool = PROCESS_TOOL;
        env
    }

    fn commits(events: &[Value]) -> usize {
        events
            .iter()
            .filter(|event| event["event"] == "profile_tool_commit")
            .count()
    }

    #[test]
    fn a_refused_argument_starts_no_process_and_the_summary_reaches_the_model() {
        let _guard = super::lock();
        // The witness: a valid call to the same tool starts it and commits.
        let env = process_env();
        let valid = run(&env, r#"{"mode":"fast"}"#, false);
        assert_eq!(valid.result.as_deref(), Ok("done"));
        assert_eq!(std::fs::read(env.root.join(OUTPUT)).unwrap(), CANDIDATE);
        assert_eq!(commits(&env.events()), 1, "{:?}", env.events());

        let env = process_env();
        let refused = run(&env, MANY, false);
        let feedback = super::assert_bounded_refusal(&refused, "process");
        assert!(
            feedback.contains("stage=received_native violations=8 shown=3 omitted=5:"),
            "{feedback}"
        );
        assert_eq!(shown(&feedback).len(), 3, "{feedback}");
        assert!(!env.root.join(OUTPUT).exists(), "a process committed");
        let events = env.events();
        assert_eq!(commits(&events), 0, "{events:?}");
        let rejected = events
            .iter()
            .filter(|event| event["event"] == "profile_tool_effect_rejected")
            .collect::<Vec<_>>();
        assert_eq!(rejected.len(), 1, "{events:?}");
        assert_eq!(rejected[0]["kind"], "schema_violation", "{events:?}");
        // Nothing ran, so nothing was said.
        assert_eq!(rejected[0]["diagnostics"], json!([]), "{events:?}");
        assert_eq!(rejected[0]["stderr_tail"], "", "{events:?}");
        set_env(REGISTRY_ENV, None);
    }
}
