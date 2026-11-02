//! Issue #973 (G2): nested arguments checked against a schema bound to an input
//! revision, at the host's receiving boundary and again right before an execution.
//!
//! Success runs drive the real Chat Completions client against a disposable fake on
//! 127.0.0.1, because an injected client declares no schema wire and is refused. The
//! raw reply bytes are what a repeated key can only exist in. No model, key, external
//! provider, or Jev call is used. Scenarios are listed in
//! `tests/corpus/apps/profile-tool-schema/`; the off control compares with
//! `fixtures/off-golden.json`, recorded from base 29d4d369 before this feature existed.

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
use commandagent::mode::ExecutionMode;
use commandagent::planner::runner::{
    generate_and_run_step_plan, generate_and_run_step_plan_with_ui, generate_and_run_ultra_plan,
    generate_and_run_ultra_plan_with_ui, generate_step_plan, generate_ultra_plan,
    generate_ultra_plan_with_ui, run_plan_file, run_step_plan, run_step_plan_with_ui,
    run_ultra_plan, run_ultra_plan_file, run_ultra_plan_file_with_ui, run_ultra_plan_with_ui,
};
use commandagent::planner::step_plan::{PlanStep, StepPlan, render_step_plan};
use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan, render_ultra_plan};
use commandagent::providers::profile_tool_schema::ProfileToolSchemaWire;
use commandagent::providers::{AssistantReply, ChatClient, client_from_config};
use commandagent::sensitive_data::{SecretCatalog, install_scope};
use commandagent::state::{ConversationMessage, SessionSnapshot};
use commandagent::tools::profile_tools::{
    ProfileToolReadExecutor, ProfileToolSchemaCompiler, ReadToolOutput, SchemaCompileInput,
    admit_selected, register_read_executor, register_schema_compiler,
};
use commandagent::tools::registry::{ToolContext, ToolSpec};
use commandagent::tools::workspace_policy::WorkspacePolicy;
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

const OFF_GOLDEN: &str = include_str!("corpus/apps/profile-tool-schema/fixtures/off-golden.json");
const CASES: &str = include_str!("corpus/apps/profile-tool-schema/fixtures/cases.json");
const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
const FLAT_PROFILE: &str = "schema-off-flat";
const FLAT_TOOL: &str = "off-flat-read";
const FLAT_RUNTIME: &str = "off-flat-runtime";
const DUMMY_SECRET: &str = "fake-dummy-credential-973-not-a-real-key";
const FLAT_PARAMETERS: &str = r#"[{"name":"path","kind":"workspace_path","required":true,"description":"Workspace-relative file."}]"#;
const CONFIG_PROFILE: &str = "schema-config";
const INDEX_PROFILE: &str = "schema-index";
const CONFIG_TOOL_ID: &str = "config-submit";
const INDEX_TOOL_ID: &str = "index-submit";
const CONFIG_TOOL: &str = "SubmitConfig";
const INDEX_TOOL: &str = "SubmitIndex";
const RUNTIME: &str = "schema-test-runtime";
const COMPILER: &str = "schema-test-compiler";

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
// The fake provider: 127.0.0.1, one reply per request, raw bytes as given.
// ---------------------------------------------------------------------------

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

    /// The responder sees each request first; `None` takes the next scripted reply.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wire {
    Native,
    Xml,
}

const WIRES: [Wire; 2] = [Wire::Native, Wire::Xml];

impl Wire {
    fn stage(self) -> &'static str {
        match self {
            Self::Native => "received_native",
            Self::Xml => "received_xml",
        }
    }

    fn call(self, tool: &str, raw: &str) -> String {
        match self {
            Self::Native => tool_reply(&[(tool, raw)]),
            Self::Xml => text_reply(&format!(
                "<anvil_tool_call name=\"{tool}\">{raw}</anvil_tool_call>"
            )),
        }
    }

    fn configure(self, config: &mut Config) {
        if self == Self::Xml {
            config.tool_protocol = Some(ToolProtocol::Text);
        }
    }
}

// ---------------------------------------------------------------------------
// An injected client, which declares no schema wire.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct InjectedClient {
    calls: Arc<AtomicUsize>,
}

impl InjectedClient {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ChatClient for InjectedClient {
    fn label(&self) -> &str {
        "injected"
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(AssistantReply::text("done"))
    }
}

// ---------------------------------------------------------------------------
// The operator: fake executor, compiler, and descriptors.
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

fn install_reader() -> Arc<RecordingReader> {
    let reader = Arc::new(RecordingReader::default());
    register_read_executor(RUNTIME, reader.clone());
    reader
}

/// A derived enum: the allowed IDs are the lines of the adopted input bytes.
#[derive(Debug)]
struct LinesCompiler;

impl ProfileToolSchemaCompiler for LinesCompiler {
    fn compile(&self, input: &SchemaCompileInput<'_>) -> anyhow::Result<Value> {
        let ids = input
            .inputs
            .iter()
            .flat_map(|input| {
                String::from_utf8_lossy(&input.bytes)
                    .into_owned()
                    .lines()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut root = input.static_root.clone();
        root["properties"]["id"] = json!({"type": "string", "enum": ids});
        Ok(root)
    }
}

/// The same rule on other bytes: stands in for a compiler replaced after adoption.
#[derive(Debug)]
struct ReplacedCompiler;

impl ProfileToolSchemaCompiler for ReplacedCompiler {
    fn compile(&self, input: &SchemaCompileInput<'_>) -> anyhow::Result<Value> {
        let mut root = input.static_root.clone();
        root["properties"]["id"] = json!({"type": "string", "enum": ["replaced"]});
        Ok(root)
    }
}

fn closed(properties: Value) -> Value {
    json!({"type": "object", "additionalProperties": false, "properties": properties})
}

fn config_root() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["mode", "limits", "targets"],
        "properties": {
            "mode": {"type": "string", "enum": ["fast", "safe"]},
            "limits": {
                "type": "object",
                "additionalProperties": false,
                "required": ["retries"],
                "properties": {
                    "retries": {"type": "integer", "minimum": 1, "maximum": 3},
                    "ratio": {"type": "number", "minimum": 0.5, "maximum": 1.5},
                },
            },
            "targets": {
                "type": "array",
                "minItems": 1,
                "maxItems": 2,
                "items": {"anyOf": [
                    {
                        "type": "object",
                        "additionalProperties": false,
                        "required": ["host"],
                        "properties": {
                            "host": {"type": "string", "minLength": 2, "maxLength": 4, "pattern": "^[a-z]+$"},
                            "port": {"type": "integer"},
                        },
                    },
                    {"type": "null"},
                ]},
            },
            "name": {"type": "string"},
            "tool": {"type": "string"},
            "path": {"type": "string"},
            "arguments": closed(json!({"name": {"type": "string"}, "args": {"type": "string"}})),
            "enabled": {"type": "boolean"},
        },
    })
}

/// Every key here is one the shared recovery would reshape or remove.
const VALID_CONFIG: &str = r#"{"mode":"fast","limits":{"retries":3,"ratio":0.5},"targets":[{"host":"ab","port":1},null],"name":"n","tool":"t","path":"a/../b","arguments":{"name":"inner","args":"it's"},"enabled":false}"#;

fn index_root() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["documents"],
        "properties": {
            "documents": {
                "type": "array",
                "minItems": 1,
                "items": closed(json!({
                    "id": {"type": "string"},
                    "refs": {"type": "array", "items": {"type": "string"}},
                })),
            },
        },
    })
}

const VALID_INDEX: &str =
    r#"{"documents":[{"id":"doc-a","refs":["doc-b"]},{"id":"doc-b","refs":[]}]}"#;

fn schema_declaration(root: Value) -> Value {
    json!({"protocol": "profile-tool.schema/v1", "root": root})
}

fn write_descriptor(registry: &Path, tool_id: &str, name: &str, schema: &Value) {
    write_full_descriptor(registry, tool_id, name, schema, "1.0.0", &[]);
}

/// The pin is written out here, from the same compact text the operator would pin.
fn write_full_descriptor(
    registry: &Path,
    tool_id: &str,
    name: &str,
    schema: &Value,
    version: &str,
    requires: &[&str],
) {
    let descriptor = json!({
        "protocol": "profile-tool.descriptor/v1",
        "tool_id": tool_id,
        "name": name,
        "version": version,
        "description": "Submit one object for a synthetic fixture.",
        "effect": "read",
        "parameters": [],
        "schema": schema,
        "schema_sha256": sha256(serde_json::to_string(schema).unwrap().as_bytes()),
        "runtime": {"kind": "in_process", "id": RUNTIME},
        "requires": requires,
        "limits": {"max_input_bytes": 1 << 20, "max_output_bytes": 4096},
    });
    let directory = registry.join("profile-tools");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join(format!("{tool_id}.json")),
        descriptor.to_string(),
    )
    .unwrap();
}

fn profile_manifest(profile: &str, declaration: &str) -> String {
    format!(
        "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Schema {profile}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n{declaration}"
    )
}

fn declaration(builtins: &str, tool_id: &str) -> String {
    format!(
        "[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [{builtins}]\ntools = [\n  {{ id = \"{tool_id}\", effect = \"read\" }},\n]\n"
    )
}

fn extension() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        for (profile, declared) in [
            (FLAT_PROFILE, declaration(r#""Read", "Write""#, FLAT_TOOL)),
            (CONFIG_PROFILE, declaration(r#""Read""#, CONFIG_TOOL_ID)),
            (INDEX_PROFILE, declaration(r#""Read""#, INDEX_TOOL_ID)),
        ] {
            let directory = root.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("manifest.toml"),
                profile_manifest(profile, &declared),
            )
            .unwrap();
        }
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

// ---------------------------------------------------------------------------
// One workspace, evidence directory, and operator registry.
// ---------------------------------------------------------------------------

struct Env {
    _workspace: tempfile::TempDir,
    _evidence: tempfile::TempDir,
    registry: tempfile::TempDir,
    root: PathBuf,
    evidence: PathBuf,
}

impl Env {
    /// Sets the registry variable; the caller holds `LOCK`.
    fn new() -> Self {
        extension();
        install_reader();
        let workspace = tempfile::tempdir().unwrap();
        let evidence_dir = tempfile::tempdir().unwrap();
        let registry = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        let evidence = std::fs::canonicalize(evidence_dir.path()).unwrap();
        set_env(REGISTRY_ENV, registry.path().to_str());
        Self {
            _workspace: workspace,
            _evidence: evidence_dir,
            registry,
            root,
            evidence,
        }
    }

    fn with_config_tool() -> Self {
        let env = Self::new();
        env.write_tool(schema_declaration(config_root()));
        env
    }

    fn write_tool(&self, schema: Value) {
        write_descriptor(self.registry.path(), CONFIG_TOOL_ID, CONFIG_TOOL, &schema);
    }

    fn config(&self, url: &str, wire: Wire) -> Config {
        let mut config = base_config(&self.root, &self.evidence, CONFIG_PROFILE, url);
        wire.configure(&mut config);
        config
    }

    fn events(&self) -> String {
        std::fs::read_to_string(self.evidence.join("events.jsonl")).unwrap_or_default()
    }
}

struct SessionRun {
    executed: Vec<Value>,
    requests: Vec<String>,
    result: Result<String, String>,
}

impl SessionRun {
    fn last_message(&self, request: usize) -> String {
        let body: Value = serde_json::from_str(&self.requests[request]).unwrap();
        body["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

fn run_with_config(
    config: &Config,
    replies: Vec<String>,
    responder: Option<Responder>,
) -> (Arc<RecordingReader>, Vec<String>, Result<String, String>) {
    let reader = install_reader();
    let http =
        FakeHttp::with_responder(replies, responder.unwrap_or_else(|| Arc::new(|_, _| None)));
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
    .map(|outcome| outcome.final_text)
    .map_err(|error| format!("{error:#}"));
    let (requests, _) = http.finish();
    (reader, requests, result)
}

fn session(
    env: &Env,
    wire: Wire,
    replies: Vec<String>,
    responder: Option<Responder>,
) -> SessionRun {
    let (reader, requests, result) =
        run_with_config(&env.config("http://unused", wire), replies, responder);
    let executed = reader.calls.lock().unwrap().clone();
    SessionRun {
        executed,
        requests,
        result,
    }
}

fn parsed(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap()
}

/// A rejected argument: nothing ran, the run went on, and the model got bounded data
/// naming the stage and the reason. The session kept the wire it started with.
fn assert_rejected(env: &Env, wire: Wire, run: &SessionRun, code: &str, label: &str) {
    assert_eq!(
        run.executed,
        Vec::<Value>::new(),
        "{label}: the executor ran"
    );
    assert_eq!(run.requests.len(), 2, "{label}: {:?}", run.result);
    let feedback = run.last_message(1);
    let stage = format!("\"stage\":\"{}\"", wire.stage());
    assert!(feedback.contains("schema_violation"), "{label}: {feedback}");
    assert!(feedback.contains(&stage), "{label}: {feedback}");
    assert!(
        feedback.contains(&format!("\"code\":\"{code}\"")),
        "{label}: {feedback}"
    );
    assert_eq!(run.result.as_deref(), Ok("done"), "{label}");
    assert!(!env.events().contains("fallback_decision"), "{label}");
    if wire == Wire::Native {
        let body: Value = serde_json::from_str(&run.requests[1]).unwrap();
        assert!(body["tools"].is_array(), "{label}: native was turned off");
    }
}

// ---------------------------------------------------------------------------
// Acceptance 1: the same validator on both wires, with values untouched.
// ---------------------------------------------------------------------------

#[test]
fn a_valid_nested_object_array_and_union_reach_the_executor_unchanged_on_both_wires() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::with_config_tool();
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, VALID_CONFIG), text_reply("done")],
            None,
        );
        assert_eq!(run.result.as_deref(), Ok("done"), "{wire:?}");
        assert_eq!(run.executed, vec![parsed(VALID_CONFIG)], "{wire:?}");
        assert!(!env.events().contains("tool_args_recovered"), "{wire:?}");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn every_xml_framing_carries_the_arguments_unchanged() {
    let _guard = lock();
    for content in [
        format!("<anvil_tool_call name=\"{CONFIG_TOOL}\">{VALID_CONFIG}</anvil_tool_call>"),
        format!("<function name=\"{CONFIG_TOOL}\">{VALID_CONFIG}</function>"),
        format!("<function={CONFIG_TOOL}>{VALID_CONFIG}</function>"),
        format!(
            "<tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{VALID_CONFIG}}}</tool_call>"
        ),
        format!(
            "<anvil_tool_call>{{\"arguments\":{VALID_CONFIG},\"name\":\"{CONFIG_TOOL}\"}}</anvil_tool_call>"
        ),
    ] {
        let env = Env::with_config_tool();
        let run = session(
            &env,
            Wire::Xml,
            vec![text_reply(&content), text_reply("done")],
            None,
        );
        assert_eq!(run.executed, vec![parsed(VALID_CONFIG)], "{content}");
    }
    set_env(REGISTRY_ENV, None);
}

/// Each row differs from a valid object in exactly one way.
const REJECTED: &[(&str, &str, &str)] = &[
    (
        "nested missing key",
        r#"{"mode":"fast","limits":{},"targets":[null]}"#,
        "missing_required",
    ),
    (
        "top-level missing key",
        r#"{"mode":"fast","limits":{"retries":1}}"#,
        "missing_required",
    ),
    (
        "nested extra key",
        r#"{"mode":"fast","limits":{"retries":1,"x":1},"targets":[null]}"#,
        "unknown_key",
    ),
    (
        "top-level extra key",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[null],"x":1}"#,
        "unknown_key",
    ),
    (
        "wrong enum",
        r#"{"mode":"slow","limits":{"retries":1},"targets":[null]}"#,
        "enum_mismatch",
    ),
    (
        "wrong type",
        r#"{"mode":"fast","limits":{"retries":"1"},"targets":[null]}"#,
        "wrong_type",
    ),
    (
        "null for a string",
        r#"{"mode":null,"limits":{"retries":1},"targets":[null]}"#,
        "wrong_type",
    ),
    (
        "integer written as a float",
        r#"{"mode":"fast","limits":{"retries":1.0},"targets":[null]}"#,
        "wrong_type",
    ),
    (
        "union no branch",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[7]}"#,
        "union_no_match",
    ),
    (
        "union nested extra key",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[{"host":"ab","x":1}]}"#,
        "union_no_match",
    ),
    (
        "integer below",
        r#"{"mode":"fast","limits":{"retries":0},"targets":[null]}"#,
        "below_minimum",
    ),
    (
        "integer above",
        r#"{"mode":"fast","limits":{"retries":4},"targets":[null]}"#,
        "above_maximum",
    ),
    (
        "number below",
        r#"{"mode":"fast","limits":{"retries":1,"ratio":0.49},"targets":[null]}"#,
        "below_minimum",
    ),
    (
        "number above",
        r#"{"mode":"fast","limits":{"retries":1,"ratio":1.51},"targets":[null]}"#,
        "above_maximum",
    ),
    (
        "too few items",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[]}"#,
        "too_few_items",
    ),
    (
        "too many items",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[null,null,null]}"#,
        "too_many_items",
    ),
    (
        "repeated top-level key",
        r#"{"mode":"slow","mode":"fast","limits":{"retries":1},"targets":[null]}"#,
        "duplicate_key",
    ),
    (
        "repeated nested key",
        r#"{"mode":"fast","limits":{"retries":9,"retries":1},"targets":[null]}"#,
        "duplicate_key",
    ),
    (
        "repeated key written with an escape",
        "{\"mode\":\"slow\",\"\\u006dode\":\"fast\",\"limits\":{\"retries\":1},\"targets\":[null]}",
        "duplicate_key",
    ),
    (
        "repeated key inside an array",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[{"host":"ab","host":"cd"}]}"#,
        "duplicate_key",
    ),
    (
        "truncated JSON",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[null]"#,
        "malformed_json",
    ),
    (
        "single quotes",
        r#"{'mode':'fast','limits':{'retries':1},'targets':[null]}"#,
        "malformed_json",
    ),
    (
        "trailing comma",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[null],}"#,
        "malformed_json",
    ),
    (
        "bare keys",
        r#"{mode:"fast",limits:{retries:1},targets:[null]}"#,
        "malformed_json",
    ),
    (
        "garbage after the object",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[null]} garbage"#,
        "trailing_data",
    ),
    (
        "a second object",
        r#"{"mode":"fast","limits":{"retries":1},"targets":[null]}{"mode":"safe"}"#,
        "trailing_data",
    ),
    (
        "a number an f64 cannot hold",
        r#"{"mode":"fast","limits":{"retries":18446744073709551616},"targets":[null]}"#,
        "lossy_number",
    ),
    (
        "a fraction an f64 would round",
        r#"{"mode":"fast","limits":{"retries":1,"ratio":1.0000000000000001},"targets":[null]}"#,
        "lossy_number",
    ),
];

