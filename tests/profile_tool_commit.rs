//! Issue #974 (G3): a pinned process proposes bytes for one declared output, and only the
//! host, after checking them, adopts them.
//!
//! Success runs drive the real Chat Completions client against a disposable fake on
//! 127.0.0.1, because the raw reply is what G2 receives. The process is a fixed script for
//! `/bin/bash` with a fixed argv, written and pinned by the test: it takes no shell text
//! from a model. Under the strict policy it cannot fork, so it uses builtins only. No model,
//! key, external provider, or Jev call is used. On macOS a failed strict probe fails these
//! tests; it is never a skip. On any other OS the same entries expect `unavailable`.
//!
//! The scenarios are listed in `tests/corpus/apps/profile-tool-commit/`. The off control
//! compares with `fixtures/off-golden.json`, recorded from base 85bcca4e before this
//! feature existed. What is decided around the launch, on every OS, is in the unit tests of
//! `src/tools/profile_tools/`.

use std::collections::{BTreeMap, VecDeque};
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider, ToolProtocol,
};
use commandagent::minimal_loop::loop_run::{
    run_session, run_session_with_outcome_with_ui, run_session_with_required_paths,
    run_session_with_required_paths_with_ui,
};
use commandagent::minimal_loop::request_pattern::{
    Classification, ClassifyFailure, PatternClassifier, RequestPatternMode, RequestPatternSettings,
    run_direct_prompt_with, run_direct_prompt_with_common,
};
use commandagent::planner::runner::{
    generate_and_run_step_plan, generate_and_run_step_plan_with_ui, generate_and_run_ultra_plan,
    generate_and_run_ultra_plan_with_ui, generate_step_plan, generate_ultra_plan,
    generate_ultra_plan_with_ui, run_plan_file, run_step_plan, run_step_plan_with_ui,
    run_ultra_plan, run_ultra_plan_file, run_ultra_plan_file_with_ui, run_ultra_plan_with_ui,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan, render_step_plan};
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, render_ultra_plan};
use commandagent::providers::{ChatClient, client_from_config};
use commandagent::sensitive_data::{SecretCatalog, install_scope};
use commandagent::state::SessionSnapshot;
use commandagent::tools::profile_tools::{
    ProfileToolReadExecutor, ReadToolOutput, admit_selected, directory_pin, register_read_executor,
};
use commandagent::tui::NOOP_UI;
use commandagent::tui::boundary_shell::BoundaryShell;
use commandagent::tui::boundary_shell::ambiguity::{
    ClassifierProvenance, ProposalStatus, RouteProposal,
};
use commandagent::tui::boundary_shell::confirmation::{ExecutionPins, PackSelection};
use commandagent::tui::boundary_shell::directive::DirectiveContinuation;
use commandagent::tui::boundary_shell::route::{
    DeterministicResolution, ExplicitRouteBinding, RouteRequest, deterministic_route,
};
use commandagent::tui::slash::handle_command;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const OFF_GOLDEN: &str = include_str!("corpus/apps/profile-tool-commit/fixtures/off-golden.json");
const CASES: &str = include_str!("corpus/apps/profile-tool-commit/fixtures/cases.json");
const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
const CONFIG_PROFILE: &str = "commit-config";
const INDEX_PROFILE: &str = "commit-index";
const PACK_HOST: &str = "commit-pack-host";
const COMMIT_PACK: &str = "commit-pack";
const CONFIG_TOOL_ID: &str = "config-submit";
const INDEX_TOOL_ID: &str = "index-submit";
const CONFIG_TOOL: &str = "SubmitConfig";
#[cfg(target_os = "macos")]
const INDEX_TOOL: &str = "SubmitIndex";
const OUTPUT: &str = "config/settings.json";
const INDEX_OUTPUT: &str = "config/index.json";
#[cfg(target_os = "macos")]
const VALID: &str = r#"{"mode":"fast"}"#;
const CANDIDATE_FAST: &[u8] = b"{\n  \"mode\": \"fast\"\n}\n";
const CANDIDATE_SAFE: &[u8] = b"{\n  \"mode\": \"safe\"\n}\n";
#[cfg(target_os = "macos")]
const OLD_BYTES: &[u8] = b"the bytes before this run\n";
const DUMMY_SECRET: &str = "fake-dummy-credential-974-not-a-real-key";
const FLAT_PROFILE: &str = "commit-off-flat";
const FLAT_TOOL: &str = "off-flat-read";
const FLAT_RUNTIME: &str = "off-flat-runtime";
const FLAT_PARAMETERS: &str = r#"[{"name":"path","kind":"workspace_path","required":true,"description":"Workspace-relative file."}]"#;
const RESERVED_WORDS_PROMPT: &str = "Check fake-dummy-credential-974-not-a-real-key next to propose-artifacts pinned_process profile-tool.process/v1 profile-tool.request/v1 profile-tool.result/v1 profile-tool.commit-receipt/v1 effect_read_only effect_limit_exceeded effect_output_refused effect_path_refused effect_env_refused process_failed result_rejected commit_failed profile_tool_effect_call profile_tool_effect_rejected profile_tool_commit committed_sha256 result_sha256 runtime_sha256.";

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

type Responder = Arc<dyn Fn(usize, &str) -> Option<String> + Send + Sync>;

struct FakeHttp {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    replies: Arc<Mutex<VecDeque<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeHttp {
    fn start(replies: Vec<String>) -> Self {
        Self::with_responder(replies, Arc::new(|_, _| None))
    }

    fn with_responder(replies: Vec<String>, responder: Responder) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
        let replies = Arc::new(Mutex::new(VecDeque::from(replies)));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (requests, replies, stop) = (requests.clone(), replies.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => serve(stream, &requests, &replies, &responder),
                        Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
                    }
                }
            })
        };
        Self {
            url,
            requests,
            replies,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> (Vec<String>, usize) {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        let requests = self.requests.lock().unwrap().clone();
        let unused = self.replies.lock().unwrap().len();
        (requests, unused)
    }
}

fn serve(
    mut stream: std::net::TcpStream,
    requests: &Mutex<Vec<String>>,
    replies: &Mutex<VecDeque<String>>,
    responder: &Responder,
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
    let body = String::from_utf8_lossy(&bytes[body_start..]).to_string();
    let index = {
        let mut requests = requests.lock().unwrap();
        requests.push(body.clone());
        requests.len()
    };
    let reply = responder(index, &body)
        .or_else(|| replies.lock().unwrap().pop_front())
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

fn tool_reply(calls: &[(&str, &str)]) -> String {
    let calls = calls
        .iter()
        .enumerate()
        .map(|(index, (name, raw))| {
            json!({
                "id": format!("call-{index}"),
                "type": "function",
                "function": {"name": name, "arguments": raw},
            })
        })
        .collect::<Vec<_>>();
    json!({"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": calls}}]})
        .to_string()
}

fn profile_manifest(profile: &str, declaration: &str) -> String {
    format!(
        "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Commit {profile}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n{declaration}"
    )
}

fn declaration(builtins: &str, tool_id: &str, effect: &str) -> String {
    format!(
        "[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [{builtins}]\ntools = [\n  {{ id = \"{tool_id}\", effect = \"{effect}\" }},\n]\n"
    )
}

fn extension() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        for (profile, declared) in [
            (
                FLAT_PROFILE,
                declaration(r#""Read", "Write""#, FLAT_TOOL, "read"),
            ),
            (
                CONFIG_PROFILE,
                declaration(r#""Read""#, CONFIG_TOOL_ID, "propose-artifacts"),
            ),
            (
                INDEX_PROFILE,
                declaration(r#""Read""#, INDEX_TOOL_ID, "propose-artifacts"),
            ),
            (PACK_HOST, String::new()),
        ] {
            let directory = root.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("manifest.toml"),
                profile_manifest(profile, &declared),
            )
            .unwrap();
        }
        let eval = format!(
            "schema_version: commandagent.pack.eval/v0\npack:\n  id: {COMMIT_PACK}\n  version: 1.0.0\n  profile: {PACK_HOST}\n  intent: create\nprofile_tools:\n  schema_version: profile-tool.declaration/v1\n  builtin_tools: [Read]\n  tools:\n    - id: {CONFIG_TOOL_ID}\n      effect: propose-artifacts\n"
        );
        let pack = root.join(format!("packs/{COMMIT_PACK}/1.0.0"));
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(pack.join("eval.yaml"), &eval).unwrap();
        std::fs::write(
            pack.join("pack.sha256"),
            format!(
                "{}\n",
                commandagent::planner::pack::exact_byte_hash(None, Some(eval.as_bytes()))
            ),
        )
        .unwrap();
        commandagent::planner::extension_profiles::register(&root).unwrap();
        register_read_executor(FLAT_RUNTIME, Arc::new(FlatReader));
        root
    })
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

struct FlatReader;

impl std::fmt::Debug for FlatReader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("FlatReader")
    }
}

impl ProfileToolReadExecutor for FlatReader {
    fn read(&self, _workspace_root: &Path, arguments: &Value) -> anyhow::Result<ReadToolOutput> {
        Ok(ReadToolOutput {
            text: format!("flat read of {arguments}"),
            usage: None,
        })
    }
}

fn flat_registry() -> tempfile::TempDir {
    let registry = tempfile::tempdir().unwrap();
    let directory = registry.path().join("profile-tools");
    std::fs::create_dir_all(&directory).unwrap();
    let descriptor = json!({
        "protocol": "profile-tool.descriptor/v1",
        "tool_id": FLAT_TOOL,
        "name": "ReadFlatConfig",
        "version": "1.0.0",
        "description": "Read one configuration object.",
        "effect": "read",
        "parameters": serde_json::from_str::<Value>(FLAT_PARAMETERS).unwrap(),
        "schema_sha256": sha256(FLAT_PARAMETERS.as_bytes()),
        "runtime": {"kind": "in_process", "id": FLAT_RUNTIME},
        "limits": {"max_input_bytes": 1024, "max_output_bytes": 4096},
    });
    std::fs::write(
        directory.join(format!("{FLAT_TOOL}.json")),
        descriptor.to_string(),
    )
    .unwrap();
    registry
}

const NORMALIZED_NUMBER_FIELDS: [&str; 6] = [
    "epoch",
    "occurred_at_epoch_ms",
    "duration_ms",
    "elapsed_ms",
    "estimated_prompt_tokens_sent",
    "estimated_stable_prefix_chars",
];

fn normalize_text(paths: &[(&Path, &str)], url: &str, text: &str) -> String {
    let mut text = text.replace(url, "<URL>");
    for (path, marker) in paths {
        text = text.replace(&path.display().to_string(), marker);
    }
    for (pattern, replacement) in [
        (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
        (r"total_duration_ms=[0-9]+", "total_duration_ms=<N>"),
        (r#""epoch": ?[0-9]+"#, r#""epoch":"<N>""#),
        (
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}",
            "<ID>",
        ),
    ] {
        text = regex::Regex::new(pattern)
            .unwrap()
            .replace_all(&text, replacement)
            .into_owned();
    }
    text
}

fn normalize_value(paths: &[(&Path, &str)], url: &str, value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value =
                        if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str()) && value.is_number() {
                            json!("<N>")
                        } else {
                            normalize_value(paths, url, value)
                        };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalize_value(paths, url, item))
                .collect(),
        ),
        Value::String(text) => Value::String(normalize_text(paths, url, text)),
        other => other.clone(),
    }
}

fn artifacts(root: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap().display().to_string();
                let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).to_string();
                files.insert(relative, text);
            }
        }
    }
    files
}

struct OffRow {
    id: &'static str,
    profile: &'static str,
    text_protocol: bool,
    prompt: &'static str,
    replies: fn() -> Vec<String>,
}