#[test]
fn every_rejected_argument_runs_nothing_and_returns_bounded_data_on_both_wires() {
    let _guard = lock();
    for wire in WIRES {
        for (label, raw, code) in REJECTED {
            let env = Env::with_config_tool();
            let run = session(
                &env,
                wire,
                vec![wire.call(CONFIG_TOOL, raw), text_reply("done")],
                None,
            );
            assert_rejected(&env, wire, &run, code, &format!("{wire:?} {label}"));
        }
    }
    set_env(REGISTRY_ENV, None);
}

/// A wrapper the shared recovery would unwrap, and a JSON string it would decode,
/// stay what they are, and so fail the schema.
#[test]
fn a_shape_the_shared_recovery_would_repair_stays_refused() {
    let _guard = lock();
    let wrapped = format!(r#"{{"payload":{VALID_CONFIG}}}"#);
    let escaped = serde_json::to_string(VALID_CONFIG).unwrap();
    for wire in WIRES {
        for (raw, code) in [
            (wrapped.as_str(), "unknown_key"),
            (escaped.as_str(), "wrong_type"),
        ] {
            let env = Env::with_config_tool();
            let run = session(
                &env,
                wire,
                vec![wire.call(CONFIG_TOOL, raw), text_reply("done")],
                None,
            );
            assert_rejected(&env, wire, &run, code, raw);
        }
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn an_xml_repair_never_rescues_a_schema_tool() {
    let _guard = lock();
    let cases = [
        (
            format!(
                "<anvil_tool_call name=\"{}\">```json\n{VALID_CONFIG}\n```</anvil_tool_call>",
                CONFIG_TOOL
            ),
            "malformed_json",
        ),
        (
            format!(
                "<tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{VALID_CONFIG}}} trailing</tool_call>"
            ),
            "trailing_data",
        ),
        (
            format!(
                "<tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{VALID_CONFIG},\"extra\":1}}</tool_call>"
            ),
            "wrapper_not_exact",
        ),
        (
            format!(
                "<tool_call>{{\"tool\":\"{CONFIG_TOOL}\",\"args\":{VALID_CONFIG}}}</tool_call>"
            ),
            "wrapper_not_exact",
        ),
        (
            format!(
                "<anvil_tool_call name=\"{}\">{VALID_CONFIG}</anvil_tool_call>",
                CONFIG_TOOL.to_ascii_lowercase()
            ),
            "tool_name_not_exact",
        ),
    ];
    for (content, code) in cases {
        let env = Env::with_config_tool();
        let run = session(
            &env,
            Wire::Xml,
            vec![text_reply(&content), text_reply("done")],
            None,
        );
        assert_rejected(&env, Wire::Xml, &run, code, &content);
        assert!(!env.events().contains("repair_applied"), "{content}");
    }
    // Without a closing tag and with a value the plain parser would take, the first
    // JSON value is still all there is; nothing is appended or cut.
    let env = Env::with_config_tool();
    let content = format!("<anvil_tool_call name=\"{CONFIG_TOOL}\">{VALID_CONFIG}");
    let run = session(&env, Wire::Xml, vec![text_reply(&content)], None);
    assert_eq!(run.executed, Vec::<Value>::new());
    assert!(run.result.is_err(), "{:?}", run.result);
    assert!(!env.events().contains("repair_applied"));
    set_env(REGISTRY_ENV, None);
}

/// The XML parser removes `<think>` blocks and pulls out inner tags before it reads a
/// body. Either would change a string inside the arguments, so the reply is refused.
#[test]
fn a_tag_inside_xml_arguments_does_not_silently_change_a_string() {
    let _guard = lock();
    for name in [
        "a<think>hidden</think>b",
        "<tool_call>{'name':'Read'}</tool_call>",
    ] {
        let env = Env::with_config_tool();
        let raw =
            json!({"mode": "fast", "limits": {"retries": 1}, "targets": [null], "name": name})
                .to_string();
        let run = session(
            &env,
            Wire::Xml,
            vec![Wire::Xml.call(CONFIG_TOOL, &raw)],
            None,
        );
        assert_eq!(run.executed, Vec::<Value>::new(), "{name}");
        let error = run.result.unwrap_err();
        assert!(error.contains("schema_violation"), "{error}");
        assert!(error.contains("tag_inside_arguments"), "{error}");
    }
    set_env(REGISTRY_ENV, None);
}

/// `SubmitConfig` with its first letter as a JSON escape: the decoded name is the tool's,
/// and the raw text does not contain it.
const ESCAPED_CONFIG_TOOL: &str = "\\u0053ubmitConfig";

fn with_string(value: &str) -> String {
    json!({"mode": "fast", "limits": {"retries": 1}, "targets": [null], "name": value}).to_string()
}

/// The checks read the decoded name of the call the parser made, not the raw text, so
/// neither the order of the keys nor an escape in the name gets past them.
#[test]
fn an_xml_reply_cannot_hide_a_schema_call_from_the_think_and_repair_checks() {
    let _guard = lock();
    let think = with_string("a<think>hidden</think>b");
    let cases = [
        (
            "arguments first, with a think block in a string",
            format!("<tool_call>{{\"arguments\":{think},\"name\":\"{CONFIG_TOOL}\"}}</tool_call>"),
            Some("tag_inside_arguments"),
        ),
        (
            "an escaped name, with trailing garbage",
            format!(
                "<tool_call>{{\"name\":\"{ESCAPED_CONFIG_TOOL}\",\"arguments\":{VALID_CONFIG}}} trailing</tool_call>"
            ),
            None,
        ),
        (
            "an escaped name, with a think block in a string",
            format!(
                "<tool_call>{{\"name\":\"{ESCAPED_CONFIG_TOOL}\",\"arguments\":{think}}}</tool_call>"
            ),
            Some("tag_inside_arguments"),
        ),
        (
            "an escaped name, with a tag in a string",
            format!(
                "<tool_call>{{\"name\":\"{ESCAPED_CONFIG_TOOL}\",\"arguments\":{}}}</tool_call>",
                with_string("<tool_call>{'name':'Read'}</tool_call>")
            ),
            // The inner closing tag ends the outer call early, so the reply does not parse.
            None,
        ),
        (
            "an escaped name, with a think block after the call",
            format!(
                "<tool_call>{{\"name\":\"{ESCAPED_CONFIG_TOOL}\",\"arguments\":{VALID_CONFIG}}}</tool_call><think>later</think>"
            ),
            Some("tag_inside_arguments"),
        ),
    ];
    for (label, content, code) in cases {
        let env = Env::with_config_tool();
        let run = session(&env, Wire::Xml, vec![text_reply(&content)], None);
        assert_eq!(run.executed, Vec::<Value>::new(), "{label}");
        let error = run.result.unwrap_err();
        if let Some(code) = code {
            assert!(error.contains(code), "{label}: {error}");
        }
        assert!(!env.events().contains("repair_applied"), "{label}");
    }
    set_env(REGISTRY_ENV, None);
}

/// An escaped name in a shape the old parser would have accepted gets a refusal for the
/// tool, not a relaxed reading.
#[test]
fn an_escaped_name_in_an_unexact_wrapper_is_refused_for_the_tool() {
    let _guard = lock();
    for content in [
        format!(
            "<tool_call>{{\"tool\":\"{ESCAPED_CONFIG_TOOL}\",\"args\":{VALID_CONFIG}}}</tool_call>"
        ),
        // A trailing comma the relaxed parser would remove. No apostrophe, which it would
        // otherwise rewrite and fail on before it read the name.
        format!(
            "<tool_call>{{\"name\":\"{ESCAPED_CONFIG_TOOL}\",\"arguments\":{{\"mode\":\"fast\",\"limits\":{{\"retries\":1}},\"targets\":[null]}},}}</tool_call>"
        ),
    ] {
        let env = Env::with_config_tool();
        let run = session(
            &env,
            Wire::Xml,
            vec![text_reply(&content), text_reply("done")],
            None,
        );
        assert_rejected(&env, Wire::Xml, &run, "wrapper_not_exact", &content);
    }
    set_env(REGISTRY_ENV, None);
}

/// What the checks must not refuse: the same escaped name as a clean call, complete think
/// blocks ahead of it, and a builtin's repair in a reply without a schema call.
#[test]
fn a_clean_reply_and_a_builtin_repair_still_pass_in_a_schema_session() {
    let _guard = lock();
    for content in [
        format!(
            "<tool_call>{{\"name\":\"{ESCAPED_CONFIG_TOOL}\",\"arguments\":{VALID_CONFIG}}}</tool_call>"
        ),
        format!(
            "<think>one</think>\n<think>two</think>\n<anvil_tool_call name=\"{CONFIG_TOOL}\">{VALID_CONFIG}</anvil_tool_call>"
        ),
    ] {
        let env = Env::with_config_tool();
        let run = session(
            &env,
            Wire::Xml,
            vec![text_reply(&content), text_reply("done")],
            None,
        );
        assert_eq!(run.executed, vec![parsed(VALID_CONFIG)], "{content}");
    }
    let env = Env::with_config_tool();
    std::fs::write(env.root.join("notes.txt"), "text").unwrap();
    let run = session(
        &env,
        Wire::Xml,
        vec![
            text_reply(
                "<anvil_tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"notes.txt\"}} trailing</anvil_tool_call>",
            ),
            text_reply("done"),
        ],
        None,
    );
    assert_eq!(run.result.as_deref(), Ok("done"));
    assert!(
        env.events().contains("repair_applied"),
        "the builtin's repair went away"
    );
    set_env(REGISTRY_ENV, None);
}

/// The requirement plan runs only on the `generic` profile, which cannot select a schema
/// tool: it has no manifest, and a pack cannot target it. So on a schema profile the plan
/// skips, and a block in a string reaches the executor unchanged. If the plan ever applied
/// here, this fails, or the reply is refused by the check unit-tested in `invocation.rs`.
#[test]
fn a_requirement_plan_block_in_a_string_stays_unchanged_because_the_plan_skips() {
    let _guard = lock();
    const PLAN_ENV: &str = "COMMANDAGENT_REQUIREMENT_PLAN";
    set_env(PLAN_ENV, Some("once"));
    let raw = with_string("a<requirement_plan>{}</requirement_plan>b");
    let env = Env::with_config_tool();
    let run = session(
        &env,
        Wire::Xml,
        vec![Wire::Xml.call(CONFIG_TOOL, &raw), text_reply("done")],
        None,
    );
    set_env(PLAN_ENV, None);
    assert_eq!(run.executed, vec![parsed(&raw)]);
    assert!(
        env.events().contains("requirement_plan"),
        "the plan was not armed"
    );
    set_env(REGISTRY_ENV, None);
}

/// A `number` is compared as an `f64`, so an integer past 2^53 would round into its bound,
/// and a fraction past an `f64`'s reach would change on the way to the executor.
#[test]
fn a_number_an_f64_would_change_never_passes_a_bound_on_both_wires() {
    let _guard = lock();
    let schema = schema_declaration(closed(json!({
        "n": {"type": "number", "maximum": 9_007_199_254_740_992_u64},
    })));
    for wire in WIRES {
        for raw in [
            r#"{"n":9007199254740993}"#,
            r#"{"n":-9007199254740993}"#,
            r#"{"n":1.0000000000000001}"#,
        ] {
            let env = Env::new();
            env.write_tool(schema.clone());
            let run = session(
                &env,
                wire,
                vec![wire.call(CONFIG_TOOL, raw), text_reply("done")],
                None,
            );
            assert_rejected(&env, wire, &run, "lossy_number", raw);
        }
        for raw in [
            r#"{"n":9007199254740992}"#,
            r#"{"n":-9007199254740992}"#,
            r#"{"n":0.30000000000000004}"#,
            r#"{"n":1.5e0}"#,
        ] {
            let env = Env::new();
            env.write_tool(schema.clone());
            let run = session(
                &env,
                wire,
                vec![wire.call(CONFIG_TOOL, raw), text_reply("done")],
                None,
            );
            assert_eq!(run.executed, vec![parsed(raw)], "{wire:?} {raw}");
        }
    }
    // The direct registry has only a value, and the same rule.
    let env = Env::new();
    env.write_tool(schema);
    let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
        .unwrap()
        .unwrap();
    let error = admitted
        .registry()
        .unwrap()
        .execute(
            CONFIG_TOOL,
            &json!({"n": 9_007_199_254_740_993_u64}),
            &registry_context(&env),
        )
        .unwrap_err();
    assert!(format!("{error:#}").contains("lossy_number"), "{error:#}");
    set_env(REGISTRY_ENV, None);
}

/// Doubles `serde_json`'s fast path would land one step off. The token is the shortest
/// decimal of the correctly rounded value, so it must pass, and the executor must get
/// that value. The expectation comes from the standard library's parse.
const CORRECTLY_ROUNDED: [&str; 4] = [
    "14.700000000000001",
    "0.9242829794947799",
    "398.91571404865124",
    "1.602176634e-19",
];

#[test]
fn a_correctly_rounded_double_reaches_the_executor_as_that_value_on_both_wires() {
    let _guard = lock();
    let schema = schema_declaration(closed(json!({"n": {"type": "number"}})));
    for wire in WIRES {
        for token in CORRECTLY_ROUNDED {
            let env = Env::new();
            env.write_tool(schema.clone());
            let raw = format!(r#"{{"n":{token}}}"#);
            let run = session(
                &env,
                wire,
                vec![wire.call(CONFIG_TOOL, &raw), text_reply("done")],
                None,
            );
            assert_eq!(run.executed.len(), 1, "{wire:?} {token}: {:?}", run.result);
            assert_eq!(
                run.executed[0]["n"].as_f64(),
                Some(token.parse::<f64>().unwrap()),
                "{wire:?} {token}"
            );
        }
    }
    set_env(REGISTRY_ENV, None);
}

/// A bound in a descriptor is read the same way, so it holds at its exact value.
#[test]
fn a_correctly_rounded_bound_holds_at_its_exact_value() {
    let _guard = lock();
    let directory_schema = r#"{"protocol":"profile-tool.schema/v1","root":{"additionalProperties":false,"properties":{"n":{"maximum":14.700000000000001,"type":"number"}},"type":"object"}}"#;
    let bound: f64 = "14.700000000000001".parse().unwrap();
    let next_above = f64::from_bits(bound.to_bits() + 1);
    for (raw, runs) in [
        ("14.700000000000001".to_string(), true),
        (format!("{next_above}"), false),
    ] {
        let env = Env::new();
        let directory = env.registry.path().join("profile-tools");
        std::fs::create_dir_all(&directory).unwrap();
        let pin = sha256(directory_schema.as_bytes());
        std::fs::write(
            directory.join(format!("{CONFIG_TOOL_ID}.json")),
            format!(
                r#"{{"protocol":"profile-tool.descriptor/v1","tool_id":"{CONFIG_TOOL_ID}","name":"{CONFIG_TOOL}","version":"1.0.0","description":"d","effect":"read","parameters":[],"schema":{directory_schema},"schema_sha256":"{pin}","runtime":{{"kind":"in_process","id":"{RUNTIME}"}},"limits":{{"max_input_bytes":1024,"max_output_bytes":4096}}}}"#
            ),
        )
        .unwrap();
        let call = format!(r#"{{"n":{raw}}}"#);
        let run = session(
            &env,
            Wire::Native,
            vec![Wire::Native.call(CONFIG_TOOL, &call), text_reply("done")],
            None,
        );
        assert_eq!(
            run.executed.len(),
            usize::from(runs),
            "{raw}: {:?}",
            run.result
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// A closing tag of another kind in a string used to read as closing the outer call, so a
/// call could be cut out of the string, and a second call made from what the model wrote
/// as data. Nothing runs: neither the schema tool nor the builtin.
#[test]
fn a_closing_tag_of_another_kind_in_a_string_runs_nothing() {
    let _guard = lock();
    let read = "{'name':'Read','arguments':{'path':'notes.txt'}}";
    let cases = [
        format!(
            "<tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{{\"s\":\"a</anvil_tool_call><anvil_tool_call>{read}</anvil_tool_call>b\"}}}}</tool_call>"
        ),
        format!(
            "<function={CONFIG_TOOL}>{{\"s\":\"a</tool_call><tool_call>{read}</tool_call>b\"}}</function>"
        ),
        format!(
            "<function={CONFIG_TOOL}>{{\"s\":\"a</function_call><function_call>{read}</function_call>b\"}}</function>"
        ),
    ];
    for content in cases {
        let env = Env::new();
        env.write_tool(schema_declaration(closed(json!({"s": {"type": "string"}}))));
        std::fs::write(env.root.join("notes.txt"), "text").unwrap();
        let run = session(&env, Wire::Xml, vec![text_reply(&content)], None);
        assert_eq!(run.executed, Vec::<Value>::new(), "{content}");
        assert!(!env.events().contains("tool_execute"), "{content}");
        let error = run.result.unwrap_err();
        assert!(error.contains("tag_inside_arguments"), "{content}: {error}");
    }
    set_env(REGISTRY_ENV, None);
}

/// The control: calls side by side are not inside one another, of one kind or of two.
#[test]
fn sibling_calls_still_run_in_a_schema_session() {
    let _guard = lock();
    let schema = schema_declaration(closed(json!({"s": {"type": "string"}})));
    let (first, second) = (r#"{"s":"one"}"#, r#"{"s":"two"}"#);
    for content in [
        format!(
            "<tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{first}}}</tool_call><tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{second}}}</tool_call>"
        ),
        format!(
            "<anvil_tool_call name=\"{CONFIG_TOOL}\">{first}</anvil_tool_call>\n<function={CONFIG_TOOL}>{second}</function>"
        ),
        format!(
            "</tool_call>stray<tool_call>{{\"name\":\"{CONFIG_TOOL}\",\"arguments\":{first}}}</tool_call><function name=\"{CONFIG_TOOL}\">{second}</function>"
        ),
    ] {
        let env = Env::new();
        env.write_tool(schema.clone());
        let run = session(
            &env,
            Wire::Xml,
            vec![text_reply(&content), text_reply("done")],
            None,
        );
        assert_eq!(run.result.as_deref(), Ok("done"), "{content}");
        assert_eq!(
            run.executed,
            vec![parsed(first), parsed(second)],
            "{content}"
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// An `integer` compares exactly past 2^53, because it is never an `f64`.
#[test]
fn an_integer_past_2_to_the_53_still_compares_exactly() {
    let _guard = lock();
    let schema = schema_declaration(closed(json!({
        "n": {"type": "integer", "maximum": 9_007_199_254_740_992_u64},
    })));
    for wire in WIRES {
        let env = Env::new();
        env.write_tool(schema.clone());
        let run = session(
            &env,
            wire,
            vec![
                wire.call(CONFIG_TOOL, r#"{"n":9007199254740993}"#),
                text_reply("done"),
            ],
            None,
        );
        assert_rejected(&env, wire, &run, "above_maximum", "integer above");
    }
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// The REPL's own order, on a new pseudo-terminal: no real terminal is opened.
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod repl_order {
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
        CONFIG_PROFILE, Env, FLAT_PROFILE, FakeHttp, REGISTRY_ENV, base_config, closed, extension,
        flat_registry, install_reader, json, lock, set_env, text_reply,
    };

    const CHILD_ENV: &str = "COMMANDAGENT_TEST_973_REPL_CHILD";
    const ROOT_ENV: &str = "COMMANDAGENT_TEST_973_REPL_ROOT";
    const EVIDENCE_ENV: &str = "COMMANDAGENT_TEST_973_REPL_EVIDENCE";
    const PROFILE_ENV: &str = "COMMANDAGENT_TEST_973_REPL_PROFILE";
    const URL_ENV: &str = "COMMANDAGENT_TEST_973_REPL_URL";
    const PROMPT: &str = "commandagent> ";
    const FREE_TEXT: &str = "Please tidy this up";
    const ROUTED: &str = "Route remains";
    const REFUSED: &str = "TASK FAILED";
    const TIMEOUT: Duration = Duration::from_secs(60);
    const CHILD_DEADLINE: Duration = Duration::from_secs(300);

    /// The REPL itself, with its stdin on the parent's pseudo-terminal. The profile is
    /// not explicit, which is what leaves a route ambiguous and calls the classifier.
    #[test]
    fn repl_child_process() {
        if std::env::var_os(CHILD_ENV).is_none() {
            return;
        }
        extension();
        install_reader();
        let var = |name: &str| std::env::var(name).unwrap();
        let mut config = base_config(
            Path::new(&var(ROOT_ENV)),
            Path::new(&var(EVIDENCE_ENV)),
            &var(PROFILE_ENV),
            &var(URL_ENV),
        );
        config.profile_explicit = false;
        // `/provider ollama` builds a client from this: it must name the fake too.
        config.ollama_host = var(URL_ENV);
        exit_when_abandoned();
        commandagent::tui::repl::run(config).unwrap();
    }

    /// A parent that was killed runs no `Drop`, and a REPL on a terminal nobody writes to
    /// never ends by itself. So the child leaves when its parent goes, or at a deadline.
    fn exit_when_abandoned() {
        // SAFETY: `getppid` has no preconditions.
        let parent = unsafe { libc::getppid() };
        let started = Instant::now();
        std::thread::spawn(move || {
            loop {
                // SAFETY: as above.
                let current = unsafe { libc::getppid() };
                if current != parent || started.elapsed() > CHILD_DEADLINE {
                    std::process::exit(124);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }

    struct Repl {
        child: Child,
        writer: File,
        output: Arc<Mutex<Vec<u8>>>,
        stop: Arc<AtomicBool>,
        timeout: Duration,
    }

    /// On every way out of a test: a pass, a failed assertion, a timeout, and a panic.
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
        fn start(env: &Env, profile: &str, url: &str, home: &Path) -> Self {
            let (master, slave) = open_pty();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "repl_order::repl_child_process", "--nocapture"])
                .env(CHILD_ENV, "1")
                .env(ROOT_ENV, &env.root)
                .env(EVIDENCE_ENV, &env.evidence)
                .env(PROFILE_ENV, profile)
                .env(URL_ENV, url)
                .env("HOME", home)
                .env("TERM", "xterm")
                .stdin(Stdio::from(slave.try_clone().unwrap()))
                .stdout(Stdio::from(slave.try_clone().unwrap()))
                .stderr(Stdio::from(slave.try_clone().unwrap()));
            // SAFETY: only async-signal-safe calls, between fork and exec. This gives the
            // child a new session, so it has no other controlling terminal to reach.
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
                        // A line editor asks where the cursor is, and waits for the answer.
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
                timeout: TIMEOUT,
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
                if started.elapsed() > self.timeout {
                    panic!("no `{needle}` after byte {from}:\n{text}");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }

        /// One line, until `marker` and the next prompt after it. Returns what it printed.
        fn run_line(&mut self, line: &str, marker: &str) -> String {
            std::thread::sleep(Duration::from_millis(100));
            let from = self.text().len();
            self.writer.write_all(line.as_bytes()).unwrap();
            self.writer.write_all(b"\r").unwrap();
            let end = self.wait_for(marker, from);
            let end = self.wait_for(PROMPT, end);
            self.text()[from..end].to_string()
        }

        fn finish(mut self) {
            std::thread::sleep(Duration::from_millis(100));
            self.writer.write_all(b"/exit\r").unwrap();
            let started = Instant::now();
            let status = loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    break status;
                }
                if started.elapsed() > self.timeout {
                    panic!("the REPL did not exit:\n{}", self.text());
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            assert!(status.success(), "{status}:\n{}", self.text());
        }
    }

    /// Running, and not only a zombie nobody reaped, as under a container's first process.
    fn is_running(pid: i32) -> bool {
        let state = if Path::new("/proc/self/stat").exists() {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|stat| stat.rsplit_once(')').map(|(_, rest)| rest.to_string()))
        } else {
            Command::new("ps")
                .args(["-o", "stat=", "-p", &pid.to_string()])
                .output()
                .ok()
                .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        };
        state.is_some_and(|state| {
            let state = state.trim_start();
            !state.is_empty() && !state.starts_with('Z')
        })
    }

    #[test]
    fn a_failing_or_timed_out_repl_test_leaves_no_child() {
        let _guard = lock();
        let env = Env::with_config_tool();
        let home = tempfile::tempdir().unwrap();
        let http = classifier_fake();
        for timed_out in [false, true] {
            let pid = Mutex::new(0);
            let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut repl = Repl::start(&env, CONFIG_PROFILE, &http.url, home.path());
                *pid.lock().unwrap() = repl.child.id() as i32;
                assert!(is_running(repl.child.id() as i32));
                if timed_out {
                    repl.timeout = Duration::from_millis(100);
                    repl.wait_for("never printed", 0);
                }
                panic!("a failed assertion in a test");
            }));
            assert!(failed.is_err());
            let pid = *pid.lock().unwrap();
            assert!(pid > 1);
            assert!(!is_running(pid), "the child {pid} outlived its test");
        }
        http.finish();
        set_env(REGISTRY_ENV, None);
    }

    const PARENT_ENV: &str = "COMMANDAGENT_TEST_973_REPL_PARENT";

    /// A test process that dies as a killed one does: it runs no `Drop`.
    #[test]
    fn repl_parent_process() {
        let Some(pid_file) = std::env::var_os(PARENT_ENV) else {
            return;
        };
        let _guard = lock();
        let env = Env::with_config_tool();
        let home = tempfile::tempdir().unwrap();
        let http = classifier_fake();
        let repl = Repl::start(&env, CONFIG_PROFILE, &http.url, home.path());
        std::fs::write(pid_file, repl.child.id().to_string()).unwrap();
        // SAFETY: `_exit` ends this process at once, which is what is being tested.
        unsafe { libc::_exit(0) };
    }

    #[test]
    fn a_killed_test_process_leaves_no_child() {
        let private = tempfile::tempdir().unwrap();
        let pid_file = private.path().join("child.pid");
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "repl_order::repl_parent_process", "--nocapture"])
            .env(PARENT_ENV, &pid_file)
            .env("TMPDIR", private.path())
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "{status}");
        let pid = std::fs::read_to_string(&pid_file)
            .unwrap()
            .parse::<i32>()
            .unwrap();
        assert!(pid > 1);
        let started = Instant::now();
        while is_running(pid) {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "the child {pid} outlived its killed parent"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Any reply a classifier cannot use: the route stays typed unknown, and a call to
    /// the fake is what is measured.
    fn classifier_fake() -> FakeHttp {
        let reply = text_reply("not a route");
        FakeHttp::with_responder(Vec::new(), Arc::new(move |_, _| Some(reply.clone())))
    }

    /// Named by the classifier's own prompt: the REPL makes other requests at its start.
    fn classifier_calls(http: &FakeHttp) -> usize {
        let requests = http.requests.lock().unwrap();
        requests
            .iter()
            .filter(|body| body.contains("route proposal classifier"))
            .count()
    }

    #[test]
    fn a_repl_refuses_free_text_on_an_unsupported_provider_before_its_classifier() {
        let _guard = lock();
        let env = Env::with_config_tool();
        let home = tempfile::tempdir().unwrap();
        let http = classifier_fake();
        let mut repl = Repl::start(&env, CONFIG_PROFILE, &http.url, home.path());

        // The witness: on a supported client the same text reaches the classifier.
        repl.run_line(FREE_TEXT, ROUTED);
        assert_eq!(classifier_calls(&http), 1);

        repl.run_line("/provider ollama", "Executor provider set");
        let before = http.requests.lock().unwrap().len();
        let shown = repl.run_line(FREE_TEXT, REFUSED);
        assert!(shown.contains("schema_unsupported"), "{shown}");
        assert_eq!(
            http.requests.lock().unwrap().len(),
            before,
            "a provider call ran before the refusal"
        );

        // The REPL goes on, and takes the text again once the client can carry the tool.
        repl.run_line("/provider lm-studio", "Executor provider set");
        repl.run_line(FREE_TEXT, ROUTED);
        assert_eq!(classifier_calls(&http), 2);
        repl.finish();
        http.finish();
        set_env(REGISTRY_ENV, None);
    }

    #[test]
    fn a_repl_refuses_free_text_after_an_input_edit_before_its_classifier() {
        let _guard = lock();
        let env = Env::new();
        env.write_tool(json!({
            "protocol": "profile-tool.schema/v1",
            "root": closed(json!({})),
            "inputs": ["ids.txt"],
        }));
        let input = env.root.join("ids.txt");
        std::fs::write(&input, "alpha\n").unwrap();
        let home = tempfile::tempdir().unwrap();
        let http = classifier_fake();
        let mut repl = Repl::start(&env, CONFIG_PROFILE, &http.url, home.path());

        repl.run_line(FREE_TEXT, ROUTED);
        assert_eq!(classifier_calls(&http), 1);

        std::fs::write(&input, "edited\n").unwrap();
        let before = http.requests.lock().unwrap().len();
        let shown = repl.run_line(FREE_TEXT, REFUSED);
        assert!(shown.contains("input_revision_mismatch"), "{shown}");
        assert_eq!(
            http.requests.lock().unwrap().len(),
            before,
            "a provider call ran before the refusal"
        );

        // Restoring the bytes is the documented recovery.
        std::fs::write(&input, "alpha\n").unwrap();
        repl.run_line(FREE_TEXT, ROUTED);
        assert_eq!(classifier_calls(&http), 2);
        repl.finish();
        http.finish();
        set_env(REGISTRY_ENV, None);
    }

    /// With no declaration, and with a flat tool, the classifier runs as it always did,
    /// also on a provider a schema tool could not use.
    #[test]
    fn a_repl_without_a_schema_tool_still_calls_its_classifier() {
        let _guard = lock();
        let flat = flat_registry();
        for (profile, registry) in [
            ("generic", "/nonexistent/profile-tool-registry"),
            (FLAT_PROFILE, flat.path().to_str().unwrap()),
        ] {
            let env = Env::new();
            set_env(REGISTRY_ENV, Some(registry));
            let home = tempfile::tempdir().unwrap();
            let http = classifier_fake();
            let mut repl = Repl::start(&env, profile, &http.url, home.path());
            let first = repl.run_line(FREE_TEXT, ROUTED);
            assert!(!first.contains(REFUSED), "{profile}: {first}");
            assert_eq!(classifier_calls(&http), 1, "{profile}");

            repl.run_line("/provider ollama", "Executor provider set");
            let second = repl.run_line(FREE_TEXT, ROUTED);
            assert!(!second.contains(REFUSED), "{profile}: {second}");
            assert_eq!(classifier_calls(&http), 2, "{profile}");
            repl.finish();
            http.finish();
        }
        set_env(REGISTRY_ENV, None);
    }
}

/// A key repeated around the arguments is gone from a whole-body `Value` too. The body
/// here repeats `arguments` itself, which no `reply(name, Value)` can express.
#[test]
fn a_repeated_key_in_the_native_envelope_is_refused() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let body = format!(
        r#"{{"choices":[{{"message":{{"role":"assistant","content":"","tool_calls":[{{"id":"c","type":"function","function":{{"name":"{CONFIG_TOOL}","arguments":"{{}}","arguments":{}}}}}]}}}}]}}"#,
        serde_json::to_string(VALID_CONFIG).unwrap()
    );
    let run = session(&env, Wire::Native, vec![body, text_reply("done")], None);
    assert_rejected(&env, Wire::Native, &run, "duplicate_key", "envelope");
    set_env(REGISTRY_ENV, None);
}

#[test]
fn native_arguments_that_are_not_a_string_are_refused_without_a_parse_error() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let body = json!({"choices": [{"message": {"role": "assistant", "content": "", "tool_calls": [
        {"id": "c", "type": "function", "function": {"name": CONFIG_TOOL, "arguments": parsed(VALID_CONFIG)}},
    ]}}]})
    .to_string();
    let run = session(&env, Wire::Native, vec![body, text_reply("done")], None);
    assert_rejected(
        &env,
        Wire::Native,
        &run,
        "malformed_json",
        "object arguments",
    );
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// The limits, at their edges.
// ---------------------------------------------------------------------------

fn nested_schema(depth: usize) -> Value {
    let mut schema = closed(json!({}));
    for _ in 1..depth {
        schema = closed(json!({"a": schema}));
    }
    schema
}

fn nested_raw(depth: usize) -> String {
    format!(
        "{}{{}}{}",
        r#"{"a":"#.repeat(depth - 1),
        "}".repeat(depth - 1)
    )
}

#[test]
fn the_depth_limit_holds_at_its_edge_on_both_wires() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::new();
        env.write_tool(schema_declaration(nested_schema(32)));
        let at_limit = nested_raw(32);
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, &at_limit), text_reply("done")],
            None,
        );
        assert_eq!(run.executed, vec![parsed(&at_limit)], "{wire:?}");

        let env = Env::new();
        env.write_tool(schema_declaration(nested_schema(32)));
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, &nested_raw(33)), text_reply("done")],
            None,
        );
        assert_rejected(&env, wire, &run, "arguments_too_deep", "depth 33");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn the_size_limit_holds_at_its_edge_on_both_wires() {
    let _guard = lock();
    let schema = schema_declaration(closed(json!({"s": {"type": "string"}})));
    let raw_of = |bytes: usize| format!(r#"{{"s":"{}"}}"#, "a".repeat(bytes - r#"{"s":""}"#.len()));
    assert_eq!(raw_of(64 * 1024).len(), 64 * 1024);
    for wire in WIRES {
        let env = Env::new();
        env.write_tool(schema.clone());
        let at_limit = raw_of(64 * 1024);
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, &at_limit), text_reply("done")],
            None,
        );
        assert_eq!(run.executed, vec![parsed(&at_limit)], "{wire:?}");

        let env = Env::new();
        env.write_tool(schema.clone());
        let run = session(
            &env,
            wire,
            vec![
                wire.call(CONFIG_TOOL, &raw_of(64 * 1024 + 1)),
                text_reply("done"),
            ],
            None,
        );
        assert_rejected(&env, wire, &run, "arguments_too_large", "64KiB + 1");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn the_element_limit_holds_at_its_edge_on_both_wires() {
    let _guard = lock();
    let schema = schema_declaration(closed(json!({
        "a": {"type": "array", "items": {"type": "integer"}},
    })));
    // The root, the array, and its items.
    let raw_of = |elements: usize| {
        let items = vec!["1"; elements - 2].join(",");
        format!(r#"{{"a":[{items}]}}"#)
    };
    for wire in WIRES {
        let env = Env::new();
        env.write_tool(schema.clone());
        let at_limit = raw_of(512);
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, &at_limit), text_reply("done")],
            None,
        );
        assert_eq!(run.executed, vec![parsed(&at_limit)], "{wire:?}");

        let env = Env::new();
        env.write_tool(schema.clone());
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, &raw_of(513)), text_reply("done")],
            None,
        );
        assert_rejected(&env, wire, &run, "too_many_elements", "513 elements");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn an_operator_can_only_narrow_a_limit() {
    let _guard = lock();
    let mut narrowed = schema_declaration(closed(json!({"s": {"type": "string"}})));
    narrowed["limits"] = json!({"max_argument_bytes": 16});
    let env = Env::new();
    env.write_tool(narrowed);
    let run = session(
        &env,
        Wire::Native,
        vec![
            Wire::Native.call(CONFIG_TOOL, r#"{"s":"0123456789abc"}"#),
            text_reply("done"),
        ],
        None,
    );
    assert_rejected(&env, Wire::Native, &run, "arguments_too_large", "narrowed");

    for (key, value) in [
        ("max_argument_bytes", 64 * 1024 + 1),
        ("max_depth", 33),
        ("max_elements", 513),
        ("max_schema_bytes", 256 * 1024 + 1),
        ("max_depth", 0),
    ] {
        let mut wider = schema_declaration(closed(json!({})));
        wider["limits"] = json!({ key: value });
        let env = Env::new();
        env.write_tool(wider);
        let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("profile_tool_refused: schema_limit_exceeded:"),
            "{key}: {error}"
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// The input limit a tool was admitted with narrows the same way, and the G1 check of
/// it stays in place right before the execution.
#[test]
fn the_declared_input_limit_narrows_the_arguments_too() {
    let _guard = lock();
    let env = Env::new();
    let schema = schema_declaration(closed(json!({"s": {"type": "string"}})));
    let descriptor = json!({
        "protocol": "profile-tool.descriptor/v1",
        "tool_id": CONFIG_TOOL_ID,
        "name": CONFIG_TOOL,
        "version": "1.0.0",
        "description": "Submit one object for a synthetic fixture.",
        "effect": "read",
        "parameters": [],
        "schema": schema,
        "schema_sha256": sha256(serde_json::to_string(&schema).unwrap().as_bytes()),
        "runtime": {"kind": "in_process", "id": RUNTIME},
        "limits": {"max_input_bytes": 16, "max_output_bytes": 4096},
    });
    let directory = env.registry.path().join("profile-tools");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join(format!("{CONFIG_TOOL_ID}.json")),
        descriptor.to_string(),
    )
    .unwrap();
    let run = session(
        &env,
        Wire::Native,
        vec![
            Wire::Native.call(CONFIG_TOOL, r#"{"s":"0123456789abc"}"#),
            text_reply("done"),
        ],
        None,
    );
    assert_rejected(
        &env,
        Wire::Native,
        &run,
        "arguments_too_large",
        "input limit",
    );
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// Acceptance 2: the binding is the host's, adopted once.
// ---------------------------------------------------------------------------

/// The expected digest, from the documented rule, not from the implementation.
fn expected_revision(inputs: &[(&str, &[u8])]) -> String {
    let mut digest = Sha256::new();
    for (path, bytes) in inputs {
        digest.update(path.as_bytes());
        digest.update([0]);
        digest.update(bytes.len().to_string().as_bytes());
        digest.update([0]);
        digest.update(bytes);
    }
    format!("{:x}", digest.finalize())
}

fn revision_root() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["input_revision", "id"],
        "properties": {
            "input_revision": {"type": "string"},
            "schema_sha256": {"type": "string"},
            "accepted": {"type": "boolean"},
            "id": {"type": "string"},
        },
    })
}

fn compiled_tool(env: &Env) -> String {
    register_schema_compiler(COMPILER, Arc::new(LinesCompiler));
    let schema = json!({
        "protocol": "profile-tool.schema/v1",
        "root": revision_root(),
        "inputs": ["inputs/ids.txt"],
        "compiler": COMPILER,
        "revision_argument": "input_revision",
    });
    env.write_tool(schema);
    std::fs::create_dir_all(env.root.join("inputs")).unwrap();
    std::fs::write(env.root.join("inputs/ids.txt"), "alpha\nbeta\n").unwrap();
    expected_revision(&[("inputs/ids.txt", b"alpha\nbeta\n")])
}

fn bound_call(revision: &str, id: &str) -> String {
    json!({"input_revision": revision, "id": id}).to_string()
}

#[test]
fn a_compiler_derives_the_schema_from_the_adopted_bytes_and_the_binding_is_visible() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::new();
        let revision = compiled_tool(&env);
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        let binding = &admitted.bindings()[0];
        assert_eq!(binding.input_revision(), Some(revision.as_str()));
        assert_eq!(binding.version(), "1.0.0");
        let schema = admitted
            .registry()
            .unwrap()
            .specs()
            .iter()
            .find(|spec| spec.function.name == CONFIG_TOOL)
            .unwrap()
            .function
            .parameters
            .clone();
        assert_eq!(schema["properties"]["id"]["enum"], json!(["alpha", "beta"]));
        assert_eq!(
            schema["properties"]["input_revision"]["enum"],
            json!([revision])
        );
        assert_eq!(
            binding.adopted_schema_sha256(),
            Some(sha256(schema.to_string().as_bytes()).as_str())
        );

        let good = bound_call(&revision, "beta");
        let run = session(
            &env,
            wire,
            vec![wire.call(CONFIG_TOOL, &good), text_reply("done")],
            None,
        );
        assert_eq!(run.executed, vec![parsed(&good)], "{wire:?}");

        let env = Env::new();
        let revision = compiled_tool(&env);
        let run = session(
            &env,
            wire,
            vec![
                wire.call(CONFIG_TOOL, &bound_call(&revision, "gamma")),
                text_reply("done"),
            ],
            None,
        );
        assert_rejected(&env, wire, &run, "enum_mismatch", "an ID outside the input");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_call_echoing_another_revision_is_refused_and_self_declared_values_do_nothing() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::new();
        let revision = compiled_tool(&env);
        let old = expected_revision(&[("inputs/ids.txt", b"alpha\n")]);
        let run = session(
            &env,
            wire,
            vec![
                wire.call(CONFIG_TOOL, &bound_call(&old, "alpha")),
                text_reply("done"),
            ],
            None,
        );
        assert_rejected(&env, wire, &run, "enum_mismatch", "an old revision");

        // Declared properties are data. Claiming acceptance or another hash neither
        // passes a check nor becomes the expectation for a later call.
        let env = Env::new();
        let revision_two = compiled_tool(&env);
        assert_eq!(revision, revision_two);
        let forged = json!({
            "input_revision": revision_two,
            "id": "alpha",
            "accepted": true,
            "schema_sha256": "0".repeat(64),
        })
        .to_string();
        let (first, second) = (wire.call(CONFIG_TOOL, &forged), text_reply("done"));
        let run = session(&env, wire, vec![first, second], None);
        assert_eq!(run.executed, vec![parsed(&forged)], "{wire:?}");
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            admitted.bindings()[0].input_revision(),
            Some(revision.as_str())
        );
        assert_ne!(
            admitted.bindings()[0].adopted_schema_sha256(),
            Some("0".repeat(64).as_str())
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// Sends a model's hash for bytes that changed: the model cannot carry the run across.
#[test]
fn a_model_that_sends_the_new_digest_does_not_move_the_adopted_revision() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::new();
        compiled_tool(&env);
        let path = env.root.join("inputs/ids.txt");
        let new = expected_revision(&[("inputs/ids.txt", b"alpha\nbeta\ngamma\n")]);
        let call = bound_call(&new, "gamma");
        let reply = wire.call(CONFIG_TOOL, &call);
        let responder: Responder = Arc::new(move |_, _| {
            std::fs::write(&path, "alpha\nbeta\ngamma\n").unwrap();
            None
        });
        let run = session(&env, wire, vec![reply], Some(responder));
        assert_eq!(run.executed, Vec::<Value>::new(), "{wire:?}");
        let error = run.result.unwrap_err();
        assert!(
            error.contains("profile_tool_refused: input_revision_mismatch"),
            "{error}"
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// An input changed between the provider's request and its reply: the receipt sees it.
/// A later session and a repair in this process meet the same adoption; nothing adopts
/// again. Restoring the bytes restores the run, which is the comparison at work.
#[test]
fn an_input_changed_during_a_run_is_refused_at_receipt_and_never_adopted_again() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::new();
        let revision = compiled_tool(&env);
        let path = env.root.join("inputs/ids.txt");
        let call = bound_call(&revision, "alpha");
        let reply = wire.call(CONFIG_TOOL, &call);
        let during = path.clone();
        let responder: Responder = Arc::new(move |_, _| {
            std::fs::write(&during, "changed\n").unwrap();
            None
        });
        let run = session(&env, wire, vec![reply.clone()], Some(responder));
        assert_eq!(run.executed, Vec::<Value>::new(), "{wire:?}");
        let error = run.result.unwrap_err();
        assert!(
            error.contains("profile_tool_refused: input_revision_mismatch: stage=received_call"),
            "{error}"
        );
        assert!(!env.events().contains("fallback_decision"), "{error}");

        let error = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("input_revision_mismatch: stage=admission"),
            "a later admission adopted again: {error}"
        );
        let later = session(&env, wire, vec![reply.clone()], None);
        assert_eq!(
            later.requests.len(),
            0,
            "a session started on changed bytes"
        );
        assert!(
            later
                .result
                .unwrap_err()
                .contains("input_revision_mismatch")
        );

        std::fs::write(&path, "alpha\nbeta\n").unwrap();
        let restored = session(&env, wire, vec![reply, text_reply("done")], None);
        assert_eq!(restored.executed, vec![parsed(&call)], "{wire:?}");
    }
    set_env(REGISTRY_ENV, None);
}