const OFF_ROWS: [OffRow; 5] = [
    OffRow {
        id: "chat-native-write-and-read",
        profile: "generic",
        text_protocol: false,
        prompt: "Write notes.txt and read it.",
        replies: || {
            vec![
                tool_reply(&[("Write", r#"{"path":"notes.txt","content":"first"}"#)]),
                tool_reply(&[("Read", r#"{"path":"notes.txt"}"#)]),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "text-xml-write",
        profile: "generic",
        text_protocol: true,
        prompt: "Write notes.txt.",
        replies: || {
            vec![
                text_reply(
                    "<anvil_tool_call name=\"Write\">{\"path\":\"notes.txt\",\"content\":\"a\"}</anvil_tool_call>",
                ),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "flat-extension-positive",
        profile: FLAT_PROFILE,
        text_protocol: false,
        prompt: "Read the settings.",
        replies: || {
            vec![
                tool_reply(&[("ReadFlatConfig", r#"{"path":"settings.json"}"#)]),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "flat-extension-negative",
        profile: FLAT_PROFILE,
        text_protocol: false,
        prompt: "Read the settings.",
        replies: || {
            vec![
                tool_reply(&[("ReadFlatConfig", r#"{"path":"settings.json","extra":1}"#)]),
                tool_reply(&[("ReadFlatConfig", r#"{}"#)]),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "dummy-secret-next-to-reserved-words",
        profile: "generic",
        text_protocol: false,
        prompt: RESERVED_WORDS_PROMPT,
        replies: || {
            vec![
                tool_reply(&[(
                    "Write",
                    r#"{"path":"notes.txt","content":"result_rejected fake-dummy-credential-974-not-a-real-key commit_failed"}"#,
                )]),
                text_reply("effect_read_only done"),
            ]
        },
    },
];

fn record_off_row(row: &OffRow) -> Value {
    let workspace = tempfile::tempdir().unwrap();
    let evidence = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(workspace.path()).unwrap();
    let evidence = std::fs::canonicalize(evidence.path()).unwrap();
    std::fs::write(root.join("settings.json"), "{\"mode\":\"fast\"}").unwrap();
    let http = FakeHttp::start((row.replies)());
    let url = http.url.clone();
    let mut config = base_config(&root, &evidence, row.profile, &url);
    if row.text_protocol {
        config.tool_protocol = Some(ToolProtocol::Text);
    }
    let registry = flat_registry();
    set_env(
        REGISTRY_ENV,
        (row.profile == FLAT_PROFILE)
            .then(|| registry.path().to_str().unwrap())
            .or(None),
    );
    let mut catalog = SecretCatalog::new();
    catalog
        .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
        .unwrap();
    install_scope(catalog, Some(&root), config.eval_events_path.as_deref());
    let mut client = client_from_config(&config, false).unwrap();
    let outcome = run_session_with_outcome_with_ui(
        client.as_mut(),
        &mut SessionSnapshot::new(),
        row.prompt,
        &[],
        &config,
        &NOOP_UI,
    );
    set_env(REGISTRY_ENV, None);
    let (requests, unused) = http.finish();
    let paths = [
        (root.as_path(), "<ROOT>"),
        (evidence.as_path(), "<EVIDENCE>"),
    ];
    let outcome = match outcome {
        Ok(outcome) => json!({
            "stop_reason": format!("{:?}", outcome.stop_reason),
            "final_text": outcome.final_text,
            "iterations": outcome.iterations,
            "tool_calls": outcome.tool_calls,
            "changed_paths": outcome.changed_paths,
        }),
        Err(error) => json!({"error": normalize_text(&paths, &url, &format!("{error:#}"))}),
    };
    let requests = requests
        .iter()
        .map(|body| normalize_text(&paths, &url, body))
        .collect::<Vec<_>>();
    let events = std::fs::read_to_string(evidence.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            normalize_value(&paths, &url, &value).to_string()
        })
        .collect::<Vec<_>>();
    json!({
        "id": row.id,
        "outcome": outcome,
        "requests": requests,
        "events": events,
        "artifacts": artifacts(&root)
            .into_iter()
            .map(|(path, text)| (path, normalize_text(&paths, &url, &text)))
            .collect::<BTreeMap<_, _>>(),
        "unused_replies": unused,
    })
}

fn golden_row(id: &str) -> Value {
    let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
    golden["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no golden row {id}"))
        .clone()
}

/// Acceptance 6: the requests, events, outcome, and artifacts of the base.
#[test]
fn off_wire_events_and_artifacts_match_the_base_recording_byte_for_byte() {
    let _guard = lock();
    extension();
    for row in &OFF_ROWS {
        let recorded = record_off_row(row);
        let golden = golden_row(row.id);
        for key in ["outcome", "requests", "artifacts", "unused_replies"] {
            assert_eq!(recorded[key], golden[key], "{}: {key}", row.id);
        }
        let (new, old) = (
            recorded["events"].as_array().unwrap(),
            golden["events"].as_array().unwrap(),
        );
        for (index, (new, old)) in new.iter().zip(old).enumerate() {
            assert_eq!(new, old, "{}: event {index}", row.id);
        }
        assert_eq!(new.len(), old.len(), "{}: event count", row.id);
    }
}

#[test]
fn off_starts_nothing_and_reads_no_registry() {
    let _guard = lock();
    set_env(REGISTRY_ENV, Some("/nonexistent/operator-root"));
    let workspace = tempfile::tempdir().unwrap();
    let admitted = admit_selected(workspace.path(), "generic", None).unwrap();
    assert!(admitted.is_none());
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// The operator: a pinned shell, a fixed script, and a descriptor.
// ---------------------------------------------------------------------------

/// One executable that exists on each OS. It starts only on macOS; elsewhere the pins are
/// still real, so a refusal there is the isolation's and not a bad pin.
fn pinned_shell() -> PathBuf {
    let path = if cfg!(target_os = "macos") {
        "/bin/bash"
    } else {
        "/bin/sh"
    };
    std::fs::canonicalize(path).unwrap()
}

/// What an operator's serializer would print. Written here on its own: the host's decoder
/// is what is under test.
fn encode_base64(bytes: &[u8]) -> String {
    const SYMBOLS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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

/// The parts of a result. Each default repeats the request; a scenario replaces one.
#[derive(Clone)]
struct Fake {
    head: String,
    request_id: String,
    tool_id: String,
    runtime: String,
    revision: String,
    schema: String,
    checks: String,
    outputs: String,
    diagnostics: String,
    tail: String,
}

/// Picks the candidate from the arguments: this stands in for a serializer. Bash has no
/// digest or base64 builtin, so both candidates are computed here and written into the
/// fixed script. No part of the script comes from a model.
fn serializer_outputs(id: &str) -> String {
    format!(r#"[{{\"id\":\"{id}\",\"sha256\":\"$digest\",\"bytes_base64\":\"$encoded\"}}]"#)
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            head: String::new(),
            request_id: "$rid".to_string(),
            tool_id: "$tid".to_string(),
            runtime: "$run".to_string(),
            revision: "$rev".to_string(),
            schema: "$sch".to_string(),
            checks: r#"[{\"id\":\"shape\",\"status\":\"passed\"}]"#.to_string(),
            outputs: serializer_outputs("artifact"),
            diagnostics: "[]".to_string(),
            tail: String::new(),
        }
    }
}

impl Fake {
    fn script(&self) -> String {
        format!(
            r#"IFS= read -r request
take() {{ local pattern="\"$1\":\"([^\"]+)\""; [[ $request =~ $pattern ]] && printf -v "$2" '%s' "${{BASH_REMATCH[1]}}"; }}
take request_id rid; take tool_id tid; take runtime_sha256 run; take input_revision rev; take schema_sha256 sch
if [[ $request == *'"mode":"safe"'* ]]; then digest={safe_digest}; encoded={safe_encoded}; else digest={fast_digest}; encoded={fast_encoded}; fi
{head}
printf '%s' "{{\"protocol\":\"profile-tool.result/v1\",\"request_id\":\"{request_id}\",\"tool_id\":\"{tool_id}\",\"runtime_sha256\":\"{runtime}\",\"input_revision\":\"{revision}\",\"schema_sha256\":\"{schema}\",\"checks\":{checks},\"outputs\":{outputs},\"diagnostics\":{diagnostics}}}"
{tail}
"#,
            safe_digest = sha256(CANDIDATE_SAFE),
            safe_encoded = encode_base64(CANDIDATE_SAFE),
            fast_digest = sha256(CANDIDATE_FAST),
            fast_encoded = encode_base64(CANDIDATE_FAST),
            head = self.head,
            request_id = self.request_id,
            tool_id = self.tool_id,
            runtime = self.runtime,
            revision = self.revision,
            schema = self.schema,
            checks = self.checks,
            outputs = self.outputs,
            diagnostics = self.diagnostics,
            tail = self.tail,
        )
    }
}

struct Env {
    _workspace: tempfile::TempDir,
    _evidence: tempfile::TempDir,
    _operator: tempfile::TempDir,
    root: PathBuf,
    evidence: PathBuf,
    registry: PathBuf,
    tools: PathBuf,
    outside: PathBuf,
}

fn config_schema() -> Value {
    json!({
        "protocol": "profile-tool.schema/v1",
        "root": {
            "type": "object",
            "additionalProperties": false,
            "required": ["mode"],
            "properties": {
                "mode": {"type": "string", "enum": ["fast", "safe"]},
                "pad": {"type": "string"},
            },
        },
    })
}

#[cfg(target_os = "macos")]
fn index_schema() -> Value {
    json!({
        "protocol": "profile-tool.schema/v1",
        "root": {
            "type": "object",
            "additionalProperties": false,
            "required": ["documents"],
            "properties": {
                "documents": {
                    "type": "array",
                    "items": {"type": "string"},
                },
            },
        },
    })
}

impl Env {
    /// Sets the registry variable; the caller holds `LOCK`.
    fn new() -> Self {
        extension();
        let workspace = tempfile::tempdir().unwrap();
        let evidence = tempfile::tempdir().unwrap();
        let operator = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        let evidence_path = std::fs::canonicalize(evidence.path()).unwrap();
        let base = std::fs::canonicalize(operator.path()).unwrap();
        let (registry, tools, outside) = (
            base.join("registry"),
            base.join("tools"),
            base.join("outside"),
        );
        for directory in [&registry.join("profile-tools"), &tools, &outside] {
            std::fs::create_dir_all(directory).unwrap();
        }
        set_env(REGISTRY_ENV, registry.to_str());
        Self {
            _workspace: workspace,
            _evidence: evidence,
            _operator: operator,
            root,
            evidence: evidence_path,
            registry,
            tools,
            outside,
        }
    }

    /// The descriptor with every pin computed here. `change` takes the finished value,
    /// so a scenario can break exactly one thing.
    fn write_tool_with(
        &self,
        tool_id: &str,
        name: &str,
        schema: Value,
        output: &str,
        fake: &Fake,
        change: &dyn Fn(&mut Value),
    ) {
        let directory = self.tools.join(tool_id);
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("tool.sh");
        std::fs::write(&script, fake.script()).unwrap();
        let shell = pinned_shell();
        let mut descriptor = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": tool_id,
            "name": name,
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
                "argv": [script],
                "cwd": directory,
                "env": {"TOOL_MODE": "fixture"},
                "limits": {"timeout_ms": 10_000, "max_result_bytes": 64 * 1024},
                "outputs": [{"id": "artifact", "path": output}],
            },
        });
        change(&mut descriptor);
        std::fs::write(
            self.registry.join(format!("profile-tools/{tool_id}.json")),
            descriptor.to_string(),
        )
        .unwrap();
    }

    fn write_tool(&self, fake: &Fake) {
        self.write_changed(fake, &|_| {});
    }

    fn write_changed(&self, fake: &Fake, change: &dyn Fn(&mut Value)) {
        self.write_tool_with(
            CONFIG_TOOL_ID,
            CONFIG_TOOL,
            config_schema(),
            OUTPUT,
            fake,
            change,
        );
    }

    fn config(&self, url: &str) -> Config {
        base_config(&self.root, &self.evidence, CONFIG_PROFILE, url)
    }

    fn events(&self) -> String {
        std::fs::read_to_string(self.evidence.join("events.jsonl")).unwrap_or_default()
    }

    fn saved(&self) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(OUTPUT)).ok()
    }

    #[cfg(target_os = "macos")]
    fn with_old_bytes(&self) {
        std::fs::create_dir_all(self.root.join("config")).unwrap();
        std::fs::write(self.root.join(OUTPUT), OLD_BYTES).unwrap();
    }

    #[cfg(target_os = "macos")]
    fn assert_nothing_new_saved(&self, label: &str) {
        assert_eq!(self.saved().as_deref(), Some(OLD_BYTES), "{label}");
        let leftovers = std::fs::read_dir(self.root.join("config"))
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name() != "settings.json")
            .count();
        assert_eq!(leftovers, 0, "{label}: a temporary stayed");
        assert!(
            !self.events().contains("\"event\":\"profile_tool_commit\""),
            "{label}: a commit was recorded"
        );
    }
}

#[cfg(target_os = "macos")]
fn run_with(config: &Config, replies: Vec<String>) -> (Vec<String>, Result<Vec<String>, String>) {
    let http = FakeHttp::start(replies);
    let mut config = config.clone();
    config.lm_studio_host = http.url.clone();
    let mut client = client_from_config(&config, false).unwrap();
    let result = run_session_with_outcome_with_ui(
        client.as_mut(),
        &mut SessionSnapshot::new(),
        "Submit the object.",
        &[],
        &config,
        &NOOP_UI,
    )
    .map(|outcome| outcome.changed_paths)
    .map_err(|error| format!("{error:#}"));
    let (requests, _) = http.finish();
    (requests, result)
}

#[cfg(target_os = "macos")]
fn call(raw: &str) -> Vec<String> {
    vec![tool_reply(&[(CONFIG_TOOL, raw)]), text_reply("done")]
}

// ---------------------------------------------------------------------------
// The public entries.
// ---------------------------------------------------------------------------

const PLAN_GOAL: &str = "Submit the object";

fn step_plan(kind: &str) -> StepPlan {
    StepPlan {
        goal: PLAN_GOAL.to_string(),
        steps: vec![PlanStep {
            id: format!("step-{kind}"),
            kind: kind.to_string(),
            expected_result: "pass".to_string(),
            instruction: "Submit the object".to_string(),
            expected_paths: if kind == "implement" {
                vec![OUTPUT.to_string()]
            } else {
                Vec::new()
            },
            verify: Vec::new(),
        }],
    }
}

fn ultra_plan(profile: &str) -> UltraPlan {
    UltraPlan {
        goal: PLAN_GOAL.to_string(),
        profile: profile.to_string(),
        style: "default".to_string(),
        intent: "create".to_string(),
        phases: vec![
            UltraPhase {
                id: "implementation".to_string(),
                prompt: "Submit the object.".to_string(),
            },
            UltraPhase {
                id: "verification".to_string(),
                prompt: "Recheck the object.".to_string(),
            },
        ],
    }
}

#[cfg(target_os = "macos")]
fn step_plan_reply() -> String {
    json!({
        "goal": PLAN_GOAL,
        "steps": [{
            "id": "step-implement",
            "kind": "implement",
            "expected_result": "pass",
            "instruction": "Submit the object",
            "expected_paths": [OUTPUT],
            "verify": [],
        }],
    })
    .to_string()
}

fn write_plans(env: &Env, profile: &str) {
    std::fs::write(
        env.root.join("steps.yaml"),
        render_step_plan(&step_plan("implement")),
    )
    .unwrap();
    std::fs::write(
        env.root.join("ultra.yaml"),
        render_ultra_plan(&ultra_plan(profile)),
    )
    .unwrap();
}

struct CountingClassifier {
    calls: AtomicUsize,
}

impl PatternClassifier for CountingClassifier {
    fn classify(&self, _request: &str) -> Result<Classification, ClassifyFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Classification {
            pattern: None,
            confidence: 1.0,
            model: None,
        })
    }
}

const JEV: RequestPatternSettings = RequestPatternSettings {
    mode: RequestPatternMode::Jev,
    threshold: 0.5,
};

const SESSION_ENTRIES: &[&str] = &[
    "run_session",
    "run_session_with_required_paths",
    "run_session_with_required_paths_with_ui",
    "run_session_with_outcome_with_ui",
    "run_direct_prompt_with",
    "run_direct_prompt_with_common",
    "run_step_plan",
    "run_step_plan_with_ui",
    "run_plan_file",
    "generate_and_run_step_plan",
    "generate_and_run_step_plan_with_ui",
    "generate_and_run_ultra_plan",
    "generate_and_run_ultra_plan_with_ui",
    "run_ultra_plan",
    "run_ultra_plan_with_ui",
    "run_ultra_plan_file",
    "run_ultra_plan_file_with_ui",
    "auto_recovery::generate_and_run_with_ui",
    "auto_recovery::run_file_with_ui",
    "auto_recovery::run_plan_with_ui",
    "/run-plan steps.yaml",
    "/plan-run Submit the object",
    "/ultra-plan-run Submit the object",
    "/run-ultra-plan ultra.yaml",
    "/plan-run --profile commit-config Submit the object",
    "/ultra-plan-run --profile commit-config Submit the object",
    "profile-command:/plan-run Submit the object",
    "gate-one",
    "directive",
];

const OTHER_ENTRIES: &[&str] = &[
    "generate_step_plan",
    "generate_ultra_plan",
    "generate_ultra_plan_with_ui",
    "/plan-steps Submit the object",
    "/ultra-plan Submit the object",
    "model_probe::run_configured",
    "model_probe::run_with_output_dir",
];

fn run_entry(
    entry: &str,
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    env: &Env,
    classifier: &CountingClassifier,
) -> Result<String, String> {
    use commandagent::planner::auto_recovery;
    let prompt = "Submit the object.";
    let plan = step_plan("implement");
    let ultra = ultra_plan(&config.profile);
    let file = Path::new("ultra.yaml");
    let mut session = SessionSnapshot::new();
    let result: anyhow::Result<String> = match entry {
        "run_session" => run_session(execution, &mut session, prompt, config),
        "run_session_with_required_paths" => {
            run_session_with_required_paths(execution, &mut session, prompt, &[], config)
        }
        "run_session_with_required_paths_with_ui" => run_session_with_required_paths_with_ui(
            execution,
            &mut session,
            prompt,
            &[],
            config,
            &NOOP_UI,
        ),
        "run_session_with_outcome_with_ui" => {
            run_session_with_outcome_with_ui(execution, &mut session, prompt, &[], config, &NOOP_UI)
                .map(|outcome| outcome.final_text)
        }
        "run_direct_prompt_with" => run_direct_prompt_with(
            execution,
            &mut session,
            prompt,
            config,
            &NOOP_UI,
            JEV,
            Some(classifier),
        )
        .map(|outcome| outcome.final_text),
        "run_direct_prompt_with_common" => run_direct_prompt_with_common(
            execution,
            &mut session,
            prompt,
            config,
            &NOOP_UI,
            JEV,
            Some(classifier),
            Some("Common guidance."),
        )
        .map(|outcome| outcome.final_text),
        "run_step_plan" => run_step_plan(execution, &plan, config),
        "run_step_plan_with_ui" => run_step_plan_with_ui(execution, &plan, config, &NOOP_UI),
        "run_plan_file" => run_plan_file(execution, Path::new("steps.yaml"), config),
        "generate_step_plan" => {
            generate_step_plan(planner, PLAN_GOAL, config).map(|plan| plan.goal)
        }
        "generate_and_run_step_plan" => {
            generate_and_run_step_plan(planner, execution, PLAN_GOAL, config)
        }
        "generate_and_run_step_plan_with_ui" => {
            generate_and_run_step_plan_with_ui(planner, execution, PLAN_GOAL, config, &NOOP_UI)
        }
        "generate_ultra_plan" => {
            generate_ultra_plan(planner, PLAN_GOAL, config).map(|plan| plan.goal)
        }
        "generate_ultra_plan_with_ui" => {
            generate_ultra_plan_with_ui(planner, PLAN_GOAL, config, &NOOP_UI).map(|plan| plan.goal)
        }
        "generate_and_run_ultra_plan" => {
            generate_and_run_ultra_plan(planner, execution, PLAN_GOAL, config)
        }
        "generate_and_run_ultra_plan_with_ui" => {
            generate_and_run_ultra_plan_with_ui(planner, execution, PLAN_GOAL, config, &NOOP_UI)
        }
        "run_ultra_plan" => run_ultra_plan(planner, execution, &ultra, config),
        "run_ultra_plan_with_ui" => {
            run_ultra_plan_with_ui(planner, execution, &ultra, config, &NOOP_UI)
        }
        "run_ultra_plan_file" => run_ultra_plan_file(planner, execution, file, config),
        "run_ultra_plan_file_with_ui" => {
            run_ultra_plan_file_with_ui(planner, execution, file, config, &NOOP_UI)
        }
        "auto_recovery::generate_and_run_with_ui" => {
            auto_recovery::generate_and_run_with_ui(planner, execution, PLAN_GOAL, config, &NOOP_UI)
        }
        "auto_recovery::run_file_with_ui" => {
            auto_recovery::run_file_with_ui(planner, execution, file, config, &NOOP_UI)
        }
        "auto_recovery::run_plan_with_ui" => {
            auto_recovery::run_plan_with_ui(planner, execution, &ultra, config, &NOOP_UI)
        }
        "model_probe::run_with_output_dir" => commandagent::model_probe::run_with_output_dir(
            config,
            planner,
            execution,
            env.evidence.join("probe-output"),
        )
        .map(|_| String::new()),
        "model_probe::run_configured" => {
            commandagent::model_probe::run_configured(config, planner, execution)
                .map(|_| String::new())
        }
        "gate-one" => gate_one(planner, execution, config, env),
        "directive" => directive(planner, execution, config, env),
        line => match line.strip_prefix("profile-command:") {
            Some(command) => {
                let mut updated = config.clone();
                updated.field_sources.profile = "repl".to_string();
                updated.profile_inference = None;
                handle_command(command, &updated, planner, execution, &NOOP_UI)
            }
            None => handle_command(line, config, planner, execution, &NOOP_UI),
        },
    };
    result.map_err(|error| format!("{error:#}"))
}

fn gate_one_shell(config: &Config, env: &Env) -> (BoundaryShell, String, String) {
    let deterministic = deterministic_route(RouteRequest {
        request: PLAN_GOAL,
        workspace: &env.root,
        explicit: ExplicitRouteBinding {
            profile: Some(commandagent::planner::profile::ProfileId::parse(
                &config.profile,
            )),
            ..ExplicitRouteBinding::default()
        },
    });
    assert_eq!(deterministic.resolution, DeterministicResolution::Unique);
    let proposal = RouteProposal {
        selected: deterministic.candidates.first().cloned(),
        alternatives: deterministic.candidates,
        classifier: ClassifierProvenance {
            used: false,
            provider: "lm-studio".to_string(),
            model: "planner".to_string(),
            prompt_version: "issue974-test",
            candidate_keys: Vec::new(),
            raw_response_hash: None,
            parse_reason: "deterministic_unique".to_string(),
        },
        status: ProposalStatus::AwaitingConfirmation,
        confirmation_required: true,
    };
    let audit = env.evidence.join("boundary-events.jsonl");
    let mut shell = BoundaryShell::new(env.evidence.join("confirmations"), Some(audit));
    let identity = shell
        .begin_gate_one(
            proposal,
            PLAN_GOAL,
            &env.root,
            ExecutionPins {
                planner_provider: "lm-studio".to_string(),
                planner_model: "m".to_string(),
                executor_provider: "lm-studio".to_string(),
                executor_model: "m".to_string(),
                preset: "profile".to_string(),
                think: None,
            },
            PackSelection::None,
        )
        .unwrap()
        .clone();
    shell.confirm(&identity.card_hash().unwrap()).unwrap();
    let (profile, request) = (identity.profile.to_string(), identity.request.to_string());
    (shell, profile, request)
}

fn gate_one(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    env: &Env,
) -> anyhow::Result<String> {
    let (mut shell, profile, request) = gate_one_shell(config, env);
    let command = format!("/ultra-plan-run --profile {profile} {request}");
    shell.dispatch(|_| handle_command(&command, config, planner, execution, &NOOP_UI))
}

fn directive(
    planner: &mut dyn ChatClient,
    execution: &mut dyn ChatClient,
    config: &Config,
    env: &Env,
) -> anyhow::Result<String> {
    let (mut shell, _, _) = gate_one_shell(config, env);
    shell.dispatch(|_| Ok("failed".to_string())).unwrap();
    shell
        .present_terminal(
            "# sheet\n\n## 5. Stop reason\nfailed".to_string(),
            false,
            Some("failed".to_string()),
        )
        .unwrap();
    commandagent::eval_events::emit(
        Some(&env.evidence.join("boundary-events.jsonl")),
        json!({
            "event": "tui_command_stop",
            "ok": false,
            "status": "failed",
            "stop_reason": "fixture failure",
        }),
    );
    let directive = shell
        .begin_directive("Repair the object", "run-001", 1)
        .unwrap()
        .clone();
    let continuation = DirectiveContinuation {
        plan_path: env.root.join("ultra.yaml"),
        plan_workspace_path: "ultra.yaml".to_string(),
        target_run_id: "run-001".to_string(),
        directive_round: 1,
        directive_hash: directive.hash().to_string(),
        regression_freeze: None,
    };
    shell.confirm_directive(directive.hash()).unwrap();
    let command = format!("/run-ultra-plan {}", continuation.plan_workspace_path);
    shell.dispatch_directive(&continuation, || {
        handle_command(&command, config, planner, execution, &NOOP_UI)
    })
}

struct CountingEndpoint {
    url: String,
    count: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CountingEndpoint {
    fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (count, stop) = (count.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok(_) => {
                            count.fetch_add(1, Ordering::SeqCst);
                        }
                        Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
                    }
                }
            })
        };
        Self {
            url,
            count,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> usize {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.count.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// Refused before the start: every entry, zero calls, zero saves.
// ---------------------------------------------------------------------------

const DUMMY_ENV_SECRET: &str = "fake-dummy-env-credential-974-not-a-real-key";

/// A reason the host refuses before the start, with the kind it must report here. A row
/// that needs the strict probe to pass first is `unavailable` where the probe has no
/// backend: that is the expected value there, and is not a skip.
struct StartRefusal {
    id: &'static str,
    kind: &'static str,
    after_probe: bool,
    prepare: fn(&Env, &mut Config),
}

fn change_process(env: &Env, change: fn(&mut Value)) {
    env.write_changed(&Fake::default(), &|descriptor| {
        change(&mut descriptor["process"]);
    });
}

const START_REFUSALS: &[StartRefusal] = &[
    StartRefusal {
        id: "two outputs",
        kind: "effect_output_refused",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["outputs"] = json!([
                    {"id": "artifact", "path": "config/a.json"},
                    {"id": "second", "path": "config/b.json"},
                ]);
            });
        },
    },
    StartRefusal {
        id: "a clock one over its limit",
        kind: "effect_limit_exceeded",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["limits"]["timeout_ms"] = json!(30_001);
            });
        },
    },
    StartRefusal {
        id: "a result one over its limit",
        kind: "effect_limit_exceeded",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["limits"]["max_result_bytes"] = json!(64 * 1024 + 1);
            });
        },
    },
    StartRefusal {
        id: "a destination outside the workspace",
        kind: "effect_path_refused",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["outputs"][0]["path"] = json!("../outside.json");
            });
        },
    },
    StartRefusal {
        id: "a credential destination",
        kind: "effect_path_refused",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["outputs"][0]["path"] = json!(".env");
            });
        },
    },
    StartRefusal {
        id: "a credential input",
        kind: "effect_path_refused",
        after_probe: false,
        prepare: |env, _| {
            std::fs::write(env.root.join(".env"), "A=1\n").unwrap();
            let mut schema = config_schema();
            schema["inputs"] = json!([".env"]);
            env.write_tool_with(
                CONFIG_TOOL_ID,
                CONFIG_TOOL,
                schema,
                OUTPUT,
                &Fake::default(),
                &|_| {},
            );
        },
    },
    StartRefusal {
        id: "the executable does not match its pin",
        kind: "invalid_pin",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["executable"]["sha256"] = json!("0".repeat(64));
            });
        },
    },
    StartRefusal {
        id: "a dependency directory does not match its pin",
        kind: "invalid_pin",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["dependency_directories"][0]["sha256"] = json!("0".repeat(64));
            });
        },
    },
    StartRefusal {
        id: "a key a loader reads",
        kind: "effect_env_refused",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                process["env"]["DYLD_INSERT_LIBRARIES"] = json!("/tmp/x.dylib");
            });
        },
    },
    StartRefusal {
        id: "too many env keys",
        kind: "effect_limit_exceeded",
        after_probe: false,
        prepare: |env, _| {
            change_process(env, |process| {
                for index in 0..17 {
                    process["env"][format!("KEY_{index}")] = json!("v");
                }
            });
        },
    },
    StartRefusal {
        id: "an env value the scrub would change",
        kind: "effect_env_refused",
        after_probe: true,
        prepare: |env, _| {
            change_process(env, |process| {
                process["env"]["TOOL_TOKEN"] = json!(DUMMY_ENV_SECRET);
            });
        },
    },
    StartRefusal {
        id: "the text tool protocol",
        kind: "schema_unsupported",
        after_probe: true,
        prepare: |env, config| {
            env.write_tool(&Fake::default());
            config.tool_protocol = Some(ToolProtocol::Text);
        },
    },
    StartRefusal {
        id: "a protected destination",
        kind: "effect_path_refused",
        after_probe: true,
        prepare: |env, config| {
            env.write_tool(&Fake::default());
            let contract = env.evidence.join("contract.json");
            std::fs::write(
                &contract,
                json!({"protected_paths": ["config"]}).to_string(),
            )
            .unwrap();
            config.completion_contract_path = Some(contract);
        },
    },
    StartRefusal {
        id: "no authority to write",
        kind: "effect_read_only",
        after_probe: true,
        prepare: |env, config| {
            env.write_tool(&Fake::default());
            config.yes = false;
        },
    },
    StartRefusal {
        id: "an action that only plans",
        kind: "effect_read_only",
        after_probe: true,
        prepare: |env, config| {
            env.write_tool(&Fake::default());
            config.action = Action::PlanSteps(PLAN_GOAL.to_string());
        },
    },
    StartRefusal {
        id: "a destination that is a symlink",
        kind: "effect_path_refused",
        after_probe: true,
        prepare: |env, _| {
            env.write_tool(&Fake::default());
            std::fs::create_dir_all(env.root.join("config")).unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(env.outside.join("target"), env.root.join(OUTPUT)).unwrap();
        },
    },
    StartRefusal {
        id: "an executable its group can write",
        kind: "unavailable",
        after_probe: false,
        prepare: |env, _| {
            let copy = env.tools.join("shell-copy");
            std::fs::copy(pinned_shell(), &copy).unwrap();
            set_mode(&copy, 0o775);
            let digest = sha256(&std::fs::read(&copy).unwrap());
            env.write_changed(&Fake::default(), &|descriptor| {
                descriptor["process"]["executable"] = json!({"path": copy, "sha256": digest});
            });
        },
    },
    StartRefusal {
        id: "a file in a dependency directory others can write",
        kind: "unavailable",
        after_probe: false,
        prepare: |env, _| {
            env.write_tool(&Fake::default());
            set_mode(&env.tools.join(CONFIG_TOOL_ID).join("tool.sh"), 0o646);
        },
    },
];

fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
}

/// What is running now, by its full command line.
fn running_command_lines() -> Vec<String> {
    if Path::new("/proc/self/cmdline").exists() {
        return std::fs::read_dir("/proc")
            .unwrap()
            .flatten()
            .filter_map(|entry| std::fs::read(entry.path().join("cmdline")).ok())
            .map(|bytes| String::from_utf8_lossy(&bytes).replace('\0', " "))
            .collect();
    }
    let listing = std::process::Command::new("/bin/ps")
        .args(["-axo", "command"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&listing.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// Watches for an operator's tool by its own script's path, from outside any policy. The
/// strict probe's fixed child has no such path, so it is not counted. For this to see a
/// start, the script stays running: see [`staying_fake`].
struct LaunchWatch {
    seen: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LaunchWatch {
    fn start(env: &Env) -> Self {
        let script = env.tools.join(CONFIG_TOOL_ID).join("tool.sh");
        let script = script.to_str().unwrap().to_string();
        let seen = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (seen, stop) = (seen.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    if running_command_lines()
                        .iter()
                        .any(|line| line.contains(&script))
                    {
                        seen.fetch_add(1, Ordering::SeqCst);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
            })
        };
        Self {
            seen,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> usize {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.seen.load(Ordering::SeqCst)
    }
}

/// A tool that stays until its clock ends, so a start of it cannot be missed.
fn staying_fake() -> Fake {
    Fake {
        head: "while :; do :; done".to_string(),
        ..Fake::default()
    }
}

/// The kind this OS must report for this row.
fn expected_kind(refusal: &StartRefusal) -> &'static str {
    if refusal.after_probe && !cfg!(target_os = "macos") {
        "unavailable"
    } else {
        refusal.kind
    }
}

fn install_dummy_secret(root: &Path, config: &Config) {
    let mut catalog = SecretCatalog::new();
    catalog
        .register_credential("DUMMY_API_KEY", DUMMY_ENV_SECRET)
        .unwrap();
    install_scope(catalog, Some(root), config.eval_events_path.as_deref());
}

#[test]
fn every_entry_refuses_before_any_provider_classifier_process_or_save() {
    let _guard = lock();
    let mut failures = Vec::new();
    for refusal in START_REFUSALS {
        for entry in SESSION_ENTRIES.iter().chain(OTHER_ENTRIES) {
            let env = Env::new();
            write_plans(&env, CONFIG_PROFILE);
            let endpoint = CountingEndpoint::start();
            let mut config = env.config(&endpoint.url);
            (refusal.prepare)(&env, &mut config);
            install_dummy_secret(&env.root, &config);
            let mut planner = client_from_config(&config, true).unwrap();
            let mut execution = client_from_config(&config, false).unwrap();
            let classifier = CountingClassifier {
                calls: AtomicUsize::new(0),
            };
            let before = artifacts_without_links(&env.root);
            let outcome = run_entry(
                entry,
                planner.as_mut(),
                execution.as_mut(),
                &config,
                &env,
                &classifier,
            );
            let expected = format!("profile_tool_refused: {}", expected_kind(refusal));
            let refused = outcome
                .as_ref()
                .err()
                .is_some_and(|error| error.contains(&expected));
            let calls = endpoint.finish();
            let classified = classifier.calls.load(Ordering::SeqCst);
            let changed = artifacts_without_links(&env.root) != before;
            let probe_output = env.evidence.join("probe-output").exists();
            let started = env.events().contains("profile_tool_effect_call");
            if !refused || calls != 0 || classified != 0 || changed || probe_output || started {
                failures.push(format!(
                    "{entry} / {}: refused={refused} calls={calls} classifier={classified} changed={changed} probe={probe_output} started={started} {outcome:?}",
                    refusal.id
                ));
            }
        }
    }
    set_env(REGISTRY_ENV, None);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn artifacts_without_links(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap().flatten() {
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            let relative = path.strip_prefix(root).unwrap().display().to_string();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() {
                files.insert(relative, std::fs::read(&path).unwrap());
            } else {
                files.insert(relative, b"<link>".to_vec());
            }
        }
    }
    files
}

// ---------------------------------------------------------------------------
// The binary, a workflow, and a pack.
// ---------------------------------------------------------------------------

struct BinaryRun {
    code: Option<i32>,
    stderr: String,
    provider: usize,
    jev: usize,
}

/// The child takes only what is named here. Its output stays in the parent's assertion.
fn binary_command(root: &Path, registry: &Path, home: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .arg("--extension-root")
        .arg(extension())
        .args(["--model", "m", "--chat-retries", "0"])
        .current_dir(root)
        .env_clear()
        .env("HOME", home)
        .env(REGISTRY_ENV, registry);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    command
}

fn binary_refused(env: &Env, arguments: &[&str], extra: &[(&str, &str)]) -> BinaryRun {
    let provider = CountingEndpoint::start();
    let jev = CountingEndpoint::start();
    let home = tempfile::tempdir().unwrap();
    let mut command = binary_command(&env.root, &env.registry, home.path());
    command
        .args(["--provider", "lm-studio", "--lm-studio-host", &provider.url])
        .args(["--request-pattern", "jev"])
        .args(arguments)
        .env("JEV_API_KEY", "dummy-974-not-a-real-credential")
        .env("HTTPS_PROXY", &jev.url);
    for (name, value) in extra {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    BinaryRun {
        code: output.status.code(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        provider: provider.finish(),
        jev: jev.finish(),
    }
}

fn pack_selection_env() -> [(&'static str, String); 4] {
    let directory = extension().join(format!("packs/{COMMIT_PACK}/1.0.0"));
    let loaded = commandagent::planner::pack::load_directory(&directory).unwrap();
    [
        (
            "COMMANDAGENT_PACK_DIRECTORY",
            directory.to_str().unwrap().to_string(),
        ),
        ("COMMANDAGENT_PACK_ID", COMMIT_PACK.to_string()),
        ("COMMANDAGENT_PACK_VERSION", "1.0.0".to_string()),
        ("COMMANDAGENT_PACK_HASH", loaded.hash),
    ]
}

#[test]
fn the_binary_refuses_before_a_provider_or_jev_on_each_action_and_selection() {
    let _guard = lock();
    let env = Env::new();
    write_plans(&env, CONFIG_PROFILE);
    change_process(&env, |process| {
        process["outputs"] = json!([
            {"id": "artifact", "path": "config/a.json"},
            {"id": "second", "path": "config/b.json"},
        ]);
    });
    let pack = format!("{COMMIT_PACK}@1.0.0");
    let inherited = pack_selection_env();
    let inherited = inherited
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect::<Vec<_>>();
    let prompt = ["--prompt", "Submit the object."];
    for (label, arguments, extra) in [
        (
            "prompt",
            [&["--profile", CONFIG_PROFILE][..], &prompt[..]].concat(),
            Vec::new(),
        ),
        ("repl", vec!["--profile", CONFIG_PROFILE], Vec::new()),
        (
            "run-plan",
            vec!["--profile", CONFIG_PROFILE, "--run-plan", "steps.yaml"],
            Vec::new(),
        ),
        (
            "selected pack",
            [
                &["--profile", PACK_HOST, "--pack", pack.as_str()][..],
                &prompt[..],
            ]
            .concat(),
            Vec::new(),
        ),
        (
            "inherited pack",
            [&["--profile", PACK_HOST][..], &prompt[..]].concat(),
            inherited.clone(),
        ),
    ] {
        let run = binary_refused(&env, &arguments, &extra);
        assert_eq!(run.code, Some(2), "{label}: {}", run.stderr);
        assert!(
            run.stderr
                .contains("profile_tool_refused: effect_output_refused"),
            "{label}: {}",
            run.stderr
        );
        assert_eq!((run.provider, run.jev), (0, 0), "{label}");
        assert_eq!(env.saved(), None, "{label}");
    }
    set_env(REGISTRY_ENV, None);
}

const REPRODUCER_MARKER: &str = "reproducer-ran";

fn workflow_origin(root: &Path, child_profile: &str) -> (PathBuf, PathBuf) {
    let origin = root.join("origin");
    let run_dir = origin.join(".anvil/runs/origin-run");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::create_dir_all(origin.join(".anvil/plans")).unwrap();
    std::fs::create_dir_all(origin.join("pipeline")).unwrap();
    std::fs::write(
        run_dir.join("events.jsonl"),
        concat!(
            r#"{"event":"run_start","action":"Prompt(\"origin goal\")"}"#,
            "\n",
            r#"{"event":"verify_default_bound","bound_checks":["test -f marker"]}"#,
            "\n",
            r#"{"event":"run_stop","status":"failed"}"#,
            "\n"
        ),
    )
    .unwrap();
    std::fs::write(
        origin.join(".anvil/plans/recovery-origin.yaml"),
        "version: 1\n",
    )
    .unwrap();
    std::fs::write(
        origin.join("pipeline/main.py"),
        format!("open(\"{REPRODUCER_MARKER}\", \"w\").close()\nraise SystemExit(1)\n"),
    )
    .unwrap();
    let definition = root.join("workflow.yaml");
    std::fs::write(
        &definition,
        format!(
            "workflow: commit-child\nversion: 0.2\nentry: create\nnodes:\n  create: {{ intent: create, profile: generic }}\n  rebuild: {{ intent: create, profile: {child_profile} }}\nroutes:\n  - {{ from: create, on: failed, when: recovery_yaml_present, to: rebuild, carry: [workspace, recovery_yaml] }}\nterminal: {{}}\n"
        ),
    )
    .unwrap();
    (origin, definition)
}

#[test]
fn a_binary_workflow_child_is_refused_before_its_provider_call() {
    let _guard = lock();
    let env = Env::new();
    env.write_changed(&staying_fake(), &|descriptor| {
        descriptor["process"]["env"]["LD_PRELOAD"] = json!("/tmp/x.so");
    });
    let private = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(private.path()).unwrap();
    let outer = root.join("outer");
    let home = root.join("home");
    std::fs::create_dir_all(&outer).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let (origin, definition) = workflow_origin(&root, CONFIG_PROFILE);
    let provider = CountingEndpoint::start();
    let watch = LaunchWatch::start(&env);
    let output = binary_command(&outer, &env.registry, &home)
        .args(["--provider", "lm-studio", "--lm-studio-host", &provider.url])
        .arg("--workflow")
        .arg(&definition)
        .arg("--origin")
        .arg(&origin)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(provider.finish(), 0, "{stderr}");
    assert_eq!(watch.finish(), 0, "the tool started: {stderr}");
    assert!(!origin.join(OUTPUT).exists(), "{stderr}");
    assert!(!outer.join(OUTPUT).exists(), "{stderr}");
    let circle: Value = serde_json::from_str(
        &std::fs::read_to_string(origin.join("evidence/workflow-circle.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(circle["verdict"], "circle_failed", "{circle}");
    assert!(circle.to_string().contains("node_failed:"), "{circle}");
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_library_workflow_refuses_on_its_origin_before_the_reproducer() {
    let private = tempfile::tempdir().unwrap();
    let home = private.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    if !in_own_process(
        "a_library_workflow_refuses_on_its_origin_before_the_reproducer",
        &home,
    ) {
        return;
    }
    let _guard = lock();
    let env = Env::new();
    env.write_changed(&staying_fake(), &|descriptor| {
        descriptor["process"]["limits"]["timeout_ms"] = json!(30_001);
    });
    let root = std::fs::canonicalize(private.path()).unwrap();
    let (origin, definition) = workflow_origin(&root, CONFIG_PROFILE);
    let endpoint = CountingEndpoint::start();
    let config = env.config(&endpoint.url);
    let watch = LaunchWatch::start(&env);
    let error = commandagent::workflow::orchestrator::run_workflow(&config, &definition, &origin)
        .unwrap_err();
    let error = format!("{error:#}");
    assert!(
        error.contains("profile_tool_refused: effect_limit_exceeded"),
        "{error}"
    );
    assert_eq!(endpoint.finish(), 0);
    assert_eq!(watch.finish(), 0, "the tool started");
    assert!(!origin.join(OUTPUT).exists());
    assert_eq!(env.saved(), None);
    assert!(!origin.join(REPRODUCER_MARKER).exists());
    assert!(!origin.join("evidence/workflow-events.jsonl").exists());
    set_env(REGISTRY_ENV, None);
}

const OWN_PROCESS_ENV: &str = "COMMANDAGENT_TEST_974_OWN_PROCESS";

/// This test binary again, started without `LOCK`, with only these variables.
fn test_child(test: &str) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["--exact", test, "--nocapture"]).env_clear();
    for name in ["PATH", "TMPDIR", "RUST_BACKTRACE"] {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    command
}

fn in_own_process(test: &str, home: &Path) -> bool {
    if std::env::var_os(OWN_PROCESS_ENV).is_some() {
        return true;
    }
    let output = test_child(test)
        .env(OWN_PROCESS_ENV, "1")
        .env("HOME", home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{test} in its own process: {}\n--- stdout\n{}\n--- stderr\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    false
}

// ---------------------------------------------------------------------------
// The REPL's free text, on its pseudo-terminal.
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod repl {
    use std::fs::File;
    use std::io::{Read as _, Write as _};
    use std::os::fd::FromRawFd as _;
    use std::os::unix::process::CommandExt as _;
    use std::path::Path;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::{
        CONFIG_PROFILE, Env, Fake, FakeHttp, NOOP_UI, REGISTRY_ENV, base_config, change_process,
        client_from_config, extension, handle_command, json, lock, set_env, text_reply,
    };

    const CHILD_ENV: &str = "COMMANDAGENT_TEST_974_REPL_CHILD";
    const ROOT_ENV: &str = "COMMANDAGENT_TEST_974_REPL_ROOT";
    const EVIDENCE_ENV: &str = "COMMANDAGENT_TEST_974_REPL_EVIDENCE";
    const URL_ENV: &str = "COMMANDAGENT_TEST_974_REPL_URL";
    const PROMPT: &str = "commandagent> ";
    const FREE_TEXT: &str = "Please tidy this up";
    const REFUSED: &str = "TASK FAILED";
    const TIMEOUT: Duration = Duration::from_secs(60);

    #[test]
    fn repl_child_process() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        extension();
        let var = |name: &str| std::env::var(name).unwrap();
        let mut config = base_config(
            Path::new(&var(ROOT_ENV)),
            Path::new(&var(EVIDENCE_ENV)),
            CONFIG_PROFILE,
            &var(URL_ENV),
        );
        config.profile_explicit = false;
        // SAFETY: `getppid` has no preconditions.
        let parent = unsafe { libc::getppid() };
        let started = Instant::now();
        std::thread::spawn(move || {
            loop {
                // SAFETY: as above.
                let current = unsafe { libc::getppid() };
                if current != parent || started.elapsed() > Duration::from_secs(300) {
                    std::process::exit(124);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        commandagent::tui::repl::run(config).unwrap();
    }

    struct Repl {
        child: Child,
        writer: File,
        output: Arc<Mutex<Vec<u8>>>,
        stop: Arc<AtomicBool>,
    }

    impl Drop for Repl {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    fn open_pty() -> (File, File) {
        // SAFETY: each call is checked, and each descriptor is owned by one `File`.
        unsafe {
            let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            assert!(master >= 0, "posix_openpt failed");
            assert_eq!(libc::grantpt(master), 0);
            assert_eq!(libc::unlockpt(master), 0);
            let name = libc::ptsname(master);
            assert!(!name.is_null());
            let slave = libc::open(name, libc::O_RDWR | libc::O_NOCTTY);
            assert!(slave >= 0, "the new pseudo-terminal did not open");
            let size = libc::winsize {
                ws_row: 50,
                ws_col: 400,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            assert_eq!(libc::ioctl(master, libc::TIOCSWINSZ as _, &size), 0);
            (File::from_raw_fd(master), File::from_raw_fd(slave))
        }
    }

    impl Repl {
        fn start(env: &Env, url: &str, home: &Path) -> Self {
            let (master, slave) = open_pty();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "repl::repl_child_process", "--nocapture"])
                .env_clear()
                .env(CHILD_ENV, "1")
                .env(ROOT_ENV, &env.root)
                .env(EVIDENCE_ENV, &env.evidence)
                .env(URL_ENV, url)
                .env(REGISTRY_ENV, &env.registry)
                .env("HOME", home)
                .env("TERM", "xterm")
                .stdin(Stdio::from(slave.try_clone().unwrap()))
                .stdout(Stdio::from(slave.try_clone().unwrap()))
                .stderr(Stdio::from(slave.try_clone().unwrap()));
            // SAFETY: only async-signal-safe calls, between fork and exec. The child gets
            // a new session, with this new terminal as its only controlling terminal.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = command.spawn().unwrap();
            drop(slave);
            let output: Arc<Mutex<Vec<u8>>> = Arc::default();
            let stop = Arc::new(AtomicBool::new(false));
            let writer = master.try_clone().unwrap();
            {
                let (output, stop) = (output.clone(), stop.clone());
                let mut answers = master.try_clone().unwrap();
                let mut reader = master;
                std::thread::spawn(move || {
                    let mut chunk = [0u8; 4096];
                    let mut answered = 0;
                    while !stop.load(Ordering::SeqCst) {
                        let Ok(read) = reader.read(&mut chunk) else {
                            break;
                        };
                        if read == 0 {
                            break;
                        }
                        let mut output = output.lock().unwrap();
                        output.extend_from_slice(&chunk[..read]);
                        let asked = output.windows(4).filter(|w| *w == b"\x1b[6n").count();
                        for _ in answered..asked {
                            let _ = answers.write_all(b"\x1b[1;1R");
                        }
                        answered = asked;
                    }
                });
            }
            let mut repl = Self {
                child,
                writer,
                output,
                stop,
            };
            repl.wait_for(PROMPT, 0);
            repl
        }

        fn text(&self) -> String {
            String::from_utf8_lossy(&self.output.lock().unwrap()).into_owned()
        }

        fn wait_for(&mut self, needle: &str, from: usize) -> usize {
            let started = Instant::now();
            loop {
                let text = self.text();
                if let Some(found) = text.get(from..).and_then(|tail| tail.find(needle)) {
                    return from + found + needle.len();
                }
                assert!(
                    started.elapsed() <= TIMEOUT,
                    "no `{needle}` after byte {from}:\n{text}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        fn run_line(&mut self, line: &str, marker: &str) -> String {
            std::thread::sleep(Duration::from_millis(100));
            let from = self.text().len();
            self.writer.write_all(line.as_bytes()).unwrap();
            self.writer.write_all(b"\r").unwrap();
            let end = self.wait_for(marker, from);
            let end = self.wait_for(PROMPT, end);
            self.text()[from..end].to_string()
        }
    }

    fn classifier_fake() -> FakeHttp {
        let reply = text_reply("not a route");
        FakeHttp::with_responder(Vec::new(), Arc::new(move |_, _| Some(reply.clone())))
    }

    /// The new reason stops free text before the route classifier, and the REPL goes on.
    #[test]
    fn a_repl_refuses_free_text_before_its_classifier() {
        let _guard = lock();
        type Row = (&'static str, fn(&mut serde_json::Value));
        let mut rows: Vec<Row> = vec![
            ("effect_output_refused", |descriptor| {
                descriptor["process"]["outputs"] = json!([
                    {"id": "artifact", "path": "config/a.json"},
                    {"id": "second", "path": "config/b.json"},
                ]);
            }),
            ("invalid_pin", |descriptor| {
                descriptor["process"]["executable"]["sha256"] = json!("0".repeat(64));
            }),
            ("effect_env_refused", |descriptor| {
                descriptor["process"]["env"]["NODE_OPTIONS"] = json!("--inspect");
            }),
            ("invalid_pin", |descriptor| {
                descriptor["process"]["executable"]["sha256"] = json!("not-a-digest");
            }),
            ("invalid_pin", |descriptor| {
                descriptor["process"]["executable"]["path"] = json!("relative/tool");
            }),
            ("unavailable", |descriptor| {
                descriptor["requires"] = json!(["os_sandbox"]);
            }),
        ];
        // Where no backend exists, the descriptor with nothing wrong is what stops here.
        if !cfg!(target_os = "macos") {
            rows.push(("unavailable", |_| {}));
        }
        for (kind, change) in rows {
            let env = Env::new();
            env.write_changed(&Fake::default(), &change);
            let home = tempfile::tempdir().unwrap();
            let http = classifier_fake();
            let mut repl = Repl::start(&env, &http.url, home.path());
            let before = http.requests.lock().unwrap().len();
            let shown = repl.run_line(FREE_TEXT, REFUSED);
            assert!(shown.contains(kind), "{kind}: {shown}");
            assert_eq!(
                http.requests.lock().unwrap().len(),
                before,
                "{kind}: a provider call ran before the refusal"
            );
            let shown = repl.run_line(FREE_TEXT, REFUSED);
            assert!(shown.contains(kind), "{kind}: the REPL went on: {shown}");
            drop(repl);
            http.finish();
            assert_eq!(env.saved(), None, "{kind}");
        }
        set_env(REGISTRY_ENV, None);
    }

    /// The known limit: a profile a route names is admitted at its command. Even there,
    /// the command is refused and nothing is saved.
    #[test]
    fn a_profile_named_after_the_free_text_gate_is_refused_at_its_command() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        change_process(&env, |process| {
            process["limits"]["max_result_bytes"] = json!(64 * 1024 + 1);
        });
        let endpoint = super::CountingEndpoint::start();
        let mut config = env.config(&endpoint.url);
        config.profile = "generic".to_string();
        let mut planner = client_from_config(&config, true).unwrap();
        let mut execution = client_from_config(&config, false).unwrap();
        let error = handle_command(
            "/plan-run --profile commit-config Submit the object",
            &config,
            planner.as_mut(),
            execution.as_mut(),
            &NOOP_UI,
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("profile_tool_refused: effect_limit_exceeded"),
            "{error:#}"
        );
        assert_eq!(endpoint.finish(), 0);
        assert_eq!(env.saved(), None);
        set_env(REGISTRY_ENV, None);
    }
}

// ---------------------------------------------------------------------------
// Off macOS: every entry is unavailable, with nothing started.
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod not_macos {
    use super::{
        Action, AtomicUsize, CONFIG_PROFILE, CountingClassifier, CountingEndpoint, Env, Fake,
        OTHER_ENTRIES, Ordering, REGISTRY_ENV, SESSION_ENTRIES, binary_refused, client_from_config,
        lock, run_entry, set_env, write_plans,
    };

    #[test]
    fn every_entry_expects_unavailable_with_no_provider_or_process() {
        let _guard = lock();
        let mut failures = Vec::new();
        for entry in SESSION_ENTRIES.iter().chain(OTHER_ENTRIES) {
            let env = Env::new();
            write_plans(&env, CONFIG_PROFILE);
            env.write_tool(&Fake::default());
            let endpoint = CountingEndpoint::start();
            let mut config = env.config(&endpoint.url);
            config.action = Action::Repl;
            let mut planner = client_from_config(&config, true).unwrap();
            let mut execution = client_from_config(&config, false).unwrap();
            let classifier = CountingClassifier {
                calls: AtomicUsize::new(0),
            };
            let outcome = run_entry(
                entry,
                planner.as_mut(),
                execution.as_mut(),
                &config,
                &env,
                &classifier,
            );
            let refused = outcome
                .as_ref()
                .err()
                .is_some_and(|error| error.contains("profile_tool_refused: unavailable"));
            let calls = endpoint.finish();
            if !refused
                || calls != 0
                || classifier.calls.load(Ordering::SeqCst) != 0
                || env.saved().is_some()
                || env.events().contains("profile_tool_effect_call")
            {
                failures.push(format!("{entry}: calls={calls} {outcome:?}"));
            }
        }
        set_env(REGISTRY_ENV, None);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn the_binary_expects_unavailable() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let run = binary_refused(
            &env,
            &[
                "--profile",
                CONFIG_PROFILE,
                "--yes",
                "--prompt",
                "Submit the object.",
            ],
            &[],
        );
        assert_eq!(run.code, Some(2), "{}", run.stderr);
        assert!(
            run.stderr.contains("profile_tool_refused: unavailable"),
            "{}",
            run.stderr
        );
        assert_eq!((run.provider, run.jev), (0, 0));
        assert_eq!(env.saved(), None);
        set_env(REGISTRY_ENV, None);
    }
}

// ---------------------------------------------------------------------------
// On macOS: the real policy. A failed probe fails these; nothing here skips.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::{
        Action, AtomicUsize, CANDIDATE_FAST, CANDIDATE_SAFE, COMMIT_PACK, CONFIG_PROFILE,
        CONFIG_TOOL, CountingClassifier, CountingEndpoint, Env, Fake, FakeHttp, INDEX_OUTPUT,
        INDEX_PROFILE, INDEX_TOOL, INDEX_TOOL_ID, NOOP_UI, OTHER_ENTRIES, OUTPUT, Ordering,
        PACK_HOST, PLAN_GOAL, REGISTRY_ENV, Responder, SESSION_ENTRIES, SessionSnapshot, VALID,
        Value, admit_selected, base_config, binary_command, binary_refused, call,
        client_from_config, config_schema, generate_step_plan, index_schema, json, lock,
        pack_selection_env, render_ultra_plan, run_entry, run_session_with_outcome_with_ui,
        run_with, set_env, sha256, step_plan, step_plan_reply, test_child, text_reply, tool_reply,
        ultra_plan, write_plans,
    };
    use commandagent::mode::ExecutionMode;
    use commandagent::tools::registry::ToolContext;
    use commandagent::tools::workspace_policy::WorkspacePolicy;
    use commandagent::tui::status::UiStatus;
    use commandagent::tui::{InteractionUi, UiGuard};
    use std::path::Path;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    fn receipt_from_events(env: &Env) -> Value {
        let line = env
            .events()
            .lines()
            .find(|line| line.contains("\"event\":\"profile_tool_commit\""))
            .unwrap_or_else(|| panic!("no commit event:\n{}", env.events()))
            .to_string();
        serde_json::from_str::<Value>(&line).unwrap()["receipt"].clone()
    }

    fn assert_committed(env: &Env, candidate: &[u8], path: &str, label: &str) {
        let receipt = receipt_from_events(env);
        assert_eq!(
            std::fs::read(env.root.join(path)).unwrap(),
            candidate,
            "{label}"
        );
        assert_eq!(receipt["committed"], true, "{label}");
        assert_eq!(receipt["committed_sha256"], sha256(candidate), "{label}");
        assert_eq!(receipt["committed_bytes"], candidate.len(), "{label}");
        assert_eq!(receipt["path"], path, "{label}");
        assert_eq!(receipt["output_id"], "artifact", "{label}");
    }

    /// Acceptance 1, through the session: G1 admission, G2 raw arguments, the process
    /// under the real policy, the host's checks, and the commit. Two profiles, one mechanism.
    #[test]
    fn a_config_object_and_a_document_index_commit_through_the_same_mechanism() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&Fake::default());
        let (requests, result) = run_with(&env.config("http://unused"), call(VALID));
        assert_eq!(result.unwrap(), [OUTPUT.to_string()], "from the receipt");
        assert_committed(&env, CANDIDATE_FAST, OUTPUT, "config");
        let events = env.events();
        assert!(events.contains("\"event\":\"profile_tool_effect_call\""));
        assert!(
            !events.contains("\"access\":\"read_only\""),
            "a mutation is not recorded as a read"
        );
        let tools: Value = serde_json::from_str(&requests[0]).unwrap();
        let names = tools["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            ["Read", CONFIG_TOOL],
            "Write, Edit, and Bash are gone"
        );
        let last: Value = serde_json::from_str(&requests[1]).unwrap();
        assert!(
            last["messages"]
                .to_string()
                .contains("profile-tool.commit-receipt/v1"),
            "the model got the receipt"
        );
        let (again, _) = run_with(&env.config("http://unused"), call(r#"{"mode":"safe"}"#));
        assert_eq!(again.len(), 2);
        assert_eq!(
            env.saved().as_deref(),
            Some(CANDIDATE_SAFE),
            "the serializer's bytes"
        );

        let env = Env::new();
        let fake = Fake::default();
        env.write_tool_with(
            INDEX_TOOL_ID,
            INDEX_TOOL,
            index_schema(),
            INDEX_OUTPUT,
            &fake,
            &|_| {},
        );
        let config = base_config(&env.root, &env.evidence, INDEX_PROFILE, "http://unused");
        let (_, result) = run_with(
            &config,
            vec![
                tool_reply(&[(INDEX_TOOL, r#"{"documents":["a","b"]}"#)]),
                text_reply("done"),
            ],
        );
        assert_eq!(result.unwrap(), [INDEX_OUTPUT.to_string()]);
        assert_committed(&env, CANDIDATE_FAST, INDEX_OUTPUT, "index");
        set_env(REGISTRY_ENV, None);
    }

    fn entry_responder(first_plan: Option<String>) -> Responder {
        let sent = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let first_plan = std::sync::Mutex::new(first_plan);
        Arc::new(move |_, body| {
            let carries = body.contains(&format!("\"name\":\"{CONFIG_TOOL}\""));
            Some(if !carries {
                let plan = first_plan.lock().unwrap().take();
                text_reply(&plan.unwrap_or_else(step_plan_reply))
            } else if sent.swap(true, Ordering::SeqCst) {
                text_reply("done")
            } else {
                tool_reply(&[(CONFIG_TOOL, VALID)])
            })
        })
    }

    /// Each session entry reaches a commit: what the run concludes after that is its own.
    #[test]
    fn every_session_entry_reaches_the_commit() {
        let _guard = lock();
        let mut failures = Vec::new();
        for entry in SESSION_ENTRIES {
            let env = Env::new();
            write_plans(&env, CONFIG_PROFILE);
            env.write_tool(&Fake::default());
            let generates = entry.contains("generate_and_run")
                || entry.starts_with("/ultra-plan-run")
                || *entry == "gate-one";
            let ultra =
                entry.contains("ultra") || entry.contains("auto_recovery") || *entry == "gate-one";
            let plan = (generates && ultra).then(|| render_ultra_plan(&ultra_plan(CONFIG_PROFILE)));
            let http = FakeHttp::with_responder(Vec::new(), entry_responder(plan));
            let config = env.config(&http.url);
            let mut planner = client_from_config(&config, true).unwrap();
            let mut execution = client_from_config(&config, false).unwrap();
            let classifier = CountingClassifier {
                calls: AtomicUsize::new(0),
            };
            let outcome = run_entry(
                entry,
                planner.as_mut(),
                execution.as_mut(),
                &config,
                &env,
                &classifier,
            );
            http.finish();
            if env.saved().as_deref() != Some(CANDIDATE_FAST)
                || !env.events().contains("\"event\":\"profile_tool_commit\"")
            {
                failures.push(format!("{entry}: {outcome:?}\n{}", env.events()));
            }
        }
        set_env(REGISTRY_ENV, None);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// An inspect step can only read: its session is refused before its provider call.
    #[test]
    fn an_inspect_step_is_refused_before_its_provider_call() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let http = FakeHttp::start(Vec::new());
        let config = env.config(&http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let error =
            super::run_step_plan(client.as_mut(), &step_plan("inspect"), &config).unwrap_err();
        assert!(
            format!("{error:#}").contains("effect_read_only"),
            "{error:#}"
        );
        let (requests, _) = http.finish();
        assert!(requests.is_empty(), "{requests:?}");
        assert_eq!(env.saved(), None);
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn a_planner_only_call_under_a_planning_action_is_refused() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let endpoint = CountingEndpoint::start();
        let mut config = env.config(&endpoint.url);
        config.action = Action::PlanSteps(PLAN_GOAL.to_string());
        let mut client = client_from_config(&config, true).unwrap();
        let error = generate_step_plan(client.as_mut(), PLAN_GOAL, &config).unwrap_err();
        assert!(
            format!("{error:#}").contains("effect_read_only"),
            "{error:#}"
        );
        assert_eq!(endpoint.finish(), 0);
        set_env(REGISTRY_ENV, None);
    }

    fn fake_with(change: impl FnOnce(&mut Fake)) -> Fake {
        let mut fake = Fake::default();
        change(&mut fake);
        fake
    }

    /// Acceptance 2: the run ends, the old bytes stay, and nothing retried.
    #[test]
    fn every_bad_result_saves_nothing_and_ends_the_run_without_a_retry() {
        let _guard = lock();
        let outputs = |id: &str, digest: &str, encoded: &str| {
            format!(
                r#"[{{\"id\":\"{id}\",\"sha256\":\"{digest}\",\"bytes_base64\":\"{encoded}\"}}]"#
            )
        };
        let good = r#"{\"id\":\"artifact\",\"sha256\":\"$digest\",\"bytes_base64\":\"$encoded\"}"#;
        let rows: Vec<(&str, &str, Fake)> = vec![
            (
                "wrong request",
                "request_id_mismatch",
                fake_with(|f| f.request_id = "other".into()),
            ),
            (
                "wrong tool",
                "tool_id_mismatch",
                fake_with(|f| f.tool_id = "other".into()),
            ),
            (
                "wrong runtime pin",
                "runtime_pin_mismatch",
                fake_with(|f| f.runtime = "other".into()),
            ),
            (
                "wrong input",
                "input_revision_mismatch",
                fake_with(|f| f.revision = "other".into()),
            ),
            (
                "wrong schema",
                "schema_hash_mismatch",
                fake_with(|f| f.schema = "other".into()),
            ),
            (
                "unknown output",
                "unknown_output",
                fake_with(|f| f.outputs = outputs("other", "$digest", "$encoded")),
            ),
            (
                "repeated output",
                "duplicate_output",
                fake_with(|f| f.outputs = format!("[{good},{good}]")),
            ),
            (
                "missing output",
                "missing_output",
                fake_with(|f| f.outputs = "[]".into()),
            ),
            (
                "wrong digest",
                "output_digest_mismatch",
                fake_with(|f| f.outputs = outputs("artifact", &"0".repeat(64), "$encoded")),
            ),
            (
                "failed check",
                "check_failed",
                fake_with(|f| f.checks = r#"[{\"id\":\"a\",\"status\":\"failed\"}]"#.into()),
            ),
            (
                "check not run",
                "check_not_run",
                fake_with(|f| f.checks = r#"[{\"id\":\"a\",\"status\":\"not_run\"}]"#.into()),
            ),
            (
                "unknown check",
                "check_unknown",
                fake_with(|f| f.checks = r#"[{\"id\":\"a\",\"status\":\"unknown\"}]"#.into()),
            ),
            (
                "no check",
                "no_checks",
                fake_with(|f| f.checks = "[]".into()),
            ),
            (
                "repeated key",
                "result_duplicate_key",
                fake_with(|f| f.diagnostics = r#"[],\"diagnostics\":[]"#.into()),
            ),
            (
                "self-declared acceptance",
                "result_unknown_field",
                fake_with(|f| f.diagnostics = r#"[],\"accepted\":true"#.into()),
            ),
            (
                "broken JSON",
                "result_malformed",
                fake_with(|f| f.tail = "printf 'trailing'".into()),
            ),
        ];
        for (label, code, fake) in &rows {
            let env = Env::new();
            env.with_old_bytes();
            env.write_tool(fake);
            let (requests, result) = run_with(&env.config("http://unused"), call(VALID));
            let error = result.unwrap_err();
            assert!(
                error.contains("profile_tool_refused: result_rejected"),
                "{label}: {error}"
            );
            assert!(error.contains(code), "{label}: {error}");
            assert_eq!(requests.len(), 1, "{label}: a retry would hide this");
            env.assert_nothing_new_saved(label);
        }
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn exit_0_with_nothing_and_a_nonzero_exit_with_a_valid_result_are_refused() {
        let _guard = lock();
        for (label, kind, tail) in [
            ("a nonzero exit", "exit_nonzero", "exit 3"),
            ("a signal", "signaled", "kill -9 $$"),
        ] {
            let env = Env::new();
            env.with_old_bytes();
            env.write_tool(&fake_with(|f| f.tail = tail.to_string()));
            let (_, result) = run_with(&env.config("http://unused"), call(VALID));
            let error = result.unwrap_err();
            assert!(error.contains("process_failed"), "{label}: {error}");
            assert!(error.contains(kind), "{label}: {error}");
            env.assert_nothing_new_saved(label);
        }
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&fake_with(|f| {
            f.head = "IFS= read -r ignored; exit 0".to_string();
        }));
        let (_, result) = run_with(&env.config("http://unused"), call(VALID));
        assert!(result.unwrap_err().contains("result_rejected"));
        env.assert_nothing_new_saved("exit 0 and no output");
        set_env(REGISTRY_ENV, None);
    }

    /// A malformed argument is G2's, and starts nothing: it goes back to the model.
    #[test]
    fn a_broken_or_repeated_argument_starts_nothing_and_saves_nothing() {
        let _guard = lock();
        for (raw, code) in [
            (r#"{"mode":"fast","mode":"safe"}"#, "duplicate_key"),
            (r#"{"mode":"fast""#, "malformed_json"),
            (r#"{"mode":"other"}"#, "enum_mismatch"),
            (r#"{"mode":"fast","extra":1}"#, "unknown_key"),
        ] {
            let env = Env::new();
            env.with_old_bytes();
            env.write_tool(&Fake::default());
            let (requests, result) = run_with(&env.config("http://unused"), call(raw));
            assert!(result.is_ok(), "{raw}: {result:?}");
            assert!(requests[1].contains(code), "{raw}: {}", requests[1]);
            assert!(requests[1].contains("schema_violation"), "{raw}");
            env.assert_nothing_new_saved(raw);
        }
        set_env(REGISTRY_ENV, None);
    }

    fn pad_raw(total: usize) -> String {
        let fixed = r#"{"mode":"fast","pad":""}"#.len();
        format!(r#"{{"mode":"fast","pad":"{}"}}"#, "x".repeat(total - fixed))
    }

    /// The limits of the arguments and the schema, each at its edge, on this entry too.
    #[test]
    fn the_argument_and_schema_limits_hold_at_their_edges() {
        let _guard = lock();
        for (raw, passes, label) in [
            (pad_raw(64 * 1024), true, "64 KiB"),
            (pad_raw(64 * 1024 + 1), false, "64 KiB and 1"),
        ] {
            let env = Env::new();
            env.with_old_bytes();
            env.write_tool(&Fake::default());
            let (_, result) = run_with(&env.config("http://unused"), call(&raw));
            assert!(result.is_ok(), "{label}: {result:?}");
            assert_eq!(
                env.saved().as_deref() == Some(CANDIDATE_FAST),
                passes,
                "{label}"
            );
        }
        let nested_schema = |depth: usize| {
            let mut schema =
                json!({"type": "object", "additionalProperties": false, "properties": {}});
            for _ in 1..depth {
                schema = json!({"type": "object", "additionalProperties": false, "properties": {"a": schema}});
            }
            json!({"protocol": "profile-tool.schema/v1", "root": schema})
        };
        let nested_raw = |depth: usize| {
            format!(
                "{}{{}}{}",
                r#"{"a":"#.repeat(depth - 1),
                "}".repeat(depth - 1)
            )
        };
        let env = Env::new();
        env.write_tool_with(
            super::CONFIG_TOOL_ID,
            CONFIG_TOOL,
            nested_schema(32),
            OUTPUT,
            &Fake::default(),
            &|_| {},
        );
        let (_, result) = run_with(&env.config("http://unused"), call(&nested_raw(32)));
        assert!(result.is_ok(), "depth 32: {result:?}");
        assert_eq!(env.saved().as_deref(), Some(CANDIDATE_FAST), "depth 32");
        let env = Env::new();
        env.write_tool_with(
            super::CONFIG_TOOL_ID,
            CONFIG_TOOL,
            nested_schema(33),
            OUTPUT,
            &Fake::default(),
            &|_| {},
        );
        let (requests, result) = run_with(&env.config("http://unused"), call(&nested_raw(33)));
        assert!(
            result.unwrap_err().contains("schema_limit_exceeded"),
            "depth 33"
        );
        assert!(requests.is_empty());
        let array_schema = json!({
            "protocol": "profile-tool.schema/v1",
            "root": {
                "type": "object",
                "additionalProperties": false,
                "properties": {"items": {"type": "array", "items": {"type": "integer"}}},
            },
        });
        for (count, passes) in [(510, true), (511, false)] {
            let env = Env::new();
            env.with_old_bytes();
            env.write_tool_with(
                super::CONFIG_TOOL_ID,
                CONFIG_TOOL,
                array_schema.clone(),
                OUTPUT,
                &Fake::default(),
                &|_| {},
            );
            let raw = format!(r#"{{"items":[{}]}}"#, vec!["1"; count].join(","));
            let (_, result) = run_with(&env.config("http://unused"), call(&raw));
            assert!(result.is_ok(), "{count} items: {result:?}");
            assert_eq!(
                env.saved().as_deref() == Some(CANDIDATE_FAST),
                passes,
                "{count} items + object + array"
            );
        }
        // The schema's bytes, padded to exactly the opening limit by the enum values.
        for (over, kind) in [(0usize, None), (1, Some("schema_limit_exceeded"))] {
            let env = Env::new();
            let schema = schema_of_bytes(256 * 1024 + over);
            env.write_tool_with(
                super::CONFIG_TOOL_ID,
                CONFIG_TOOL,
                schema,
                OUTPUT,
                &Fake::default(),
                &|_| {},
            );
            let refused = admit_selected(&env.root, CONFIG_PROFILE, None).err();
            match kind {
                Some(kind) => assert!(
                    refused.as_ref().is_some_and(|r| r.kind.as_str() == kind),
                    "{refused:?}"
                ),
                None => assert_eq!(refused, None),
            }
        }
        set_env(REGISTRY_ENV, None);
    }

    /// A root whose compact JSON is exactly this long, within the element limit.
    fn schema_of_bytes(total: usize) -> Value {
        let build = |values: &[String]| {
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {"mode": {"type": "string", "enum": values}},
            })
        };
        let chunk = 1000usize;
        let mut values = (0..260)
            .map(|index| format!("{index:04}{}", "v".repeat(chunk - 4)))
            .collect::<Vec<_>>();
        while serde_json::to_vec(&build(&values)).unwrap().len() > total {
            values.pop();
        }
        let current = serde_json::to_vec(&build(&values)).unwrap().len();
        let mut rest = total - current;
        while rest > 0 {
            // `,"` and `"` around a new value cost three bytes, which the value makes up.
            let last = values.last_mut().unwrap();
            let room = 1024 - last.len();
            if room > 0 {
                let take = room.min(rest);
                last.push_str(&"w".repeat(take));
                rest -= take;
            } else {
                let value = format!("{:04}x", values.len());
                let take = (value.len() + 3).min(rest);
                assert_eq!(take, value.len() + 3, "the padding fits in a new value");
                values.push(value);
                rest -= take;
            }
        }
        assert_eq!(serde_json::to_vec(&build(&values)).unwrap().len(), total);
        json!({"protocol": "profile-tool.schema/v1", "root": build(&values)})
    }

    #[test]
    fn a_result_over_its_narrowed_limit_is_stopped_and_saves_nothing() {
        let _guard = lock();
        for (label, tail) in [
            ("stdout", "while :; do printf 'xxxxxxxxxxxxxxxx'; done"),
            ("stderr", "while :; do printf 'xxxxxxxxxxxxxxxx' >&2; done"),
        ] {
            let env = Env::new();
            env.with_old_bytes();
            env.write_changed(&fake_with(|f| f.tail = tail.to_string()), &|descriptor| {
                descriptor["process"]["limits"]["max_result_bytes"] = json!(2048);
            });
            let started = Instant::now();
            let (_, result) = run_with(&env.config("http://unused"), call(VALID));
            let error = result.unwrap_err();
            assert!(error.contains("output_exceeded"), "{label}: {error}");
            assert!(started.elapsed() < Duration::from_secs(20), "{label}");
            env.assert_nothing_new_saved(label);
        }
        set_env(REGISTRY_ENV, None);
    }

    /// Acceptance 3: a process that never ends, one that never reads, and one that tries
    /// to leave something behind.
    #[test]
    fn a_stuck_process_is_stopped_at_its_clock_and_saves_nothing() {
        let _guard = lock();
        let spin = "while :; do :; done";
        let env = Env::new();
        env.with_old_bytes();
        env.write_changed(&fake_with(|f| f.head = spin.to_string()), &|descriptor| {
            descriptor["process"]["limits"]["timeout_ms"] = json!(300);
        });
        let started = Instant::now();
        let (_, result) = run_with(&env.config("http://unused"), call(VALID));
        assert!(result.unwrap_err().contains("timed_out"));
        assert!(started.elapsed() < Duration::from_secs(20));
        env.assert_nothing_new_saved("spin");
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn a_process_that_never_reads_a_large_request_does_not_hold_the_host() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        std::fs::create_dir_all(env.root.join("inputs")).unwrap();
        std::fs::write(env.root.join("inputs/large.txt"), vec![b'x'; 1 << 20]).unwrap();
        let mut schema = config_schema();
        schema["inputs"] = json!(["inputs/large.txt"]);
        let fake = Fake::default();
        env.write_tool_with(
            super::CONFIG_TOOL_ID,
            CONFIG_TOOL,
            schema,
            OUTPUT,
            &fake,
            &|descriptor| {
                descriptor["process"]["limits"]["timeout_ms"] = json!(300);
            },
        );
        // The script starts by reading a line: replace it with a spin, still pinned.
        let directory = env.tools.join(super::CONFIG_TOOL_ID);
        std::fs::write(directory.join("tool.sh"), "while :; do :; done\n").unwrap();
        repin(&env);
        let started = Instant::now();
        let (_, result) = run_with(&env.config("http://unused"), call(VALID));
        assert!(result.unwrap_err().contains("timed_out"));
        assert!(started.elapsed() < Duration::from_secs(20));
        env.assert_nothing_new_saved("stdin");
        set_env(REGISTRY_ENV, None);
    }

    fn repin(env: &Env) {
        let path = env.registry.join("profile-tools/config-submit.json");
        let mut descriptor: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let directory = env.tools.join(super::CONFIG_TOOL_ID);
        descriptor["process"]["dependency_directories"][0]["sha256"] =
            json!(super::directory_pin(&directory).unwrap());
        std::fs::write(&path, descriptor.to_string()).unwrap();
    }

    /// The policy refuses a fork, so a descendant in the same group or in a new session
    /// cannot start: the fixed script dies at the attempt, and the result is refused. What
    /// the host does when one does exist is in `process.rs`, with a real `setsid` child.
    #[test]
    fn a_process_that_tries_to_fork_gets_no_descendant_and_no_commit() {
        let _guard = lock();
        for (label, head) in [
            ("a same-group child", "( while :; do :; done ) &"),
            ("a subshell", "( : )"),
        ] {
            let env = Env::new();
            env.with_old_bytes();
            env.write_tool(&fake_with(|f| f.head = head.to_string()));
            let (_, result) = run_with(&env.config("http://unused"), call(VALID));
            assert!(result.is_err(), "{label}: {result:?}");
            let error = result.unwrap_err();
            let failed = error.contains("process_failed") || error.contains("result_rejected");
            assert!(failed, "{label}: {error}");
            env.assert_nothing_new_saved(label);
        }
        set_env(REGISTRY_ENV, None);
    }

    /// The witness for the zero in the refusal tests: the same watch sees the tool when it
    /// really starts, in the binary too.
    #[test]
    fn the_launch_watch_sees_a_tool_that_really_starts() {
        let _guard = lock();
        let env = Env::new();
        env.write_changed(&super::staying_fake(), &|descriptor| {
            descriptor["process"]["limits"]["timeout_ms"] = json!(1500);
        });
        let http = FakeHttp::with_responder(Vec::new(), entry_responder(None));
        let home = tempfile::tempdir().unwrap();
        let watch = super::LaunchWatch::start(&env);
        let output = binary_command(&env.root, &env.registry, home.path())
            .args([
                "--provider",
                "lm-studio",
                "--lm-studio-host",
                &http.url,
                "--yes",
            ])
            .args([
                "--profile",
                CONFIG_PROFILE,
                "--prompt",
                "Submit the object.",
            ])
            .output()
            .unwrap();
        http.finish();
        assert!(
            watch.finish() > 0,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        set_env(REGISTRY_ENV, None);
    }

    const NOISE: &str = r#"[{\"code\":\"note\",\"message\":\"said fake-dummy-env-credential-974-not-a-real-key\"}]"#;
    const STDERR_NOISE: &str =
        "printf 'stderr says fake-dummy-env-credential-974-not-a-real-key' >&2";

    /// Through the public session, on each way of failing: bounded words on the event, the
    /// dummy secret scrubbed, and nothing of it in the error or in what the model was sent.
    #[test]
    fn what_a_failed_process_said_stays_on_the_event_as_scrubbed_data() {
        let _guard = lock();
        let cases: Vec<(&str, Fake, u64)> = vec![
            (
                "a nonzero exit",
                fake_with(|f| {
                    f.diagnostics = NOISE.to_string();
                    f.tail = format!("{STDERR_NOISE}\nexit 3");
                }),
                10_000,
            ),
            (
                "a clock",
                fake_with(|f| {
                    f.diagnostics = NOISE.to_string();
                    f.tail = format!("{STDERR_NOISE}\nwhile :; do :; done");
                }),
                500,
            ),
            (
                "a binding",
                fake_with(|f| {
                    f.request_id = "another-request".to_string();
                    f.diagnostics = NOISE.to_string();
                    f.tail = STDERR_NOISE.to_string();
                }),
                10_000,
            ),
        ];
        for (label, fake, timeout_ms) in &cases {
            let env = Env::new();
            env.with_old_bytes();
            env.write_changed(fake, &|descriptor| {
                descriptor["process"]["limits"]["timeout_ms"] = json!(*timeout_ms);
            });
            let config = env.config("http://unused");
            super::install_dummy_secret(&env.root, &config);
            let (requests, result) = run_with(&config, call(VALID));
            let error = result.unwrap_err();
            let events = env.events();
            let line = events
                .lines()
                .find(|line| line.contains("profile_tool_effect_rejected"))
                .unwrap_or_else(|| panic!("{label}: {events}"));
            let event: Value = serde_json::from_str(line).unwrap();
            assert_eq!(event["diagnostics"][0]["code"], "note", "{label}: {line}");
            let said = event["diagnostics"][0]["message"].as_str().unwrap();
            assert!(said.starts_with("said "), "{label}: {said}");
            let tail = event["stderr_tail"].as_str().unwrap();
            assert!(tail.starts_with("stderr says "), "{label}: {tail}");
            assert!(
                !events.contains(super::DUMMY_ENV_SECRET),
                "{label}: not scrubbed"
            );
            assert!(!error.contains("said"), "{label}: {error}");
            assert!(!error.contains("stderr says"), "{label}: {error}");
            assert_eq!(requests.len(), 1, "{label}: nothing went back to the model");
            env.assert_nothing_new_saved(label);
        }
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn what_a_successful_process_said_does_not_reach_the_model() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&fake_with(|f| f.diagnostics = NOISE.to_string()));
        let config = env.config("http://unused");
        super::install_dummy_secret(&env.root, &config);
        let (requests, result) = run_with(&config, call(VALID));
        assert!(result.is_ok(), "{result:?}");
        assert!(requests[1].contains("profile-tool.commit-receipt/v1"));
        assert!(!requests[1].contains("said "), "{}", requests[1]);
        assert!(!requests[1].contains("\\\"diagnostics"), "{}", requests[1]);
        let said = receipt_from_events(&env)["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(said.starts_with("said "), "{said}");
        set_env(REGISTRY_ENV, None);
    }

    /// A dependency's file is rewritten while the process runs: the digest computed after it
    /// ended differs from the admission's, and nothing is committed.
    #[test]
    fn a_dependency_rewritten_during_the_process_stops_the_commit() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&fake_with(|f| {
            f.head = "until [[ -e $PWD/changed ]]; do :; done".to_string();
        }));
        let directory = env.tools.join(super::CONFIG_TOOL_ID);
        let script = directory.join("tool.sh").display().to_string();
        let writer = std::thread::spawn(move || {
            let started = Instant::now();
            while !super::running_command_lines()
                .iter()
                .any(|line| line.starts_with("/bin/bash") && line.contains(&script))
            {
                assert!(started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(5));
            }
            std::fs::write(directory.join("changed"), "x").unwrap();
        });
        let (_, result) = run_with(&env.config("http://unused"), call(VALID));
        writer.join().unwrap();
        let error = result.unwrap_err();
        assert!(error.contains("invalid_pin"), "{error}");
        env.assert_nothing_new_saved("a rewritten dependency");
        set_env(REGISTRY_ENV, None);
    }

    struct CancelUi {
        interrupted: Box<dyn Fn() -> bool>,
    }

    impl InteractionUi for CancelUi {
        fn before_model_call(&self, _label: &str) -> UiGuard {
            UiGuard::noop()
        }
        fn before_tool_call(&self, _name: &str) -> UiGuard {
            UiGuard::noop()
        }
        fn publish_status(&self, _status: UiStatus) {}
        fn interrupted(&self) -> bool {
            (self.interrupted)()
        }
    }

    fn with_ui(env: &Env, interrupted: Box<dyn Fn() -> bool>) -> Result<Vec<String>, String> {
        let http = FakeHttp::start(call(VALID));
        let config = env.config(&http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let result = run_session_with_outcome_with_ui(
            client.as_mut(),
            &mut SessionSnapshot::new(),
            "Submit the object.",
            &[],
            &config,
            &CancelUi { interrupted },
        )
        .map(|outcome| outcome.changed_paths)
        .map_err(|error| format!("{error:#}"));
        http.finish();
        result
    }

    /// Through the public session, with the real process, result, and events: the cancel
    /// reaches the process while it runs, and the old bytes stay.
    #[test]
    fn a_cancel_during_the_process_stops_it_and_saves_nothing() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&fake_with(|f| f.head = "while :; do :; done".to_string()));
        // Fires only once the fixed script is really running, by its unique path.
        let script = env.tools.join(super::CONFIG_TOOL_ID).join("tool.sh");
        let started = Instant::now();
        let result = with_ui(
            &env,
            Box::new(move || {
                std::thread::sleep(Duration::from_millis(5));
                let listing = std::process::Command::new("/bin/ps")
                    .args(["-axo", "command"])
                    .output()
                    .unwrap();
                String::from_utf8_lossy(&listing.stdout)
                    .lines()
                    .any(|line| {
                        line.starts_with("/bin/bash") && line.contains(script.to_str().unwrap())
                    })
            }),
        );
        assert!(result.is_err(), "{result:?}");
        assert!(
            env.events().contains("process ended with `cancelled`"),
            "the cancel reached the process: {}",
            env.events()
        );
        assert!(started.elapsed() < Duration::from_secs(10));
        env.assert_nothing_new_saved("cancel");
        set_env(REGISTRY_ENV, None);
    }

    /// The cancel turns true the moment the destination holds the new bytes. The commit
    /// stands: the file, the event, and the receipt agree, whatever the run says afterwards.
    #[test]
    fn a_cancel_after_the_commit_keeps_the_commit_and_its_receipt() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&Fake::default());
        let target = env.root.join(OUTPUT);
        let result = with_ui(
            &env,
            Box::new(move || std::fs::read(&target).is_ok_and(|bytes| bytes == CANDIDATE_FAST)),
        );
        assert_committed(&env, CANDIDATE_FAST, OUTPUT, "cancel after");
        assert_eq!(receipt_from_events(&env)["cancelled_after_commit"], true);
        drop(result);
        set_env(REGISTRY_ENV, None);
    }

    fn registry_context(env: &Env) -> ToolContext {
        ToolContext {
            root: env.root.clone(),
            mode: ExecutionMode::Act,
            auto_approve: true,
            interactive_approval: false,
            offline: false,
            workspace_policy: WorkspacePolicy::for_task_request(),
            eval_events_path: Some(env.evidence.join("events.jsonl")),
            expected_paths: Vec::new(),
            protected_paths: Vec::new(),
        }
    }

    /// Acceptance 5, on the public registry: no entry guard stands in front of this.
    #[test]
    fn the_public_registry_cannot_go_around_the_guard() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        let registry = admitted.registry().unwrap();
        let arguments: Value = serde_json::from_str(VALID).unwrap();
        for builtin in ["Write", "Edit", "Bash"] {
            let error = registry
                .execute(
                    builtin,
                    &json!({"path": OUTPUT, "content": "x"}),
                    &registry_context(&env),
                )
                .unwrap_err();
            assert!(error.to_string().contains("unknown tool"), "{builtin}");
        }
        let mut plan = registry_context(&env);
        plan.mode = ExecutionMode::Plan;
        for result in [
            registry.execute(CONFIG_TOOL, &arguments, &plan),
            registry.execute_with_cancel(CONFIG_TOOL, &arguments, &plan, || false, || false),
        ] {
            let error = result.unwrap_err();
            assert!(error.to_string().contains("effect_read_only"), "{error:#}");
        }
        let mut unapproved = registry_context(&env);
        unapproved.auto_approve = false;
        let error = registry
            .execute(CONFIG_TOOL, &arguments, &unapproved)
            .unwrap_err();
        assert!(error.to_string().contains("effect_read_only"), "{error:#}");
        let mut protected = registry_context(&env);
        protected.protected_paths = vec!["config".to_string()];
        let error = registry
            .execute(CONFIG_TOOL, &arguments, &protected)
            .unwrap_err();
        assert!(
            error.to_string().contains("effect_path_refused"),
            "{error:#}"
        );
        let error = registry
            .execute(
                CONFIG_TOOL,
                &json!({"mode": "other"}),
                &registry_context(&env),
            )
            .unwrap_err();
        assert!(error.to_string().contains("schema_violation"), "{error:#}");
        assert_eq!(env.saved(), None);
        assert!(admitted.commit_receipts().is_empty());

        let text = registry
            .execute(CONFIG_TOOL, &arguments, &registry_context(&env))
            .unwrap();
        assert_eq!(env.saved().as_deref(), Some(CANDIDATE_FAST));
        let receipt: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(receipt["committed_sha256"], sha256(CANDIDATE_FAST));
        let receipts = admitted.commit_receipts();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].path, OUTPUT);
        let runs = admitted.receipts();
        assert_eq!(runs.last().unwrap().usage, None, "unknown stays unknown");
        assert!(
            admitted.bindings()[0].runtime_sha256().is_some(),
            "the binding names its runtime"
        );
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn the_public_registry_delivers_a_cancel_and_keeps_a_commit_it_already_made() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&fake_with(|f| f.head = "while :; do :; done".to_string()));
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        let registry = admitted.registry().unwrap();
        let arguments: Value = serde_json::from_str(VALID).unwrap();
        for force in [false, true] {
            let started = Instant::now();
            let fired = || started.elapsed() > Duration::from_millis(100);
            let error = registry
                .execute_with_cancel(
                    CONFIG_TOOL,
                    &arguments,
                    &registry_context(&env),
                    || !force && fired(),
                    || force && fired(),
                )
                .unwrap_err();
            assert!(error.to_string().contains("cancelled"), "{error:#}");
            assert!(started.elapsed() < Duration::from_secs(10));
            env.assert_nothing_new_saved("registry cancel");
        }
        let env = Env::new();
        env.with_old_bytes();
        env.write_tool(&Fake::default());
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        let registry = admitted.registry().unwrap();
        let target = env.root.join(OUTPUT);
        let text = registry
            .execute_with_cancel(
                CONFIG_TOOL,
                &arguments,
                &registry_context(&env),
                || std::fs::read(&target).is_ok_and(|bytes| bytes == CANDIDATE_FAST),
                || false,
            )
            .unwrap();
        let receipt: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(receipt["cancelled_after_commit"], true);
        assert_eq!(env.saved().as_deref(), Some(CANDIDATE_FAST));
        set_env(REGISTRY_ENV, None);
    }

    /// Acceptance 4: the fixed script tries each of these and reports the shell's status.
    /// Each must have been refused, and only the host's commit changes the workspace. The
    /// `errno` itself is shown in `isolation.rs`, on the process's stderr.
    #[test]
    fn the_process_cannot_write_read_or_connect_and_only_the_host_commits() {
        let _guard = lock();
        let env = Env::new();
        env.with_old_bytes();
        std::fs::create_dir_all(env.root.join("inputs")).unwrap();
        let (input, protected, credential) = (
            env.root.join("inputs/source.txt"),
            env.root.join("inputs/frozen.txt"),
            env.root.join(".env"),
        );
        for path in [&input, &protected, &credential] {
            std::fs::write(path, "unchanged").unwrap();
        }
        let secret = env.outside.join("home/.ssh/id_dummy");
        std::fs::create_dir_all(secret.parent().unwrap()).unwrap();
        std::fs::write(&secret, "dummy-secret").unwrap();
        let outside = env.outside.join("written-by-the-process");
        let in_workspace = env.root.join("written-by-the-process");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let target = env.root.join(OUTPUT);
        let head = format!(
            r#"attempt() {{ eval "$1" 2>/dev/null; codes="$codes$?,"; }}
codes=""
attempt '{{ : > {outside}; }}'
attempt '{{ : > {in_workspace}; }}'
attempt '{{ echo x >> {target}; }}'
attempt '{{ echo x >> {input}; }}'
attempt '{{ echo x >> {protected}; }}'
attempt '{{ echo x >> {credential}; }}'
attempt '{{ read -r leaked < {secret}; }}'
attempt '{{ read -r leaked < {credential}; }}'
attempt '{{ : > /dev/tcp/127.0.0.1/{port}; }}'
attempt '{{ : > /dev/udp/127.0.0.1/{port}; }}'
attempt '{{ : > /dev/tcp/192.0.2.1/80; }}'
"#,
            outside = outside.display(),
            in_workspace = in_workspace.display(),
            target = target.display(),
            input = input.display(),
            protected = protected.display(),
            credential = credential.display(),
            secret = secret.display(),
        );
        let fake = fake_with(|f| {
            f.head = head;
            f.diagnostics =
                r#"[{\"code\":\"attempts\",\"message\":\"$codes leaked=[$leaked] parent=[$G3_PARENT_DUMMY] home=[${HOME-unset}] mode=[$TOOL_MODE]\"}]"#.to_string();
        });
        let mut schema = config_schema();
        schema["inputs"] = json!(["inputs/source.txt"]);
        env.write_tool_with(
            super::CONFIG_TOOL_ID,
            CONFIG_TOOL,
            schema,
            OUTPUT,
            &fake,
            &|_| {},
        );
        set_env("G3_PARENT_DUMMY", Some("dummy-parent-secret"));
        let (requests, result) = run_with(&env.config("http://unused"), call(VALID));
        set_env("G3_PARENT_DUMMY", None);
        assert!(result.is_ok(), "{result:?}");
        assert_committed(&env, CANDIDATE_FAST, OUTPUT, "the host's commit");
        let message = receipt_from_events(&env)["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            message.starts_with("1,1,1,1,1,1,1,1,1,1,1, "),
            "every attempt was refused: {message}"
        );
        assert!(message.contains("leaked=[]"), "{message}");
        assert!(message.contains("parent=[]"), "{message}");
        assert!(message.contains("home=[unset]"), "{message}");
        assert!(message.contains("mode=[fixture]"), "{message}");
        assert!(!outside.exists());
        assert!(!in_workspace.exists());
        for path in [&input, &protected, &credential] {
            assert_eq!(std::fs::read_to_string(path).unwrap(), "unchanged");
        }
        assert!(
            listener.accept().is_err(),
            "a connection reached the listener"
        );
        assert!(!requests.is_empty());
        set_env(REGISTRY_ENV, None);
    }

    /// The fixed env: only what is declared, and a changed value is a pin mismatch.
    #[test]
    fn a_changed_env_value_after_admission_is_a_pin_mismatch() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        let registry = admitted.registry().unwrap();
        let path = env.registry.join("profile-tools/config-submit.json");
        let mut descriptor: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        descriptor["process"]["env"]["TOOL_MODE"] = json!("changed");
        std::fs::write(&path, descriptor.to_string()).unwrap();
        let arguments: Value = serde_json::from_str(VALID).unwrap();
        let error = registry
            .execute(CONFIG_TOOL, &arguments, &registry_context(&env))
            .unwrap_err();
        assert!(error.to_string().contains("invalid_pin"), "{error:#}");
        assert_eq!(env.saved(), None);
        set_env(REGISTRY_ENV, None);
    }

    /// The host's side of a path race, injected while the provider answers: the start's
    /// checks passed, and the commit's own walk and checks refuse.
    fn raced(label: &str, kind: &str, swap: fn(&Path, &Path)) {
        let env = Env::new();
        env.write_tool(&Fake::default());
        std::fs::create_dir_all(env.root.join("config")).unwrap();
        let replies = Arc::new(std::sync::Mutex::new(call(VALID)));
        let swapped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (root, outside) = (env.root.clone(), env.outside.clone());
        let responder: Responder = {
            let (replies, swapped) = (replies.clone(), swapped.clone());
            Arc::new(move |_, _| {
                if !swapped.swap(true, Ordering::SeqCst) {
                    swap(&root, &outside);
                }
                Some(replies.lock().unwrap().remove(0))
            })
        };
        let http = FakeHttp::with_responder(Vec::new(), responder);
        let config = env.config(&http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let result = run_session_with_outcome_with_ui(
            client.as_mut(),
            &mut SessionSnapshot::new(),
            "Submit the object.",
            &[],
            &config,
            &NOOP_UI,
        );
        http.finish();
        let error = format!(
            "{:#}",
            result
                .err()
                .unwrap_or_else(|| anyhow::anyhow!("no refusal"))
        );
        assert!(error.contains(kind), "{label}: {error}");
        assert!(
            !env.events().contains("\"event\":\"profile_tool_commit\""),
            "{label}"
        );
        assert!(
            std::fs::read_dir(&env.outside)
                .unwrap()
                .flatten()
                .all(|entry| {
                    std::fs::read(entry.path()).map_or(true, |bytes| bytes != CANDIDATE_FAST)
                }),
            "{label}: the candidate reached the outside"
        );
    }

    #[test]
    fn a_path_swapped_after_the_start_is_refused_by_the_commit() {
        let _guard = lock();
        raced("a symlink leaf", "effect_path_refused", |root, outside| {
            std::os::unix::fs::symlink(outside.join("target"), root.join(OUTPUT)).unwrap();
        });
        raced(
            "a parent replaced by a symlink",
            "effect_path_refused",
            |root, outside| {
                std::fs::remove_dir(root.join("config")).unwrap();
                std::os::unix::fs::symlink(outside, root.join("config")).unwrap();
            },
        );
        raced("a hard link", "effect_path_refused", |root, outside| {
            let other = outside.join("other-name");
            std::fs::write(&other, "unchanged").unwrap();
            std::fs::hard_link(&other, root.join(OUTPUT)).unwrap();
        });
        raced("a directory", "effect_path_refused", |root, _| {
            std::fs::create_dir(root.join(OUTPUT)).unwrap();
        });
        raced("a FIFO", "effect_path_refused", |root, _| {
            let name = std::ffi::CString::new(root.join(OUTPUT).to_str().unwrap()).unwrap();
            // SAFETY: `name` is a NUL-terminated path that outlives the call.
            assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        });
        raced("a socket", "effect_path_refused", |root, _| {
            // The listener stays bound for this run: a name is what the commit meets.
            let socket = std::os::unix::net::UnixListener::bind(root.join(OUTPUT)).unwrap();
            std::mem::forget(socket);
        });
        set_env(REGISTRY_ENV, None);
    }

    /// Acceptance 1 and the probe's other side: the binary starts the process too.
    #[test]
    fn the_binary_commits_on_prompt_run_plan_and_each_pack_selection() {
        let _guard = lock();
        let pack = format!("{COMMIT_PACK}@1.0.0");
        let inherited = pack_selection_env();
        let prompt = vec!["--prompt", "Submit the object."];
        let cases: Vec<(&str, Vec<&str>, bool)> = vec![
            (
                "prompt",
                [vec!["--profile", CONFIG_PROFILE], prompt.clone()].concat(),
                false,
            ),
            (
                "run-plan",
                vec!["--profile", CONFIG_PROFILE, "--run-plan", "steps.yaml"],
                false,
            ),
            (
                "selected pack",
                [
                    vec!["--profile", PACK_HOST, "--pack", pack.as_str()],
                    prompt.clone(),
                ]
                .concat(),
                false,
            ),
            (
                "inherited pack",
                [vec!["--profile", PACK_HOST], prompt.clone()].concat(),
                true,
            ),
        ];
        for (label, arguments, inherits) in cases {
            let env = Env::new();
            write_plans(&env, CONFIG_PROFILE);
            env.write_tool(&Fake::default());
            let http = FakeHttp::with_responder(Vec::new(), entry_responder(None));
            let home = tempfile::tempdir().unwrap();
            let mut command = binary_command(&env.root, &env.registry, home.path());
            command
                .args([
                    "--provider",
                    "lm-studio",
                    "--lm-studio-host",
                    &http.url,
                    "--yes",
                ])
                .args(&arguments);
            if inherits {
                for (name, value) in &inherited {
                    command.env(name, value);
                }
            }
            let output = command.output().unwrap();
            http.finish();
            assert_eq!(
                env.saved().as_deref(),
                Some(CANDIDATE_FAST),
                "{label}: {}\n--- stdout\n{}\n--- stderr\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn the_binary_refuses_an_explicit_read_only_policy_before_any_call() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let run = binary_refused(
            &env,
            &[
                "--profile",
                CONFIG_PROFILE,
                "--allow",
                "read",
                "--prompt",
                "Submit the object.",
            ],
            &[],
        );
        assert_eq!(run.code, Some(2), "{}", run.stderr);
        assert!(run.stderr.contains("effect_read_only"), "{}", run.stderr);
        assert_eq!((run.provider, run.jev), (0, 0), "{}", run.stderr);
        set_env(REGISTRY_ENV, None);
    }

    const OUTER_SANDBOX: &str = "(version 1)(allow default)";
    const UNDER_OUTER_ENV: &str = "COMMANDAGENT_TEST_974_UNDER_OUTER_SANDBOX";

    /// A real failure of the strict probe, with nothing in production to ask for it: a
    /// second `sandbox-exec` cannot start under a first one.
    fn under_outer_sandbox(arguments: &[&std::ffi::OsStr]) -> std::process::Command {
        let mut command = std::process::Command::new("/usr/bin/sandbox-exec");
        command.args(["-p", OUTER_SANDBOX]).args(arguments);
        command
    }

    #[test]
    fn the_binary_is_unavailable_when_the_strict_probe_really_fails() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(&Fake::default());
        let provider = CountingEndpoint::start();
        let home = tempfile::tempdir().unwrap();
        let output = under_outer_sandbox(&[env!("CARGO_BIN_EXE_commandagent").as_ref()])
            .arg("--extension-root")
            .arg(super::extension())
            .args(["--model", "m", "--chat-retries", "0", "--yes"])
            .args(["--provider", "lm-studio", "--lm-studio-host", &provider.url])
            .args([
                "--profile",
                CONFIG_PROFILE,
                "--prompt",
                "Submit the object.",
            ])
            .current_dir(&env.root)
            .env_clear()
            .env("HOME", home.path())
            .env(REGISTRY_ENV, &env.registry)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        assert_eq!(output.status.code(), Some(2), "{stderr}");
        assert!(
            stderr.contains("profile_tool_refused: unavailable"),
            "{stderr}"
        );
        assert!(stderr.contains("strict isolation probe"), "{stderr}");
        assert_eq!(provider.finish(), 0);
        assert_eq!(env.saved(), None);
        set_env(REGISTRY_ENV, None);
    }

    /// The same, for every library entry, in a child of this test binary.
    #[test]
    fn every_library_entry_is_unavailable_when_the_strict_probe_really_fails() {
        let name = "macos::every_library_entry_is_unavailable_when_the_strict_probe_really_fails";
        if std::env::var_os(UNDER_OUTER_ENV).is_none() {
            let home = tempfile::tempdir().unwrap();
            let child = test_child(name);
            let mut command = under_outer_sandbox(&[child.get_program()]);
            command
                .args(child.get_args())
                .env_clear()
                .env(UNDER_OUTER_ENV, "1")
                .env("HOME", home.path());
            for (key, value) in child.get_envs() {
                if let Some(value) = value {
                    command.env(key, value);
                }
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}\n--- stdout\n{}\n--- stderr\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("test result: ok. 1 passed"),
                "the child ran the test"
            );
            return;
        }
        let _guard = lock();
        let mut failures = Vec::new();
        for entry in SESSION_ENTRIES.iter().chain(OTHER_ENTRIES) {
            let env = Env::new();
            write_plans(&env, CONFIG_PROFILE);
            env.write_tool(&Fake::default());
            let endpoint = CountingEndpoint::start();
            let config = env.config(&endpoint.url);
            let mut planner = client_from_config(&config, true).unwrap();
            let mut execution = client_from_config(&config, false).unwrap();
            let classifier = CountingClassifier {
                calls: AtomicUsize::new(0),
            };
            let outcome = run_entry(
                entry,
                planner.as_mut(),
                execution.as_mut(),
                &config,
                &env,
                &classifier,
            );
            let refused = outcome
                .as_ref()
                .err()
                .is_some_and(|error| error.contains("profile_tool_refused: unavailable"));
            let calls = endpoint.finish();
            if !refused
                || calls != 0
                || classifier.calls.load(Ordering::SeqCst) != 0
                || env.saved().is_some()
            {
                failures.push(format!("{entry}: calls={calls} {outcome:?}"));
            }
        }
        set_env(REGISTRY_ENV, None);
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

// ---------------------------------------------------------------------------
// The corpus.
// ---------------------------------------------------------------------------

#[test]
fn the_corpus_manifest_pins_its_fixtures() {
    let manifest: Value = serde_json::from_str(include_str!(
        "corpus/apps/profile-tool-commit/source-manifest.json"
    ))
    .unwrap();
    assert_eq!(
        manifest["fixture_sha256"]["cases.json"],
        sha256(CASES.as_bytes())
    );
    assert_eq!(
        manifest["fixture_sha256"]["off-golden.json"],
        sha256(OFF_GOLDEN.as_bytes())
    );
    assert_eq!(
        manifest["base_sha"],
        "85bcca4e1b85ffc33549a908a29250fcf87dd261"
    );
}

#[test]
fn the_corpus_names_what_this_binary_runs() {
    let cases: Value = serde_json::from_str(CASES).unwrap();
    let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
    let ids = |rows: &Value| {
        rows.as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    let off = OFF_ROWS
        .iter()
        .map(|row| row.id.to_string())
        .collect::<Vec<_>>();
    assert_eq!(ids(&golden["rows"]), off);
    assert_eq!(ids(&cases["off"]), off);
    let refused = cases["refused_before_start"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap().to_string(),
                row["refusal"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        refused,
        START_REFUSALS
            .iter()
            .map(|row| (row.id.to_string(), row.kind.to_string()))
            .collect::<Vec<_>>()
    );
    assert_eq!(cases["accepted"][0]["profile"], CONFIG_PROFILE);
    assert_eq!(cases["accepted"][1]["profile"], INDEX_PROFILE);
    assert_eq!(cases["accepted"][0]["output"], OUTPUT);
    assert_eq!(cases["accepted"][1]["output"], INDEX_OUTPUT);
    for row in cases["accepted"].as_array().unwrap() {
        assert_eq!(row["wires"], json!(["native"]), "{row}");
    }
    assert_eq!(
        cases["os_table"]["macos"],
        json!("the real policy: success, and the OS refuses each attempt")
    );
    assert_eq!(
        cases["os_table"]["other"],
        json!("unavailable before the start")
    );
}