fn registry_context(env: &Env) -> ToolContext {
    ToolContext {
        root: env.root.clone(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: false,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: None,
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

/// The public registry has no receipts and no provider. The executor boundary still
/// validates the value and compares the binding.
#[test]
fn the_public_registry_validates_and_compares_right_before_the_execution() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let reader = install_reader();
    let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
        .unwrap()
        .unwrap();
    let registry = admitted.registry().unwrap();
    let context = registry_context(&env);
    let valid = parsed(VALID_CONFIG);
    for invalid in [
        json!({"mode": "slow", "limits": {"retries": 1}, "targets": [null]}),
        json!({"mode": "fast", "limits": {"retries": 1, "x": 1}, "targets": [null]}),
        json!({"mode": "fast", "limits": {"retries": 4}, "targets": [null]}),
        json!({"mode": "fast", "limits": {}, "targets": [null]}),
        json!({"mode": "fast", "limits": {"retries": 1}, "targets": [7]}),
        json!(["not an object"]),
    ] {
        let error = registry
            .execute(CONFIG_TOOL, &invalid, &context)
            .unwrap_err();
        let error = format!("{error:#}");
        assert!(
            error.contains("profile_tool_refused: schema_violation"),
            "{error}"
        );
        assert!(error.contains("\"stage\":\"before_execution\""), "{error}");
    }
    assert_eq!(reader.calls.lock().unwrap().len(), 0);
    registry.execute(CONFIG_TOOL, &valid, &context).unwrap();
    assert_eq!(*reader.calls.lock().unwrap(), vec![valid]);
    let receipts = admitted.receipts();
    assert_eq!(receipts.len(), 7);
    assert_eq!(
        receipts.iter().filter(|receipt| receipt.executed).count(),
        1
    );
    assert_eq!(
        receipts.last().unwrap().usage,
        None,
        "unknown usage stayed unknown"
    );
    set_env(REGISTRY_ENV, None);
}

#[test]
fn the_public_registry_refuses_a_changed_input_tool_pin_or_compiler_at_the_execution() {
    let _guard = lock();
    let change_input = |env: &Env| std::fs::write(env.root.join("inputs/ids.txt"), "x\n").unwrap();
    let change_pin = |env: &Env| {
        write_full_descriptor(
            env.registry.path(),
            CONFIG_TOOL_ID,
            CONFIG_TOOL,
            &json!({
                "protocol": "profile-tool.schema/v1",
                "root": revision_root(),
                "inputs": ["inputs/ids.txt"],
                "compiler": COMPILER,
                "revision_argument": "input_revision",
            }),
            "2.0.0",
            &[],
        );
    };
    let change_compiler = |_: &Env| register_schema_compiler(COMPILER, Arc::new(ReplacedCompiler));
    type EnvChange = fn(&Env);
    let cases: [(&str, EnvChange, &str); 3] = [
        ("input", change_input, "input_revision_mismatch"),
        ("tool pin", change_pin, "invalid_pin"),
        ("compiler", change_compiler, "schema_mismatch"),
    ];
    for (label, change, kind) in cases {
        let env = Env::new();
        let revision = compiled_tool(&env);
        let reader = install_reader();
        let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
            .unwrap()
            .unwrap();
        let registry = admitted.registry().unwrap();
        change(&env);
        let error = registry
            .execute(
                CONFIG_TOOL,
                &parsed(&bound_call(&revision, "alpha")),
                &registry_context(&env),
            )
            .unwrap_err();
        let error = format!("{error:#}");
        // The tool pin keeps the wording of the G1 check that runs first.
        let expected = if kind == "invalid_pin" {
            format!("profile_tool_refused: {kind}: admitted descriptor changed")
        } else {
            format!("profile_tool_refused: {kind}: stage=before_execution")
        };
        assert!(error.contains(&expected), "{label}: {error}");
        assert_eq!(reader.calls.lock().unwrap().len(), 0, "{label}");
    }
    register_schema_compiler(COMPILER, Arc::new(LinesCompiler));
    set_env(REGISTRY_ENV, None);
}

/// The swaps a run can meet between its request and the reply, on both wires.
#[test]
fn a_tool_pin_or_compiler_swapped_during_a_run_is_refused_at_receipt() {
    let _guard = lock();
    for wire in WIRES {
        for (label, kind) in [("pin", "invalid_pin"), ("compiler", "schema_mismatch")] {
            let env = Env::new();
            let revision = compiled_tool(&env);
            let registry = env.registry.path().to_path_buf();
            let responder: Responder = Arc::new(move |_, _| {
                if label == "pin" {
                    write_full_descriptor(
                        &registry,
                        CONFIG_TOOL_ID,
                        CONFIG_TOOL,
                        &schema_declaration(revision_root()),
                        "2.0.0",
                        &[],
                    );
                } else {
                    register_schema_compiler(COMPILER, Arc::new(ReplacedCompiler));
                }
                None
            });
            let reply = wire.call(CONFIG_TOOL, &bound_call(&revision, "alpha"));
            let run = session(&env, wire, vec![reply], Some(responder));
            assert_eq!(run.executed, Vec::<Value>::new(), "{label}");
            let error = run.result.unwrap_err();
            assert!(
                error.contains(&format!(
                    "profile_tool_refused: {kind}: stage=received_call"
                )),
                "{label}: {error}"
            );
            register_schema_compiler(COMPILER, Arc::new(LinesCompiler));
        }
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_replaced_descriptor_is_not_adopted_by_a_later_session() {
    let _guard = lock();
    let env = Env::with_config_tool();
    admit_selected(&env.root, CONFIG_PROFILE, None)
        .unwrap()
        .unwrap();
    write_full_descriptor(
        env.registry.path(),
        CONFIG_TOOL_ID,
        CONFIG_TOOL,
        &schema_declaration(config_root()),
        "2.0.0",
        &[],
    );
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: schema_mismatch:"),
        "{error}"
    );
    set_env(REGISTRY_ENV, None);
}

#[test]
fn an_input_that_is_missing_or_a_symlink_is_refused_before_the_start() {
    let _guard = lock();
    let schema = json!({
        "protocol": "profile-tool.schema/v1",
        "root": closed(json!({})),
        "inputs": ["inputs/ids.txt"],
    });
    let env = Env::new();
    env.write_tool(schema.clone());
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: input_revision_unavailable:"),
        "{error}"
    );

    #[cfg(unix)]
    {
        let env = Env::new();
        env.write_tool(schema);
        std::fs::create_dir_all(env.root.join("inputs")).unwrap();
        std::fs::write(env.root.join("real.txt"), "a").unwrap();
        std::os::unix::fs::symlink(env.root.join("real.txt"), env.root.join("inputs/ids.txt"))
            .unwrap();
        let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("profile_tool_refused: input_revision_unavailable:"),
            "{error}"
        );
    }
    for bad in ["/etc/hosts", "../outside", ""] {
        let env = Env::new();
        env.write_tool(json!({
            "protocol": "profile-tool.schema/v1",
            "root": closed(json!({})),
            "inputs": [bad],
        }));
        let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("profile_tool_refused: input_revision_unavailable:"),
            "{bad}: {error}"
        );
    }
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// Acceptance 3: what cannot be honored is refused before the start, at every entry.
// ---------------------------------------------------------------------------

const PLAN_GOAL: &str = "Submit the object";

fn inspect_plan() -> StepPlan {
    StepPlan {
        goal: PLAN_GOAL.to_string(),
        steps: vec![PlanStep {
            id: "step-inspect".to_string(),
            kind: "inspect".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Submit the object".to_string(),
            expected_paths: Vec::new(),
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

/// What a planner answers: a step plan is requested as JSON.
fn step_plan_reply() -> String {
    json!({
        "goal": PLAN_GOAL,
        "steps": [{
            "id": "step-inspect",
            "kind": "inspect",
            "expected_result": "pass",
            "instruction": "Submit the object",
            "expected_paths": [],
            "verify": [],
        }],
    })
    .to_string()
}

fn write_plans(env: &Env, profile: &str) {
    std::fs::write(
        env.root.join("inspect.yaml"),
        render_step_plan(&inspect_plan()),
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

/// Every entry that can run a session. Each returns what the entry returned.
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
    "/run-plan inspect.yaml",
    "/plan-run Submit the object",
    "/ultra-plan-run Submit the object",
    "/run-ultra-plan ultra.yaml",
    "/plan-run --profile schema-config Submit the object",
    "/ultra-plan-run --profile schema-config Submit the object",
    "profile-command:/plan-run Submit the object",
    "gate-one",
    "directive",
];

/// Entries that only generate, or that collect failures of their own.
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
    let plan = inspect_plan();
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
        "run_plan_file" => run_plan_file(execution, Path::new("inspect.yaml"), config),
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
        // A refused `model_probe::run_configured` writes nothing below HOME, and it is
        // only reached by the rows that are refused.
        "model_probe::run_configured" => {
            commandagent::model_probe::run_configured(config, planner, execution)
                .map(|_| String::new())
        }
        "gate-one" => gate_one(planner, execution, config, env),
        "directive" => directive(planner, execution, config, env),
        line => match line.strip_prefix("profile-command:") {
            // What `/profile` leaves in the REPL's config, from another starting profile.
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
            prompt_version: "issue973-test",
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

/// A reason the host refuses before the start, with the kind it must report.
struct StartRefusal {
    label: &'static str,
    kind: &'static str,
    prepare: fn(&Env),
}

fn write_with_property(env: &Env, property: Value) {
    env.write_tool(schema_declaration(closed(json!({"a": property}))));
}

const START_REFUSALS: &[StartRefusal] = &[
    StartRefusal {
        label: "unsupported keyword",
        kind: "schema_unsupported",
        prepare: |env| write_with_property(env, json!({"$ref": "https://example.invalid/s"})),
    },
    StartRefusal {
        label: "computation too large",
        kind: "schema_limit_exceeded",
        prepare: |env| {
            let branches = (0..17).map(|_| json!({"type": "null"})).collect::<Vec<_>>();
            write_with_property(env, json!({"anyOf": branches}));
        },
    },
    StartRefusal {
        label: "a sanitizer would change it",
        kind: "schema_unsupported",
        prepare: |env| {
            env.write_tool(schema_declaration(closed(
                json!({"$id": {"type": "string"}}),
            )));
        },
    },
    StartRefusal {
        label: "a scrub would change it",
        kind: "schema_scrubbed",
        prepare: |env| {
            env.write_tool(schema_declaration(closed(
                json!({DUMMY_SECRET: {"type": "string"}}),
            )));
        },
    },
    StartRefusal {
        label: "input revision cannot be adopted",
        kind: "input_revision_unavailable",
        prepare: |env| {
            env.write_tool(json!({
                "protocol": "profile-tool.schema/v1",
                "root": closed(json!({})),
                "inputs": ["absent.txt"],
            }));
        },
    },
    StartRefusal {
        label: "an injected client declares no wire",
        kind: "schema_unsupported",
        prepare: |env| env.write_tool(schema_declaration(config_root())),
    },
];

fn install_dummy_secret(root: &Path, config: &Config) {
    let mut catalog = SecretCatalog::new();
    catalog
        .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
        .unwrap();
    install_scope(catalog, Some(root), config.eval_events_path.as_deref());
}

#[test]
fn every_entry_refuses_before_any_provider_classifier_or_event() {
    let _guard = lock();
    let mut failures = Vec::new();
    for refusal in START_REFUSALS {
        for entry in SESSION_ENTRIES.iter().chain(OTHER_ENTRIES) {
            let env = Env::new();
            write_plans(&env, CONFIG_PROFILE);
            (refusal.prepare)(&env);
            let endpoint = CountingEndpoint::start();
            let mut config = env.config(&endpoint.url, Wire::Native);
            install_dummy_secret(&env.root, &config);
            let injected = refusal.label.starts_with("an injected");
            // A planner-only entry reads the configured executor rather than a client,
            // so the row about a client that declares nothing has that configuration.
            if injected {
                config.provider = Provider::Ollama;
            }
            let mut fake_planner = InjectedClient::default();
            let mut fake_execution = InjectedClient::default();
            let mut real_planner = client_from_config(&config, true).unwrap();
            let mut real_execution = client_from_config(&config, false).unwrap();
            let (planner, execution): (&mut dyn ChatClient, &mut dyn ChatClient) = if injected {
                (&mut fake_planner, &mut fake_execution)
            } else {
                (real_planner.as_mut(), real_execution.as_mut())
            };
            let classifier = CountingClassifier {
                calls: AtomicUsize::new(0),
            };
            let before = artifacts(&env.root);
            let outcome = run_entry(entry, planner, execution, &config, &env, &classifier);
            let expected = format!("profile_tool_refused: {}", refusal.kind);
            let refused = outcome
                .as_ref()
                .err()
                .is_some_and(|error| error.contains(&expected));
            let calls = fake_planner.calls() + fake_execution.calls() + endpoint.finish();
            let classified = classifier.calls.load(Ordering::SeqCst);
            let changed = artifacts(&env.root) != before;
            let probe_output = env.evidence.join("probe-output").exists();
            if !refused || calls != 0 || classified != 0 || changed || probe_output {
                failures.push(format!(
                    "{entry} / {}: refused={refused} calls={calls} classifier={classified} changed={changed} probe={probe_output} {outcome:?}",
                    refusal.label
                ));
            }
        }
    }
    set_env(REGISTRY_ENV, None);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A later session in one run: an injected client met only at the session is refused
/// too, because the adoption of a first entry says nothing about this client.
#[test]
fn the_session_judges_the_client_it_was_given() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let config = env.config("http://127.0.0.1:9", Wire::Native);
    admit_selected(&env.root, CONFIG_PROFILE, None)
        .unwrap()
        .unwrap();
    let mut injected = InjectedClient::default();
    let error = run_session(&mut injected, &mut SessionSnapshot::new(), "x", &config).unwrap_err();
    assert!(
        format!("{error:#}").contains("profile_tool_refused: schema_unsupported"),
        "{error:#}"
    );
    assert_eq!(injected.calls(), 0);
    set_env(REGISTRY_ENV, None);
}

fn parse_config(env: &Env, arguments: &[&str]) -> Config {
    let mut all = vec![
        "commandagent".to_string(),
        "--cwd".to_string(),
        env.root.display().to_string(),
        "--state-dir".to_string(),
        env.evidence.join("state").display().to_string(),
        "--model".to_string(),
        "m".to_string(),
        "--planner-model".to_string(),
        "m".to_string(),
    ];
    all.extend(arguments.iter().map(|argument| argument.to_string()));
    let parsed = commandagent::provider_cli::parse_from(all).unwrap();
    let mut config =
        Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap();
    config.eval_events_path = Some(env.evidence.join("events.jsonl"));
    config.profile = CONFIG_PROFILE.to_string();
    config.profile_explicit = true;
    config
}

/// What each real client declares, and what the configuration says it would build,
/// including the role override. Refusals happen before any connection.
#[test]
fn the_wire_is_judged_from_the_real_client_and_the_effective_role() {
    let _guard = lock();
    let endpoint = FakeHttp::start(Vec::new());
    let url = endpoint.url.clone();
    set_env("GEMINI_API_KEY", Some("dummy-973-not-a-real-credential"));
    set_env("OPENAI_API_KEY", Some("dummy-973-not-a-real-credential"));
    type ConfigChange = fn(&mut Config);
    let cases: [(&str, ConfigChange, bool); 6] = [
        ("lm-studio chat", |_| {}, true),
        (
            "openai chat",
            |config: &mut Config| config.provider = Provider::Openai,
            true,
        ),
        (
            "lm-studio responses",
            |config: &mut Config| config.openai_api = OpenAiApi::Responses,
            false,
        ),
        (
            "openai responses",
            |config: &mut Config| {
                config.provider = Provider::Openai;
                config.openai_api = OpenAiApi::Responses;
            },
            false,
        ),
        (
            "gemini",
            |config: &mut Config| config.provider = Provider::Gemini,
            false,
        ),
        (
            "ollama",
            |config: &mut Config| config.provider = Provider::Ollama,
            false,
        ),
    ];
    for (label, change, supported) in cases {
        let env = Env::with_config_tool();
        let mut config = env.config(&url, Wire::Native);
        config.ollama_host = url.clone();
        change(&mut config);
        let mut client = client_from_config(&config, false).unwrap();
        assert_eq!(
            client.profile_tool_schema_wire() == ProfileToolSchemaWire::ChatCompletions,
            supported,
            "{label}"
        );
        // The OpenAI client has a fixed external endpoint, so it is never run here: a
        // supported row runs only on the client pointed at the fake.
        let session_result = (!supported || label == "lm-studio chat").then(|| {
            run_session(client.as_mut(), &mut SessionSnapshot::new(), "x", &config)
                .map_err(|error| format!("{error:#}"))
        });
        // The planner-only entry reads the same answer from the configuration.
        let mut planner = InjectedClient::default();
        let planner_result = generate_step_plan(&mut planner, PLAN_GOAL, &config)
            .map(|_| ())
            .map_err(|error| format!("{error:#}"));
        let refused = "profile_tool_refused: schema_unsupported";
        if supported {
            assert!(
                !format!("{session_result:?}").contains(refused),
                "{label}: {session_result:?}"
            );
            assert!(planner.calls() > 0, "{label}: {planner_result:?}");
        } else {
            let session_error = session_result.unwrap().unwrap_err();
            assert!(session_error.contains(refused), "{label}: {session_error}");
            assert!(planner_result.unwrap_err().contains(refused), "{label}");
            assert_eq!(planner.calls(), 0, "{label}");
        }
    }
    let (requests, _) = endpoint.finish();
    // Only the supported rows connected: lm-studio chat, once.
    assert_eq!(requests.len(), 1, "{requests:?}");
    set_env("GEMINI_API_KEY", None);
    set_env("OPENAI_API_KEY", None);
    set_env(REGISTRY_ENV, None);
}

#[test]
fn an_openai_compatible_role_override_decides_the_wire_not_the_provider_field() {
    let _guard = lock();
    let endpoint = FakeHttp::start(Vec::new());
    let base = format!("{}/v1", endpoint.url);
    // The session role is the override; the fields below it say Ollama.
    let env = Env::with_config_tool();
    let config = parse_config(
        &env,
        &[
            "--provider",
            "openai-compatible",
            "--base-url",
            &base,
            "--planner-provider",
            "ollama",
        ],
    );
    let mut client = client_from_config(&config, false).unwrap();
    let result = run_session(client.as_mut(), &mut SessionSnapshot::new(), "x", &config);
    assert!(
        !format!("{result:?}").contains("schema_unsupported"),
        "{result:?}"
    );
    let mut planner = InjectedClient::default();
    let _ = generate_step_plan(&mut planner, PLAN_GOAL, &config);
    assert!(planner.calls() > 0, "the executor role is what is judged");

    // The override with the Responses API is refused, whatever else is set.
    let env = Env::with_config_tool();
    let config = parse_config(
        &env,
        &[
            "--provider",
            "openai-compatible",
            "--base-url",
            &base,
            "--api",
            "responses",
        ],
    );
    let mut client = client_from_config(&config, false).unwrap();
    let error =
        run_session(client.as_mut(), &mut SessionSnapshot::new(), "x", &config).unwrap_err();
    assert!(
        format!("{error:#}").contains("schema_unsupported"),
        "{error:#}"
    );
    endpoint.finish();

    // A planner on another provider carries no schema tool, so it is not what decides.
    let env = Env::with_config_tool();
    let mut config = env.config("http://127.0.0.1:9", Wire::Native);
    config.planner_provider = Provider::Ollama;
    config.classifier_provider = Provider::Gemini;
    let mut planner = InjectedClient::default();
    let _ = generate_step_plan(&mut planner, PLAN_GOAL, &config);
    assert!(planner.calls() > 0);
    set_env(REGISTRY_ENV, None);
}

/// Every place a scrub can touch: a property, an enum value, a pattern, a required name.
#[test]
fn any_schema_value_a_scrub_would_change_is_refused_before_the_start() {
    let _guard = lock();
    for property in [
        json!({"a": {"type": "string", "enum": [DUMMY_SECRET]}}),
        json!({"a": {"type": "string", "pattern": DUMMY_SECRET}}),
        json!({DUMMY_SECRET: {"type": "null"}}),
        json!({"a": closed(json!({DUMMY_SECRET: {"type": "null"}}))}),
    ] {
        let env = Env::new();
        env.write_tool(schema_declaration(closed(property.clone())));
        let config = env.config("http://127.0.0.1:9", Wire::Native);
        install_dummy_secret(&env.root, &config);
        let mut client = client_from_config(&config, false).unwrap();
        let error =
            run_session(client.as_mut(), &mut SessionSnapshot::new(), "x", &config).unwrap_err();
        let error = format!("{error:#}");
        assert!(
            error.contains("profile_tool_refused: schema_scrubbed"),
            "{error}"
        );
        assert!(
            !error.contains(DUMMY_SECRET),
            "the refusal repeats the secret"
        );
    }
    // The same schema without the secret in it runs under the same scope.
    let env = Env::with_config_tool();
    let config = env.config("http://127.0.0.1:9", Wire::Native);
    install_dummy_secret(&env.root, &config);
    let (reader, _, result) = run_with_config(
        &config,
        vec![
            Wire::Native.call(CONFIG_TOOL, VALID_CONFIG),
            text_reply("done"),
        ],
        None,
    );
    assert_eq!(result.as_deref(), Ok("done"));
    assert_eq!(*reader.calls.lock().unwrap(), vec![parsed(VALID_CONFIG)]);
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_tool_that_needs_the_os_sandbox_stays_unavailable_with_a_valid_schema() {
    let _guard = lock();
    let env = Env::new();
    write_full_descriptor(
        env.registry.path(),
        CONFIG_TOOL_ID,
        CONFIG_TOOL,
        &schema_declaration(config_root()),
        "1.0.0",
        &["os_sandbox"],
    );
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: unavailable:"),
        "{error}"
    );
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_compiler_nobody_registered_or_that_fails_is_refused() {
    let _guard = lock();
    let env = Env::new();
    env.write_tool(json!({
        "protocol": "profile-tool.schema/v1",
        "root": closed(json!({})),
        "compiler": "never-registered",
    }));
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: unavailable:"),
        "{error}"
    );

    let env = Env::new();
    env.write_tool(json!({
        "protocol": "profile-tool.schema/v2",
        "root": closed(json!({})),
    }));
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: schema_unsupported:"),
        "{error}"
    );
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_flat_parameter_next_to_a_schema_and_a_repeated_key_in_a_descriptor_are_refused() {
    let _guard = lock();
    let env = Env::new();
    let schema = schema_declaration(closed(json!({})));
    let schema_text = serde_json::to_string(&schema).unwrap();
    let pin = sha256(schema_text.as_bytes());
    let directory = env.registry.path().join("profile-tools");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{CONFIG_TOOL_ID}.json"));
    let shape = |parameters: &str, schema_text: &str| {
        format!(
            r#"{{"protocol":"profile-tool.descriptor/v1","tool_id":"{CONFIG_TOOL_ID}","name":"{CONFIG_TOOL}","version":"1.0.0","description":"d","effect":"read","parameters":{parameters},"schema":{schema_text},"schema_sha256":"{pin}","runtime":{{"kind":"in_process","id":"{RUNTIME}"}},"limits":{{"max_input_bytes":1024,"max_output_bytes":4096}}}}"#
        )
    };
    install_reader();
    std::fs::write(&path, shape(FLAT_PARAMETERS, &schema_text)).unwrap();
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: invalid_descriptor:"),
        "{error}"
    );

    let repeated = schema_text.replacen(
        r#""type":"object""#,
        r#""type":"string","type":"object""#,
        1,
    );
    std::fs::write(&path, shape("[]", &repeated)).unwrap();
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: invalid_descriptor:"),
        "{error}"
    );

    std::fs::write(&path, shape("[]", r#"{"protocol":"other"}"#)).unwrap();
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("profile_tool_refused: invalid_pin:"),
        "{error}"
    );
    set_env(REGISTRY_ENV, None);
}

/// A depth past the artifact-contract v1 limit is read only for a schema-bearing
/// descriptor. A G1 descriptor at that depth keeps its existing refusal.
#[test]
fn the_v1_depth_limit_still_applies_to_a_descriptor_without_a_schema() {
    let _guard = lock();
    let env = Env::new();
    let directory = env.registry.path().join("profile-tools");
    std::fs::create_dir_all(&directory).unwrap();
    let deep = format!("{}1{}", "[".repeat(40), "]".repeat(40));
    let text = format!(
        r#"{{"protocol":"profile-tool.descriptor/v1","tool_id":"{CONFIG_TOOL_ID}","extra":{deep}}}"#
    );
    std::fs::write(directory.join(format!("{CONFIG_TOOL_ID}.json")), text).unwrap();
    install_reader();
    let error = admit_selected(&env.root, CONFIG_PROFILE, None).unwrap_err();
    assert!(error.to_string().contains("depth"), "{error}");
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// No fallback, and a usable prompt.
// ---------------------------------------------------------------------------

/// A malformed call to another tool in a schema session cannot start the native-to-XML
/// fallback, and a refused schema call never reaches a text or Bash path.
#[test]
fn a_schema_session_never_falls_back_to_xml() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let broken = tool_reply(&[("Read", "{not json")]);
    let run = session(
        &env,
        Wire::Native,
        vec![
            broken.clone(),
            broken.clone(),
            broken.clone(),
            broken.clone(),
            broken,
        ],
        None,
    );
    assert_eq!(run.executed, Vec::<Value>::new());
    let events = env.events();
    assert!(!events.contains(r#""allowed":true"#), "{events}");
    for request in &run.requests {
        let body: Value = serde_json::from_str(request).unwrap();
        assert!(
            body["tools"].is_array(),
            "a later request lost the native tools"
        );
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn each_wire_carries_the_adopted_schema_where_that_wire_reads_it() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let run = session(&env, Wire::Native, vec![text_reply("done")], None);
    let body: Value = serde_json::from_str(&run.requests[0]).unwrap();
    let sent = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["function"]["name"] == CONFIG_TOOL)
        .unwrap();
    assert_eq!(sent["function"]["parameters"], config_root());
    let names = body["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["function"]["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Read", CONFIG_TOOL]);
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert!(!system.contains("Arguments schema"), "{system}");

    let env = Env::with_config_tool();
    let run = session(&env, Wire::Xml, vec![text_reply("done")], None);
    let body: Value = serde_json::from_str(&run.requests[0]).unwrap();
    assert!(body.get("tools").is_none(), "{body}");
    let system = body["messages"][0]["content"].as_str().unwrap();
    let note = format!("Arguments schema: {}", config_root());
    assert!(system.contains(&note), "{system}");
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// Acceptance 4: two different synthetic profiles, one validator.
// ---------------------------------------------------------------------------

#[test]
fn a_config_object_and_a_document_index_use_the_same_validator() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::new();
        write_descriptor(
            env.registry.path(),
            INDEX_TOOL_ID,
            INDEX_TOOL,
            &schema_declaration(index_root()),
        );
        let mut config = base_config(&env.root, &env.evidence, INDEX_PROFILE, "http://unused");
        wire.configure(&mut config);
        let (reader, requests, result) = run_with_config(
            &config,
            vec![wire.call(INDEX_TOOL, VALID_INDEX), text_reply("done")],
            None,
        );
        assert_eq!(result.as_deref(), Ok("done"));
        assert_eq!(*reader.calls.lock().unwrap(), vec![parsed(VALID_INDEX)]);
        assert_eq!(requests.len(), 2);

        // The same refusal shape from a different schema.
        let env = Env::new();
        write_descriptor(
            env.registry.path(),
            INDEX_TOOL_ID,
            INDEX_TOOL,
            &schema_declaration(index_root()),
        );
        let mut config = base_config(&env.root, &env.evidence, INDEX_PROFILE, "http://unused");
        wire.configure(&mut config);
        let (reader, requests, _) = run_with_config(
            &config,
            vec![
                wire.call(INDEX_TOOL, r#"{"documents":[{"id":"a","id":"b","x":1}]}"#),
                text_reply("done"),
            ],
            None,
        );
        assert_eq!(reader.calls.lock().unwrap().len(), 0);
        let body: Value = serde_json::from_str(&requests[1]).unwrap();
        let feedback = body["messages"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            feedback.contains("\"code\":\"duplicate_key\""),
            "{feedback}"
        );
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_tool_that_reports_no_usage_stays_unknown() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let reader = install_reader();
    let admitted = admit_selected(&env.root, CONFIG_PROFILE, None)
        .unwrap()
        .unwrap();
    admitted
        .registry()
        .unwrap()
        .execute(CONFIG_TOOL, &parsed(VALID_CONFIG), &registry_context(&env))
        .unwrap();
    assert_eq!(reader.calls.lock().unwrap().len(), 1);
    let receipt = admitted.receipts().pop().unwrap();
    assert!(receipt.executed && receipt.ok);
    assert_eq!(receipt.usage, None);
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// The public entries that run a session, on both wires.
// ---------------------------------------------------------------------------

/// Answers a planner with the plan, the first request that can carry the tool with its
/// call, and everything after with final text.
fn entry_responder(wire: Wire, raw: String, first_plan: Option<String>) -> Responder {
    let sent = Arc::new(AtomicBool::new(false));
    let first_plan = Mutex::new(first_plan);
    Arc::new(move |_, body| {
        let carries = body.contains(&format!("\"name\":\"{CONFIG_TOOL}\""))
            || body.contains("Arguments schema");
        Some(if !carries {
            // The first planner answer of a generating entry, then each phase's steps.
            let plan = first_plan.lock().unwrap().take();
            text_reply(&plan.unwrap_or_else(step_plan_reply))
        } else if sent.swap(true, Ordering::SeqCst) {
            text_reply("done")
        } else {
            wire.call(CONFIG_TOOL, &raw)
        })
    })
}

fn entry_plan(entry: &str) -> Option<String> {
    let generates = entry.contains("generate_and_run")
        || entry.starts_with("/ultra-plan-run")
        || entry == "gate-one";
    let ultra = entry.contains("ultra") || entry.contains("auto_recovery") || entry == "gate-one";
    (generates && ultra).then(|| render_ultra_plan(&ultra_plan(CONFIG_PROFILE)))
}

/// A probe's sessions send the tool. Its JSON and classifier parts stay without it, and
/// what they conclude from these replies is their own: only the call is measured here.
#[test]
fn a_model_probe_session_delivers_a_call_on_both_wires() {
    let _guard = lock();
    for wire in WIRES {
        let env = Env::with_config_tool();
        let reader = install_reader();
        let http = FakeHttp::with_responder(
            Vec::new(),
            entry_responder(wire, VALID_CONFIG.to_string(), None),
        );
        let config = env.config(&http.url, wire);
        let mut planner = client_from_config(&config, true).unwrap();
        let mut execution = client_from_config(&config, false).unwrap();
        let outcome = commandagent::model_probe::run_with_output_dir(
            &config,
            planner.as_mut(),
            execution.as_mut(),
            env.evidence.join("probe-output"),
        );
        http.finish();
        assert_eq!(
            *reader.calls.lock().unwrap(),
            vec![parsed(VALID_CONFIG)],
            "{wire:?}: {:?}",
            outcome.map(|_| ())
        );
    }
    set_env(REGISTRY_ENV, None);
}

/// Receipts pair with calls in order, so a valid call cannot lend its receipt to an
/// invalid one beside it.
#[test]
fn two_calls_in_one_reply_keep_their_own_receipts() {
    let _guard = lock();
    let invalid = r#"{"mode":"slow","limits":{"retries":1},"targets":[null]}"#;
    let env = Env::with_config_tool();
    let run = session(
        &env,
        Wire::Native,
        vec![
            tool_reply(&[(CONFIG_TOOL, invalid), (CONFIG_TOOL, VALID_CONFIG)]),
            text_reply("done"),
        ],
        None,
    );
    assert_eq!(run.executed, vec![parsed(VALID_CONFIG)]);
    assert!(
        run.requests[1].contains("enum_mismatch"),
        "{}",
        run.requests[1]
    );
    set_env(REGISTRY_ENV, None);
}

/// A workflow's node runs its session in this process and receives the same way.
#[test]
fn a_library_workflow_child_delivers_a_call_on_both_wires() {
    let private = tempfile::tempdir().unwrap();
    let home = private.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    if !in_own_process(
        "a_library_workflow_child_delivers_a_call_on_both_wires",
        &home,
    ) {
        return;
    }
    let _guard = lock();
    let root = std::fs::canonicalize(private.path()).unwrap();
    for wire in WIRES {
        let env = Env::with_config_tool();
        let run_root = root.join(format!("{wire:?}"));
        std::fs::create_dir_all(&run_root).unwrap();
        let (origin, definition) = workflow_origin(&run_root, CONFIG_PROFILE);
        let reader = install_reader();
        let http = FakeHttp::with_responder(
            Vec::new(),
            entry_responder(
                wire,
                VALID_CONFIG.to_string(),
                Some(render_ultra_plan(&ultra_plan(CONFIG_PROFILE))),
            ),
        );
        let config = env.config(&http.url, wire);
        let outcome =
            commandagent::workflow::orchestrator::run_workflow(&config, &definition, &origin);
        http.finish();
        assert_eq!(
            *reader.calls.lock().unwrap(),
            vec![parsed(VALID_CONFIG)],
            "{wire:?}: {:?}",
            outcome.map_err(|error| format!("{error:#}"))
        );
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn every_session_entry_delivers_a_valid_call_unchanged_and_refuses_an_invalid_one() {
    let _guard = lock();
    let invalid = r#"{"mode":"fast","mode":"slow","limits":{"retries":1},"targets":[null]}"#;
    let mut failures = Vec::new();
    for wire in WIRES {
        for entry in SESSION_ENTRIES {
            for (raw, expected) in [
                (VALID_CONFIG, vec![parsed(VALID_CONFIG)]),
                (invalid, Vec::new()),
            ] {
                let env = Env::with_config_tool();
                write_plans(&env, CONFIG_PROFILE);
                let reader = install_reader();
                let http = FakeHttp::with_responder(
                    Vec::new(),
                    entry_responder(wire, raw.to_string(), entry_plan(entry)),
                );
                let config = env.config(&http.url, wire);
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
                let (requests, _) = http.finish();
                let executed = reader.calls.lock().unwrap().clone();
                if executed != expected {
                    failures.push(format!(
                        "{wire:?} {entry} expected {} executed {} requests={} outcome={outcome:?}",
                        expected.len(),
                        executed.len(),
                        requests.len()
                    ));
                }
                let refused = requests
                    .iter()
                    .any(|request| request.contains("duplicate_key"));
                if expected.is_empty() != refused {
                    failures.push(format!("{wire:?} {entry}: feedback refused={refused}"));
                }
            }
        }
    }
    set_env(REGISTRY_ENV, None);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A repair session in one run reuses the adoption: a changed input stops the repair
/// instead of adopting again, through the public step-plan entry.
#[test]
fn a_changed_input_stops_a_step_plan_run_without_a_new_adoption() {
    let _guard = lock();
    let env = Env::new();
    let revision = compiled_tool(&env);
    write_plans(&env, CONFIG_PROFILE);
    let reader = install_reader();
    let call = bound_call(&revision, "alpha");
    let path = env.root.join("inputs/ids.txt");
    let responder: Responder = Arc::new(move |_, _| {
        std::fs::write(&path, "changed\n").unwrap();
        Some(Wire::Native.call(CONFIG_TOOL, &call))
    });
    let http = FakeHttp::with_responder(Vec::new(), responder);
    let config = env.config(&http.url, Wire::Native);
    let mut client = client_from_config(&config, false).unwrap();
    let outcome = run_step_plan(client.as_mut(), &inspect_plan(), &config);
    http.finish();
    assert_eq!(reader.calls.lock().unwrap().len(), 0);
    let text = format!("{outcome:?}");
    assert!(text.contains("input_revision_mismatch"), "{text}");
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// A workflow and the binary: the registry is empty there, as in G1.
// ---------------------------------------------------------------------------

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
            "workflow: schema-child\nversion: 0.2\nentry: create\nnodes:\n  create: {{ intent: create, profile: generic }}\n  rebuild: {{ intent: create, profile: {child_profile} }}\nroutes:\n  - {{ from: create, on: failed, when: recovery_yaml_present, to: rebuild, carry: [workspace, recovery_yaml] }}\nterminal: {{}}\n"
        ),
    )
    .unwrap();
    (origin, definition)
}

const OWN_PROCESS_ENV: &str = "COMMANDAGENT_TEST_973_OWN_PROCESS";

/// What a workflow installs (its sandbox) is process-wide, so this runs alone.
fn in_own_process(test: &str, home: &Path) -> bool {
    if std::env::var_os(OWN_PROCESS_ENV).is_some() {
        return true;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(OWN_PROCESS_ENV, "1")
        .env("HOME", home)
        .status()
        .unwrap();
    assert!(status.success(), "{status}");
    false
}

#[test]
fn a_library_workflow_refuses_on_its_origin_and_its_workspace_before_the_reproducer() {
    let private = tempfile::tempdir().unwrap();
    let home = private.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    if !in_own_process(
        "a_library_workflow_refuses_on_its_origin_and_its_workspace_before_the_reproducer",
        &home,
    ) {
        return;
    }
    let _guard = lock();
    install_reader();
    let root = std::fs::canonicalize(private.path()).unwrap();
    for (label, prepare) in [
        (
            "unsupported provider",
            (|env: &Env, config: &mut Config| {
                env.write_tool(schema_declaration(config_root()));
                config.provider = Provider::Ollama;
            }) as fn(&Env, &mut Config),
        ),
        (
            "unsupported keyword",
            (|env: &Env, _: &mut Config| {
                write_with_property(env, json!({"not": {}}));
            }) as fn(&Env, &mut Config),
        ),
    ] {
        let env = Env::new();
        let run_root = root.join(label.replace(' ', "-"));
        std::fs::create_dir_all(&run_root).unwrap();
        let (origin, definition) = workflow_origin(&run_root, CONFIG_PROFILE);
        let endpoint = CountingEndpoint::start();
        let mut config = env.config(&endpoint.url, Wire::Native);
        config.ollama_host = endpoint.url.clone();
        prepare(&env, &mut config);
        let error =
            commandagent::workflow::orchestrator::run_workflow(&config, &definition, &origin)
                .unwrap_err();
        let error = format!("{error:#}");
        assert!(
            error.contains("profile_tool_refused: schema_unsupported"),
            "{label}: {error}"
        );
        assert_eq!(endpoint.finish(), 0, "{label}");
        assert!(!origin.join(REPRODUCER_MARKER).exists(), "{label}");
        assert!(
            !origin.join("evidence/workflow-events.jsonl").exists(),
            "{label}"
        );
        assert_eq!(env.events(), "", "{label}");
    }
    set_env(REGISTRY_ENV, None);
}

/// The binary registers no runtime, as in G1, so a schema tool cannot be admitted
/// through it. Whatever else is wrong, nothing reaches a provider or Jev.
fn binary(
    root: &Path,
    registry: &Path,
    arguments: &[&str],
    extra: &[(&str, &str)],
) -> (Option<i32>, String, usize, usize) {
    let provider = CountingEndpoint::start();
    let jev = CountingEndpoint::start();
    let home = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .arg("--extension-root")
        .arg(extension())
        .args(["--provider", "lm-studio", "--lm-studio-host", &provider.url])
        .args([
            "--ollama-host",
            &provider.url,
            "--model",
            "m",
            "--chat-retries",
            "0",
        ])
        .args(["--request-pattern", "jev"])
        .args(arguments)
        .current_dir(root)
        .env_clear()
        .env("HOME", home.path())
        .env(REGISTRY_ENV, registry)
        .env("JEV_API_KEY", "dummy-973-not-a-real-credential")
        .env("HTTPS_PROXY", &jev.url);
    if let Some(path) = std::env::var_os("PATH") {
        command.env("PATH", path);
    }
    for (name, value) in extra {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        provider.finish(),
        jev.finish(),
    )
}

#[test]
fn the_binary_refuses_a_schema_tool_before_a_provider_or_jev_on_each_action() {
    let _guard = lock();
    let env = Env::with_config_tool();
    write_plans(&env, CONFIG_PROFILE);
    for (label, arguments) in [
        (
            "prompt",
            vec![
                "--profile",
                CONFIG_PROFILE,
                "--prompt",
                "Submit the object.",
            ],
        ),
        ("repl", vec!["--profile", CONFIG_PROFILE]),
        (
            "run-plan",
            vec!["--profile", CONFIG_PROFILE, "--run-plan", "inspect.yaml"],
        ),
    ] {
        let (code, stderr, provider, jev) = binary(&env.root, env.registry.path(), &arguments, &[]);
        assert_eq!(code, Some(2), "{label}: {stderr}");
        assert!(
            stderr.contains("profile_tool_refused: unavailable"),
            "{label}: {stderr}"
        );
        assert_eq!((provider, jev), (0, 0), "{label}");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn a_binary_workflow_child_on_a_schema_tool_is_refused_before_its_provider_call() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let private = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(private.path()).unwrap();
    let outer = root.join("outer");
    std::fs::create_dir_all(&outer).unwrap();
    let (origin, definition) = workflow_origin(&root, CONFIG_PROFILE);
    let provider = CountingEndpoint::start();
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--extension-root")
        .arg(extension())
        .args(["--provider", "lm-studio", "--lm-studio-host", &provider.url])
        .args(["--model", "m"])
        .arg("--workflow")
        .arg(&definition)
        .arg("--origin")
        .arg(&origin)
        .current_dir(&outer)
        .env_clear()
        .env("HOME", &home)
        .env(REGISTRY_ENV, env.registry.path())
        .output()
        .unwrap();
    let circle: Value = serde_json::from_str(
        &std::fs::read_to_string(origin.join("evidence/workflow-circle.json")).unwrap(),
    )
    .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_eq!(provider.finish(), 0, "{stderr}");
    assert_eq!(circle["verdict"], "circle_failed", "{circle}");
    assert!(circle.to_string().contains("node_failed:"), "{circle}");
    set_env(REGISTRY_ENV, None);
}

// ---------------------------------------------------------------------------
// Acceptance 5: off, byte for byte against the base recording.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct FlatReader;

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

/// The temporary roots, the loopback port, generated IDs, and the timings inside text.
/// Nothing is left out of the comparison.
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

/// The words Issue #973 reserves, in text the dummy secret also appears in.
const RESERVED_WORDS_PROMPT: &str = "Check fake-dummy-credential-973-not-a-real-key next to profile-tool.schema/v1 schema_unsupported schema_limit_exceeded schema_scrubbed schema_mismatch schema_violation input_revision_mismatch input_revision_unavailable duplicate_key malformed_json trailing_data unknown_key union_no_match.";

const OFF_ROWS: [OffRow; 7] = [
    OffRow {
        id: "chat-native-recovery-and-last-value",
        profile: "generic",
        text_protocol: false,
        prompt: "Write notes.txt and read it.",
        replies: || {
            vec![
                tool_reply(&[(
                    "Write",
                    r#"{"path":"notes.txt","content":"first","content":"last"}"#,
                )]),
                tool_reply(&[("Read", r#"{"arguments":{"path":"notes.txt"}}"#)]),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "text-xml-positive",
        profile: "generic",
        text_protocol: true,
        prompt: "Write notes.txt and read it.",
        replies: || {
            vec![
                text_reply(
                    "<anvil_tool_call name=\"Write\">{\"path\":\"notes.txt\",\"content\":\"a\"}</anvil_tool_call>",
                ),
                text_reply(
                    "<tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"notes.txt\"}}</tool_call>",
                ),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "text-xml-relaxed-and-repaired",
        profile: "generic",
        text_protocol: true,
        prompt: "Write notes.txt and read it.",
        replies: || {
            vec![
                text_reply(
                    "<anvil_tool_call name=\"Write\">{'path':'notes.txt','content':'a',}</anvil_tool_call>",
                ),
                text_reply(
                    "<anvil_tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"notes.txt\"}} trailing</anvil_tool_call>",
                ),
                text_reply("<tool_call>{\"name\":\"Read\",\"arguments\":{\"path\":\"notes.txt\"}}"),
                text_reply("done"),
            ]
        },
    },
    OffRow {
        id: "text-xml-negative",
        profile: "generic",
        text_protocol: true,
        prompt: "Write notes.txt.",
        replies: || {
            vec![text_reply(
                "<anvil_tool_call name=\"NoSuchTool\">{\"a\":1}</anvil_tool_call>",
            )]
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
                tool_reply(&[("ReadFlatConfig", r#"{"path":7}"#)]),
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
                    r#"{"path":"notes.txt","content":"schema_violation fake-dummy-credential-973-not-a-real-key duplicate_key"}"#,
                )]),
                text_reply("schema_unsupported done"),
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

fn assert_off_row(row: &OffRow) {
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

#[test]
fn off_wire_events_and_artifacts_match_the_base_recording_byte_for_byte() {
    let _guard = lock();
    extension();
    for row in &OFF_ROWS {
        assert_off_row(row);
    }
}

/// With no schema tool selected, nothing reads the registry; with a G2 tool in another
/// profile, a flat row stays as recorded too.
#[test]
fn off_takes_no_read_of_a_registry_variable_that_names_nothing() {
    let _guard = lock();
    extension();
    set_env(REGISTRY_ENV, Some("/nonexistent/profile-tool-registry"));
    let (workspace, evidence) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let http = FakeHttp::start(vec![text_reply("done")]);
    let config = base_config(workspace.path(), evidence.path(), "generic", &http.url);
    let mut client = client_from_config(&config, false).unwrap();
    let result = run_session(
        client.as_mut(),
        &mut SessionSnapshot::new(),
        "Say done.",
        &config,
    );
    assert_eq!(result.unwrap(), "done");
    http.finish();
    set_env(REGISTRY_ENV, None);
}

/// A flat tool keeps every old path next to a selected schema tool's: the old parser's
/// last value, and the recovery, reach it. Only the schema tool's name is strict.
#[test]
fn a_builtin_in_a_schema_session_keeps_the_existing_parser() {
    let _guard = lock();
    let env = Env::with_config_tool();
    std::fs::write(env.root.join("notes.txt"), "text").unwrap();
    let run = session(
        &env,
        Wire::Native,
        vec![
            tool_reply(&[("Read", r#"{"path":"missing.txt","path":"notes.txt"}"#)]),
            tool_reply(&[("Read", r#"{"arguments":{"path":"notes.txt"}}"#)]),
            text_reply("done"),
        ],
        None,
    );
    assert_eq!(run.result.as_deref(), Ok("done"));
    // The last value won, and the wrapper was unwrapped: both reads found the file.
    assert_eq!(run.last_message(1), "notes.txt\ntext");
    assert!(
        run.last_message(2).contains("text"),
        "{}",
        run.last_message(2)
    );
    assert_eq!(run.executed, Vec::<Value>::new());

    let env = Env::with_config_tool();
    std::fs::write(env.root.join("notes.txt"), "text").unwrap();
    let run = session(
        &env,
        Wire::Xml,
        vec![
            text_reply("<anvil_tool_call name=\"Read\">{'path':'notes.txt',}</anvil_tool_call>"),
            text_reply("done"),
        ],
        None,
    );
    assert_eq!(
        run.result.as_deref(),
        Ok("done"),
        "the relaxed parser went away"
    );
    set_env(REGISTRY_ENV, None);
}

/// A flag on a slash command names the profile; the guard judges that one.
#[test]
fn a_profile_named_by_a_slash_flag_is_refused_the_same_way() {
    let _guard = lock();
    let env = Env::with_config_tool();
    let config = base_config(&env.root, &env.evidence, "generic", "http://127.0.0.1:9");
    for line in [
        format!("/plan-run --profile {CONFIG_PROFILE} Submit the object"),
        format!("/ultra-plan-run --profile {CONFIG_PROFILE} Submit the object"),
    ] {
        let mut planner = InjectedClient::default();
        let mut execution = InjectedClient::default();
        let error =
            handle_command(&line, &config, &mut planner, &mut execution, &NOOP_UI).unwrap_err();
        let error = format!("{error:#}");
        assert!(
            error.contains("profile_tool_refused: schema_unsupported"),
            "{line}: {error}"
        );
        assert_eq!(planner.calls() + execution.calls(), 0, "{line}");
    }
    set_env(REGISTRY_ENV, None);
}

#[test]
fn the_corpus_manifest_pins_its_fixtures() {
    let manifest: Value = serde_json::from_str(include_str!(
        "corpus/apps/profile-tool-schema/source-manifest.json"
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
        "29d4d369536eda948a226a1af74d6038a2203389"
    );
}

#[test]
fn the_corpus_names_what_this_binary_runs() {
    let cases: Value = serde_json::from_str(CASES).unwrap();
    let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
    let off_ids = golden["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        off_ids,
        OFF_ROWS
            .iter()
            .map(|row| row.id.to_string())
            .collect::<Vec<_>>()
    );
    let listed = cases["off"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(listed, off_ids);
    let rejected = cases["rejected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap().to_string(),
                row["raw"].as_str().unwrap().to_string(),
                row["code"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(rejected.len(), REJECTED.len());
    for (listed, (id, raw, code)) in rejected.iter().zip(REJECTED) {
        assert_eq!(
            (listed.0.as_str(), listed.1.as_str(), listed.2.as_str()),
            (*id, *raw, *code)
        );
    }
    assert_eq!(cases["accepted"][0]["raw"], VALID_CONFIG);
    assert_eq!(cases["accepted"][1]["raw"], VALID_INDEX);
    let refusals = cases["refused_before_start"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["id"].as_str().unwrap(),
                row["refusal"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        refusals,
        START_REFUSALS
            .iter()
            .map(|refusal| (refusal.label, refusal.kind))
            .collect::<Vec<_>>()
    );
}
