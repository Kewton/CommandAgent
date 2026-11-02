//! Issue #972 (G1): a selected profile or pack references opaque tool IDs and an
//! operator registry outside the workspace owns what they mean.
//!
//! Every case runs through the public minimal loop or the public step-plan
//! planner with a scripted `ChatClient`. No model, key, provider, network, or
//! external process is used. The executors are in-process fakes. Scenarios are
//! listed in `tests/corpus/apps/profile-tool-registration/`; the off controls
//! compare with `fixtures/off-golden.json`, recorded from base 104a166b before
//! this feature existed.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::planner::pack::exact_byte_hash;
use commandagent::planner::runner::{generate_step_plan, run_plan_file};
use commandagent::planner::step_plan::{PlanStep, StepPlan, render_step_plan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, SessionSnapshot, ToolCall};
use commandagent::tools::os_sandbox::{
    SandboxAvailability, SandboxMode, SandboxPolicy, install_for_test, reset_for_test,
};
use commandagent::tools::profile_tools::{
    ProfileToolReadExecutor, ReadToolOutput, ToolExecutionReceipt, ToolUsage, admit_selected,
    register_read_executor,
};
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CASES: &str = include_str!("corpus/apps/profile-tool-registration/fixtures/cases.json");
const OFF_GOLDEN: &str =
    include_str!("corpus/apps/profile-tool-registration/fixtures/off-golden.json");
const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
const PACK_ENVS: [&str; 4] = [
    "COMMANDAGENT_PACK_DIRECTORY",
    "COMMANDAGENT_PACK_ID",
    "COMMANDAGENT_PACK_VERSION",
    "COMMANDAGENT_PACK_HASH",
];
const SCHEMA: &str = "profile-tool.declaration/v1";
const OFF_DRAFT: &str = "tool-off-draft";
const OFF_PACK: &str = "tool-off-pack";
const TOOL_PACK: &str = "tool-select-pack";
const PACK_HOST: &str = "tool-pack-host";
const OTHER_PACK: &str = "tool-other-pack";
const DEFAULT_TOOLS: [&str; 6] = ["Bash", "Read", "Write", "Edit", "Glob", "Grep"];
/// The compact JSON `schema_sha256` pins, written out here so the expected pin
/// does not come from the implementation under test.
const PATH_PARAMETERS: &str = r#"[{"name":"path","kind":"workspace_path","required":true,"description":"Workspace-relative file."}]"#;

/// Every env and process-wide state mutation here holds this lock.
static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn set_env(name: &str, value: Option<&str>) {
    // SAFETY: every env mutation in this binary holds `LOCK`, and no test here
    // reads the environment without it.
    unsafe {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
}

fn clear_selection() {
    set_env(REGISTRY_ENV, None);
    for name in PACK_ENVS {
        set_env(name, None);
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// ---------------------------------------------------------------------------
// The scripted provider and the fake executors.
// ---------------------------------------------------------------------------

type BeforeReply = Arc<dyn Fn(usize) + Send + Sync>;

#[derive(Clone)]
struct ScriptedClient {
    native: bool,
    replies: Arc<Mutex<Vec<AssistantReply>>>,
    requests: Arc<Mutex<Vec<Value>>>,
    tools: Arc<Mutex<Vec<Vec<String>>>>,
    before_reply: Option<BeforeReply>,
}

impl ScriptedClient {
    fn new(replies: Vec<AssistantReply>) -> Self {
        Self {
            native: true,
            replies: Arc::new(Mutex::new(replies)),
            requests: Arc::default(),
            tools: Arc::default(),
            before_reply: None,
        }
    }

    fn calls(&self) -> usize {
        self.requests.lock().unwrap().len()
    }

    fn tools_of(&self, request: usize) -> Vec<String> {
        self.tools.lock().unwrap()[request].clone()
    }
}

impl ChatClient for ScriptedClient {
    fn label(&self) -> &str {
        "scripted"
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
        tools: &[commandagent::tools::registry::ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let call = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(json!({
                "messages": serde_json::to_string(messages).unwrap(),
                "tools": serde_json::to_string(tools).unwrap(),
                "native": native_tools_enabled,
            }));
            requests.len()
        };
        self.tools.lock().unwrap().push(
            tools
                .iter()
                .map(|tool| tool.function.name.clone())
                .collect(),
        );
        if let Some(hook) = &self.before_reply {
            hook(call);
        }
        let mut replies = self.replies.lock().unwrap();
        if replies.is_empty() {
            anyhow::bail!("scripted client exhausted");
        }
        Ok(replies.remove(0))
    }
}

fn reply(calls: Vec<(&str, Value)>) -> AssistantReply {
    AssistantReply {
        content: String::new(),
        tool_calls: calls
            .into_iter()
            .enumerate()
            .map(|(index, (name, arguments))| ToolCall {
                id: format!("call-{index}"),
                name: name.to_string(),
                arguments,
            })
            .collect(),
        prompt_tokens: None,
        completion_tokens: None,
    }
}

fn final_text(text: &str) -> AssistantReply {
    AssistantReply::text(text)
}

#[derive(Debug)]
struct FakeReader {
    calls: AtomicUsize,
    output: Result<String, String>,
    usage: Option<ToolUsage>,
}

impl ProfileToolReadExecutor for FakeReader {
    fn read(&self, _workspace_root: &Path, _arguments: &Value) -> anyhow::Result<ReadToolOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.output {
            Ok(text) => Ok(ReadToolOutput {
                text: text.clone(),
                usage: self.usage,
            }),
            Err(reason) => anyhow::bail!("{reason}"),
        }
    }
}

fn install_reader(
    runtime: &str,
    output: Result<&str, &str>,
    usage: Option<ToolUsage>,
) -> Arc<FakeReader> {
    let reader = Arc::new(FakeReader {
        calls: AtomicUsize::new(0),
        output: output.map(str::to_string).map_err(str::to_string),
        usage,
    });
    register_read_executor(runtime, reader.clone());
    reader
}

// ---------------------------------------------------------------------------
// Extension root: every profile and pack, registered once per process.
// ---------------------------------------------------------------------------

fn manifest(profile: &str, declaration: &str) -> String {
    format!(
        r#"[metadata]
id = "{profile}"
display_name = "Tool registration {profile}"
schema_version = "v2"
task_family = "generic"
[plan]
intent = "create"
phases = [
  {{ id = "implementation", prompt = "Write the requested notes for {{goal}}." }},
]
[artifacts]
required = ["notes.txt"]
[guidance.variants.contract]
triggers = [{{ condition = "always" }}]
messages = {{ instruction = "Write notes.txt." }}
[[checks.final]]
id = "scaffold_files_present"
params = {{ files = ["notes.txt"] }}
{declaration}"#
    )
}

fn tool_declaration(builtins: &str, entries: &[&str]) -> String {
    let tools = entries
        .iter()
        .map(|entry| format!("  {{ {entry} }},\n"))
        .collect::<String>();
    format!(
        "[profile_tools]\nschema_version = \"{SCHEMA}\"\nbuiltin_tools = [{builtins}]\ntools = [\n{tools}]\n"
    )
}

fn read_entry(id: &str) -> String {
    format!(r#"id = "{id}", effect = "read""#)
}

struct Extension {
    root: PathBuf,
}

/// Each rejection row in the corpus names a profile declared here.
fn declaring_profiles() -> Vec<(&'static str, String)> {
    let one = |id: &str| tool_declaration(r#""Read""#, &[&read_entry(id)]);
    vec![
        ("tool-config-a", one("config-json-read")),
        (
            "tool-index-b",
            tool_declaration(r#""Read", "Grep""#, &[&read_entry("doc-index-read")]),
        ),
        ("tool-unknown-id", one("never-registered")),
        ("tool-collision", one("collides-with-read")),
        ("tool-collision-removed", one("collides-with-write")),
        (
            "tool-collision-pair",
            tool_declaration(
                r#""Read""#,
                &[&read_entry("config-json-read"), &read_entry("same-name")],
            ),
        ),
        ("tool-bad-schema-pin", one("bad-schema-pin")),
        ("tool-bad-dependency-pin", one("bad-dependency-pin")),
        ("tool-swapped-dependency", one("swapped-dependency")),
        ("tool-symlink-descriptor", one("symlink-descriptor")),
        ("tool-workspace-dependency", one("workspace-dependency")),
        (
            "tool-effect-g3",
            tool_declaration(
                r#""Read""#,
                &[r#"id = "propose-artifacts", effect = "propose-artifacts""#],
            ),
        ),
        ("tool-nested-g2", one("nested-argument")),
        (
            "tool-declared-feature",
            tool_declaration(
                r#""Read""#,
                &[r#"id = "config-json-read", effect = "read", requires = ["nested_schema"]"#],
            ),
        ),
        (
            "tool-effect-conflict",
            tool_declaration(
                r#""Read""#,
                &[r#"id = "config-json-read", effect = "propose-artifacts""#],
            ),
        ),
        (
            "tool-wide-limit",
            tool_declaration(
                r#""Read""#,
                &[r#"id = "config-json-read", effect = "read", max_output_bytes = 999999"#],
            ),
        ),
        ("tool-no-runtime", one("no-runtime")),
        ("tool-process-runtime", one("process-runtime")),
        ("tool-os-required", one("os-required")),
        (
            "tool-keeps-write",
            tool_declaration(r#""Write", "Read""#, &[]),
        ),
        (
            "tool-narrow-output",
            tool_declaration(
                r#""Read""#,
                &[r#"id = "config-json-read", effect = "read", max_output_bytes = 4"#],
            ),
        ),
        ("tool-failing", one("failing-read")),
        ("tool-pack-conflict", one("config-json-read")),
    ]
}

fn pack_assist(id: &str, profile: &str, body: &str) -> String {
    format!(
        "schema_version: commandagent.pack.assist/v0\npack:\n  id: {id}\n  version: 1.0.0\n  profile: {profile}\n  intent: create\n{body}"
    )
}

fn pack_eval(id: &str, profile: &str, body: &str) -> String {
    format!(
        "schema_version: commandagent.pack.eval/v0\npack:\n  id: {id}\n  version: 1.0.0\n  profile: {profile}\n  intent: create\n{body}"
    )
}

fn pack_tools(builtins: &str, tool: &str) -> String {
    format!(
        "profile_tools:\n  schema_version: {SCHEMA}\n  builtin_tools: [{builtins}]\n  tools:\n    - id: {tool}\n      effect: read\n"
    )
}

const OFF_LITERAL: &str = "literals:\n  - gate: ingest_source_binding\n    example:\n      format: text\n      value: x\n";

/// Writes one member, named by its schema, and the pin of its exact bytes.
fn write_pack(root: &Path, id: &str, document: &str) {
    let directory = root.join(format!("packs/{id}/1.0.0"));
    std::fs::create_dir_all(&directory).unwrap();
    let (file, hash) = if document.contains("commandagent.pack.eval/v0") {
        (
            "eval.yaml",
            exact_byte_hash(None, Some(document.as_bytes())),
        )
    } else {
        (
            "assist.yaml",
            exact_byte_hash(Some(document.as_bytes()), None),
        )
    };
    std::fs::write(directory.join(file), document).unwrap();
    std::fs::write(directory.join("pack.sha256"), format!("{hash}\n")).unwrap();
}

fn extension() -> &'static Extension {
    static EXTENSION: OnceLock<Extension> = OnceLock::new();
    EXTENSION.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        let mut profiles = declaring_profiles();
        profiles.push((OFF_DRAFT, String::new()));
        profiles.push((PACK_HOST, String::new()));
        for (profile, declaration) in profiles {
            let directory = root.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("manifest.toml"),
                manifest(profile, &declaration),
            )
            .unwrap();
        }
        write_pack(
            &root,
            OFF_PACK,
            &pack_assist(OFF_PACK, OFF_DRAFT, OFF_LITERAL),
        );
        write_pack(
            &root,
            TOOL_PACK,
            &pack_eval(
                TOOL_PACK,
                PACK_HOST,
                &pack_tools("Read, Glob", "doc-index-read"),
            ),
        );
        write_pack(
            &root,
            OTHER_PACK,
            &pack_eval(
                OTHER_PACK,
                OFF_DRAFT,
                &pack_tools("Read", "config-json-read"),
            ),
        );
        write_pack(
            &root,
            "tool-conflict-pack",
            &pack_eval(
                "tool-conflict-pack",
                "tool-pack-conflict",
                &pack_tools("Read, Grep", "config-json-read"),
            ),
        );
        write_pack(
            &root,
            "tool-narrow-pack",
            &pack_eval(
                "tool-narrow-pack",
                "tool-keeps-write",
                &pack_tools("Read, Glob", "doc-index-read"),
            ),
        );
        commandagent::planner::extension_profiles::register(&root).unwrap();
        Extension { root }
    })
}

fn select_pack(id: &str) -> PathBuf {
    let directory = extension().root.join(format!("packs/{id}/1.0.0"));
    let loaded = commandagent::planner::pack::load_directory(&directory).unwrap();
    set_env(PACK_ENVS[0], Some(directory.to_str().unwrap()));
    set_env(PACK_ENVS[1], Some(id));
    set_env(PACK_ENVS[2], Some("1.0.0"));
    set_env(PACK_ENVS[3], Some(&loaded.hash));
    directory
}

// ---------------------------------------------------------------------------
// The operator registry: private to each test, outside each workspace.
// ---------------------------------------------------------------------------

struct Operator {
    _dir: tempfile::TempDir,
    root: PathBuf,
    dependency: PathBuf,
}

fn descriptor(id: &str, name: &str, runtime: &str) -> Value {
    json!({
        "protocol": "profile-tool.descriptor/v1",
        "tool_id": id,
        "name": name,
        "version": "1.0.0",
        "description": format!("Read through {id}."),
        "effect": "read",
        "parameters": serde_json::from_str::<Value>(PATH_PARAMETERS).unwrap(),
        "schema_sha256": sha256(PATH_PARAMETERS.as_bytes()),
        "runtime": {"kind": "in_process", "id": runtime},
        "limits": {"max_input_bytes": 1024, "max_output_bytes": 4096},
    })
}

impl Operator {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("profile-tools")).unwrap();
        let dependency = root.join("runtime.bin");
        std::fs::write(&dependency, "runtime-v1").unwrap();
        let operator = Self {
            _dir: dir,
            root,
            dependency,
        };
        operator.write("config-json-read", {
            let mut value = descriptor("config-json-read", "ReadConfigObject", "fake-config");
            value["dependencies"] = operator.dependency_pin();
            value
        });
        operator.write(
            "doc-index-read",
            descriptor("doc-index-read", "ReadDocIndex", "fake-index"),
        );
        operator.write(
            "collides-with-read",
            descriptor("collides-with-read", "Read", "fake-config"),
        );
        operator.write(
            "collides-with-write",
            descriptor("collides-with-write", "Write", "fake-config"),
        );
        operator.write(
            "same-name",
            descriptor("same-name", "ReadConfigObject", "fake-config"),
        );
        operator.write("bad-schema-pin", {
            let mut value = descriptor("bad-schema-pin", "BadSchema", "fake-config");
            value["schema_sha256"] = json!("0".repeat(64));
            value
        });
        operator.write("bad-dependency-pin", {
            let mut value = descriptor("bad-dependency-pin", "BadDependency", "fake-config");
            value["dependencies"] = json!([{
                "path": operator.dependency,
                "sha256": "0".repeat(64),
            }]);
            value
        });
        operator.write("swapped-dependency", {
            let mut value = descriptor("swapped-dependency", "Swapped", "fake-config");
            value["dependencies"] = json!([{
                "path": operator.dependency,
                "sha256": sha256(b"an earlier runtime"),
            }]);
            value
        });
        operator.write("propose-artifacts", {
            let mut value = descriptor("propose-artifacts", "ProposeArtifacts", "fake-config");
            value["effect"] = json!("propose-artifacts");
            value
        });
        operator.write("nested-argument", {
            let parameters =
                r#"[{"name":"payload","kind":"object","required":true,"description":"Nested."}]"#;
            let mut value = descriptor("nested-argument", "Nested", "fake-config");
            value["parameters"] = serde_json::from_str(parameters).unwrap();
            value["schema_sha256"] = json!(sha256(parameters.as_bytes()));
            value
        });
        operator.write(
            "no-runtime",
            descriptor("no-runtime", "NoRuntime", "nobody-registered-this"),
        );
        operator.write("process-runtime", {
            let mut value = descriptor("process-runtime", "ProcessRuntime", "fake-config");
            value["runtime"]["kind"] = json!("process");
            value
        });
        operator.write("os-required", {
            let mut value = descriptor("os-required", "OsRequired", "fake-config");
            value["requires"] = json!(["os_sandbox"]);
            value
        });
        operator.write(
            "failing-read",
            descriptor("failing-read", "FailingRead", "fake-fail"),
        );
        operator
    }

    fn dependency_pin(&self) -> Value {
        json!([{"path": self.dependency, "sha256": sha256(b"runtime-v1")}])
    }

    fn write(&self, id: &str, value: Value) {
        std::fs::write(
            self.root.join(format!("profile-tools/{id}.json")),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }

    fn descriptor_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("profile-tools/{id}.json"))
    }

    fn install(&self) {
        set_env(REGISTRY_ENV, Some(self.root.to_str().unwrap()));
    }
}

// ---------------------------------------------------------------------------
// Running a session.
// ---------------------------------------------------------------------------

fn config(root: &Path, profile: &str) -> Config {
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

const QUESTION: &str = "Report what the configuration says.";

fn run(root: &Path, profile: &str, client: &mut ScriptedClient) -> Result<String, String> {
    run_requiring(root, profile, client, &[])
}

fn run_requiring(
    root: &Path,
    profile: &str,
    client: &mut ScriptedClient,
    required: &[String],
) -> Result<String, String> {
    run_session_with_outcome_with_ui(
        client,
        &mut SessionSnapshot::new(),
        QUESTION,
        required,
        &config(root, profile),
        &NOOP_UI,
    )
    .map(|outcome| outcome.final_text)
    .map_err(|error| format!("{error:#}"))
}

fn events(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn events_named<'a>(events: &'a [Value], name: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|event| event["event"] == name)
        .collect()
}

fn workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("settings.json"), "{}").unwrap();
    (dir, root)
}

fn read_settings(tool: &str) -> (&str, Value) {
    (tool, json!({"path": "settings.json"}))
}

// ---------------------------------------------------------------------------
// Acceptance 1: selection.
// ---------------------------------------------------------------------------

#[test]
fn only_the_selected_profile_tools_are_sent_and_both_profiles_share_one_implementation() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let config_reader = install_reader("fake-config", Ok("{\"mode\":\"strict\"}"), None);
    let index_reader = install_reader("fake-index", Ok("intro: 1\nusage: 2"), None);

    let (_a_dir, a_root) = workspace();
    let mut a = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadConfigObject")]),
        final_text("The mode is strict."),
    ]);
    let a_outcome = run(&a_root, "tool-config-a", &mut a).unwrap();
    assert_eq!(a_outcome, "The mode is strict.");
    assert_eq!(a.tools_of(0), ["Read", "ReadConfigObject"]);
    assert_eq!(config_reader.calls.load(Ordering::SeqCst), 1);
    assert_eq!(index_reader.calls.load(Ordering::SeqCst), 0);

    let (_b_dir, b_root) = workspace();
    let mut b = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadDocIndex")]),
        final_text("The index has two entries."),
    ]);
    run(&b_root, "tool-index-b", &mut b).unwrap();
    assert_eq!(b.tools_of(0), ["Read", "Grep", "ReadDocIndex"]);
    assert_eq!(index_reader.calls.load(Ordering::SeqCst), 1);
    assert_eq!(config_reader.calls.load(Ordering::SeqCst), 1);

    // The same transport event from the same Rust path, for both synthetic
    // profiles; no branch tells them apart.
    for root in [&a_root, &b_root] {
        let recorded = events(root);
        let results = events_named(&recorded, "extension_tool_result");
        assert_eq!(results.len(), 1, "{recorded:?}");
        assert_eq!(results[0]["transport"], "profile_tool");
        assert_eq!(results[0]["ok"], true);
    }
    // The other profile's tool is not even known to this session.
    let (_c_dir, c_root) = workspace();
    let mut crossed = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadDocIndex")]),
        final_text("done"),
    ]);
    run(&c_root, "tool-config-a", &mut crossed).unwrap();
    assert_eq!(index_reader.calls.load(Ordering::SeqCst), 1);
    clear_selection();
}

#[test]
fn off_and_unselected_sources_publish_the_default_tools_only() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    for profile in ["generic", OFF_DRAFT, PACK_HOST] {
        // The tool pack exists under the extension root but is not selected.
        let (_dir, root) = workspace();
        let mut client = ScriptedClient::new(vec![final_text("nothing to do")]);
        run(&root, profile, &mut client).unwrap();
        assert_eq!(client.tools_of(0), DEFAULT_TOOLS, "{profile}");
    }
    clear_selection();
}

#[test]
fn a_selected_pack_contributes_its_tools_to_its_own_profile_only() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let reader = install_reader("fake-index", Ok("index"), None);
    select_pack(TOOL_PACK);

    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadDocIndex")]),
        final_text("done"),
    ]);
    run(&root, PACK_HOST, &mut client).unwrap();
    assert_eq!(client.tools_of(0), ["Read", "Glob", "ReadDocIndex"]);
    assert_eq!(reader.calls.load(Ordering::SeqCst), 1);

    // A selected pack that targets another profile cannot lend it tools.
    let (_other_dir, other_root) = workspace();
    let mut other = ScriptedClient::new(vec![final_text("never")]);
    let error = run(&other_root, "tool-config-a", &mut other).unwrap_err();
    assert!(error.contains("selected_pack_invalid"), "{error}");
    assert_eq!(other.calls(), 0);
    clear_selection();
}

#[test]
fn profile_and_pack_keep_only_the_builtins_both_kept_and_cannot_repeat_a_tool() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-config", Ok("{}"), None);
    select_pack("tool-conflict-pack");
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![final_text("never")]);
    let error = run(&root, "tool-pack-conflict", &mut client).unwrap_err();
    assert!(error.contains("conflicting_declaration"), "{error}");
    assert_eq!(client.calls(), 0);
    clear_selection();

    // The profile keeps Write and Read, the pack keeps Read and Glob: only the
    // builtin both kept is published, and neither source adds one back.
    operator.install();
    let reader = install_reader("fake-index", Ok("index"), None);
    select_pack("tool-narrow-pack");
    let (_narrow_dir, narrow_root) = workspace();
    let mut narrow = ScriptedClient::new(vec![
        reply(vec![
            ("Write", json!({"path": "written.txt", "content": "no"})),
            read_settings("ReadDocIndex"),
        ]),
        final_text("done"),
    ]);
    run(&narrow_root, "tool-keeps-write", &mut narrow).unwrap();
    assert_eq!(narrow.tools_of(0), ["Read", "ReadDocIndex"]);
    assert!(!narrow_root.join("written.txt").exists());
    assert_eq!(reader.calls.load(Ordering::SeqCst), 1);
    clear_selection();
}

// ---------------------------------------------------------------------------
// Acceptance 2: refusals before any provider call.
// ---------------------------------------------------------------------------

#[test]
fn a_declaring_pack_selection_must_verify_and_be_unambiguous() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let refused = |expected: &str| {
        let (_dir, root) = workspace();
        let mut client = ScriptedClient::new(vec![final_text("never")]);
        let error = run(&root, PACK_HOST, &mut client).unwrap_err();
        assert!(error.contains(expected), "{error}");
        assert_eq!(client.calls(), 0);
    };

    // The same checks the pack runtime makes: hash, identity, and pin.
    let directory = select_pack(TOOL_PACK);
    set_env(PACK_ENVS[3], Some("sha256:wrong"));
    refused("identity/hash mismatch");
    let directory_hash = commandagent::planner::pack::load_directory(&directory)
        .unwrap()
        .hash;
    set_env(PACK_ENVS[3], Some(&directory_hash));
    set_env(PACK_ENVS[2], Some("9.9.9"));
    refused("identity/hash mismatch");
    set_env(PACK_ENVS[2], Some("1.0.0"));
    let pin = directory.join("pack.sha256");
    let original = std::fs::read(&pin).unwrap();
    std::fs::write(&pin, "sha256:other\n").unwrap();
    refused("does not match pack.sha256");
    std::fs::write(&pin, original).unwrap();
    clear_selection();

    // Another copy of the same identity that declares something else.
    let ambiguous = extension().root.join("packs/tool-ambiguous-pack/1.0.0");
    let declared =
        |tool: &str| pack_eval("tool-ambiguous-pack", PACK_HOST, &pack_tools("Read", tool));
    std::fs::create_dir_all(&ambiguous).unwrap();
    let selected = declared("doc-index-read");
    std::fs::write(ambiguous.join("eval.yaml"), &selected).unwrap();
    let hash = exact_byte_hash(None, Some(selected.as_bytes()));
    std::fs::write(ambiguous.join("pack.sha256"), format!("{hash}\n")).unwrap();
    commandagent::planner::pack::parse_bytes(None, Some(declared("config-json-read").as_bytes()))
        .unwrap();
    set_env(PACK_ENVS[0], Some(ambiguous.to_str().unwrap()));
    set_env(PACK_ENVS[1], Some("tool-ambiguous-pack"));
    set_env(PACK_ENVS[2], Some("1.0.0"));
    set_env(PACK_ENVS[3], Some(&hash));
    operator.install();
    refused("conflicting_declaration");
    clear_selection();
}

fn cases() -> Value {
    serde_json::from_str(CASES).unwrap()
}

fn assert_refused_before_provider(root: &Path, profile: &str, kind: &str, id: &str) {
    let mut client = ScriptedClient::new(vec![final_text("never")]);
    let error = run(root, profile, &mut client).unwrap_err();
    assert!(
        error.contains(&format!("profile_tool_refused: {kind}")),
        "{id}: {error}"
    );
    assert_eq!(client.calls(), 0, "{id}: minimal loop reached the provider");

    let mut planner = ScriptedClient::new(vec![final_text("never")]);
    let error = generate_step_plan(&mut planner, QUESTION, &config(root, profile))
        .map(|_| ())
        .unwrap_err();
    assert!(
        format!("{error:#}").contains(&format!("profile_tool_refused: {kind}")),
        "{id}: {error:#}"
    );
    assert_eq!(planner.calls(), 0, "{id}: planner reached the provider");
}

#[test]
fn every_registered_refusal_happens_before_the_first_provider_call() {
    let _guard = lock();
    clear_selection();
    extension();
    install_reader("fake-config", Ok("{}"), None);
    let cases = cases();
    let rows = cases["refusals"].as_array().unwrap();
    assert!(rows.len() >= 12);
    for row in rows {
        assert_eq!(row["kind"], "synthetic", "{row}");
        let id = row["id"].as_str().unwrap();
        let operator = Operator::new();
        operator.install();
        let (_dir, root) = workspace();
        assert_refused_before_provider(
            &root,
            row["profile"].as_str().unwrap(),
            row["refusal"].as_str().unwrap(),
            id,
        );
    }
    clear_selection();
}

#[test]
fn an_unset_registry_is_refused_only_for_a_declaring_profile() {
    let _guard = lock();
    clear_selection();
    extension();
    let (_dir, root) = workspace();
    assert_refused_before_provider(&root, "tool-config-a", "registry_unavailable", "unset");
    let mut off = ScriptedClient::new(vec![final_text("fine")]);
    assert_eq!(run(&root, OFF_DRAFT, &mut off).unwrap(), "fine");
}

#[cfg(unix)]
#[test]
fn a_descriptor_that_resolves_into_the_workspace_is_refused() {
    let _guard = lock();
    clear_selection();
    extension();
    install_reader("fake-config", Ok("{}"), None);

    // The whole registry inside the workspace, as a model could write it.
    let (_inside_dir, inside) = workspace();
    std::fs::create_dir_all(inside.join("operator/profile-tools")).unwrap();
    std::fs::write(
        inside.join("operator/profile-tools/config-json-read.json"),
        serde_json::to_vec(&descriptor(
            "config-json-read",
            "ReadConfigObject",
            "fake-config",
        ))
        .unwrap(),
    )
    .unwrap();
    set_env(
        REGISTRY_ENV,
        Some(inside.join("operator").to_str().unwrap()),
    );
    assert_refused_before_provider(&inside, "tool-config-a", "workspace_descriptor", "inside");

    // An outside root that is a symlink to a directory in the workspace.
    let (_link_dir, link_workspace) = workspace();
    std::fs::create_dir_all(link_workspace.join("operator/profile-tools")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    let outside_root = std::fs::canonicalize(outside.path()).unwrap();
    std::os::unix::fs::symlink(
        link_workspace.join("operator"),
        outside_root.join("registry"),
    )
    .unwrap();
    set_env(
        REGISTRY_ENV,
        Some(outside_root.join("registry").to_str().unwrap()),
    );
    assert_refused_before_provider(
        &link_workspace,
        "tool-config-a",
        "workspace_descriptor",
        "root symlink",
    );

    // A descriptor file that is a symlink to one the model wrote.
    let operator = Operator::new();
    operator.install();
    let (_file_dir, file_workspace) = workspace();
    let written = file_workspace.join("descriptor.json");
    std::fs::write(
        &written,
        serde_json::to_vec(&descriptor(
            "symlink-descriptor",
            "FromWorkspace",
            "fake-config",
        ))
        .unwrap(),
    )
    .unwrap();
    std::os::unix::fs::symlink(&written, operator.descriptor_path("symlink-descriptor")).unwrap();
    assert_refused_before_provider(
        &file_workspace,
        "tool-symlink-descriptor",
        "workspace_descriptor",
        "descriptor symlink",
    );

    // A dependency pinned inside the workspace, with a correct digest.
    let (_dep_dir, dep_workspace) = workspace();
    let dependency = dep_workspace.join("runtime.bin");
    std::fs::write(&dependency, "same").unwrap();
    let mut value = descriptor("workspace-dependency", "WorkspaceDep", "fake-config");
    value["dependencies"] = json!([{"path": dependency, "sha256": sha256(b"same")}]);
    operator.write("workspace-dependency", value);
    assert_refused_before_provider(
        &dep_workspace,
        "tool-workspace-dependency",
        "workspace_descriptor",
        "dependency",
    );
    clear_selection();
}

#[test]
fn a_declaration_cannot_self_approve_or_decode_in_an_older_schema() {
    let _guard = lock();
    extension();
    let validate = |source: String| {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("profiles/tool-validate");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("manifest.toml");
        std::fs::write(&path, source).unwrap();
        commandagent::planner::profile_manifest::commands::validate_file(&path)
            .map_err(|error| format!("{error:#}"))
    };
    let declared =
        |entry: &str| manifest("tool-validate", &tool_declaration(r#""Read""#, &[entry]));
    validate(declared(&read_entry("some-tool"))).unwrap();
    validate(manifest("tool-validate", "")).unwrap();
    for smuggled in [
        r#"executable = "/bin/sh""#,
        r#"argv = ["-c"]"#,
        r#"module_url = "file:///tool""#,
        r#"sha256 = "00""#,
        r#"permissions = ["write"]"#,
    ] {
        let error = validate(declared(&format!(
            r#"{}, {smuggled}"#,
            read_entry("some-tool")
        )))
        .unwrap_err();
        assert!(error.contains("unknown field"), "{smuggled}: {error}");
    }
    let wrong_version = manifest(
        "tool-validate",
        "[profile_tools]\nschema_version = \"profile-tool.declaration/v9\"\nbuiltin_tools = [\"Read\"]\n",
    );
    assert!(
        validate(wrong_version)
            .unwrap_err()
            .contains("schema_version")
    );
    let repeated = manifest(
        "tool-validate",
        &tool_declaration(
            r#""Read""#,
            &[&read_entry("some-tool"), &read_entry("some-tool")],
        ),
    );
    assert!(validate(repeated).unwrap_err().contains("declared twice"));
    let unknown_builtin = manifest("tool-validate", &tool_declaration(r#""Sudo""#, &[]));
    assert!(
        validate(unknown_builtin)
            .unwrap_err()
            .contains("unknown tool")
    );
    let empty = manifest("tool-validate", &tool_declaration("", &[]));
    assert!(validate(empty).unwrap_err().contains("must keep"));
    let v1 = format!(
        "{}\n",
        manifest(
            "tool-validate",
            &tool_declaration(r#""Read""#, &[&read_entry("some-tool")])
        )
        .replace("schema_version = \"v2\"", "schema_version = \"v1\""),
    );
    assert!(validate(v1).is_err());

    // The same closed declaration in a pack.
    let smuggling_pack = pack_eval(
        "tool-validate-pack",
        OFF_DRAFT,
        &format!(
            "{}      executable: /bin/sh\n",
            pack_tools("Read", "some-tool")
        ),
    );
    let error = commandagent::planner::pack::parse_bytes(None, Some(smuggling_pack.as_bytes()))
        .map(|_| ())
        .unwrap_err();
    assert!(error.to_string().contains("executable"), "{error}");

    // An assist pack holds prompt material, not tool references.
    let assist_declaring = pack_assist(
        "tool-validate-pack",
        OFF_DRAFT,
        &pack_tools("Read", "some-tool"),
    );
    let error = commandagent::planner::pack::parse_bytes(Some(assist_declaring.as_bytes()), None)
        .map(|_| ())
        .unwrap_err();
    assert!(error.to_string().contains("unknown field"), "{error}");
}

// ---------------------------------------------------------------------------
// Runtime allowlist: a name the model returns directly.
// ---------------------------------------------------------------------------

#[test]
fn a_removed_builtin_returned_by_name_executes_nothing() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-config", Ok("{}"), None);
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![
            ("Write", json!({"path": "written.txt", "content": "no"})),
            (
                "Edit",
                json!({"path": "settings.json", "old_string": "{}", "new_string": "[]"}),
            ),
            ("Bash", json!({"command": "touch bash-ran"})),
            (
                "Bash",
                json!({"command": "touch split-one && touch split-two"}),
            ),
            ("Grep", json!({"pattern": "mode"})),
            ("Glob", json!({"pattern": "*"})),
        ]),
        final_text("done"),
    ]);
    run(&root, "tool-config-a", &mut client).unwrap();

    assert_eq!(client.tools_of(0), ["Read", "ReadConfigObject"]);
    for absent in ["written.txt", "bash-ran", "split-one", "split-two"] {
        assert!(!root.join(absent).exists(), "{absent} was created");
    }
    assert_eq!(
        std::fs::read_to_string(root.join("settings.json")).unwrap(),
        "{}"
    );
    let recorded = events(&root);
    assert!(
        events_named(&recorded, "tool_execute")
            .iter()
            .all(|event| event["status"] != "ok"),
        "{recorded:?}"
    );
    let requests = client.requests.lock().unwrap();
    let feedback = requests[1]["messages"].as_str().unwrap();
    assert!(feedback.contains("tool allowlist"), "{feedback}");
    clear_selection();
}

#[test]
fn a_declaration_cannot_widen_the_existing_tool_rules() {
    let _guard = lock();
    clear_selection();
    extension();
    // The profile keeps Write; a protected path and the workspace rules still
    // apply, and the declared Write works only where it already would.
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![
            (
                "Write",
                json!({"path": ".anvil/escalated.txt", "content": "no"}),
            ),
            ("Write", json!({"path": "allowed.txt", "content": "ok"})),
            ("Bash", json!({"command": "touch bash-ran"})),
        ]),
        final_text("done"),
    ]);
    run(&root, "tool-keeps-write", &mut client).unwrap();
    assert_eq!(client.tools_of(0), ["Read", "Write"]);
    assert!(!root.join(".anvil/escalated.txt").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("allowed.txt")).unwrap(),
        "ok"
    );
    assert!(!root.join("bash-ran").exists());

    // Leaving the workspace stays the fatal error it already is.
    let holder = tempfile::tempdir().unwrap();
    let nested = std::fs::canonicalize(holder.path()).unwrap().join("ws");
    std::fs::create_dir(&nested).unwrap();
    let mut escaping = ScriptedClient::new(vec![
        reply(vec![(
            "Write",
            json!({"path": "../outside.txt", "content": "no"}),
        )]),
        final_text("never"),
    ]);
    let error = run(&nested, "tool-keeps-write", &mut escaping).unwrap_err();
    assert!(error.contains("path may not contain .."), "{error}");
    assert!(!holder.path().join("outside.txt").exists());
    clear_selection();
}

/// The verify step's chained Bash runs through its own split path, not through
/// the registry call, so the allowlist has to hold there too.
#[test]
fn a_removed_bash_on_the_split_verify_path_executes_nothing() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-config", Ok("{}"), None);
    let run_verify_step = |profile: &str| -> (Vec<Value>, String) {
        let (_dir, root) = workspace();
        let plan = StepPlan {
            goal: "Check the settings".to_string(),
            steps: vec![PlanStep {
                id: "step-1".to_string(),
                kind: "verify".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Check the settings".to_string(),
                expected_paths: Vec::new(),
                // Failing first keeps the host's verify-first check from
                // finishing the step before the model runs.
                verify: vec!["test -f absent.txt".to_string()],
            }],
        };
        std::fs::write(root.join("plan.yaml"), render_step_plan(&plan)).unwrap();
        let mut client = ScriptedClient::new(vec![
            reply(vec![(
                "Bash",
                json!({"command": "test -f settings.json && test -d ."}),
            )]),
            final_text("checked"),
            final_text("checked"),
            final_text("checked"),
        ]);
        let outcome = run_plan_file(
            &mut client,
            &root.join("plan.yaml"),
            &config(&root, profile),
        );
        let requests = client.requests.lock().unwrap();
        assert!(
            requests.len() >= 2,
            "{outcome:?} {} {:?}",
            requests.len(),
            events(&root)
        );
        let second = requests[1]["messages"].as_str().unwrap().to_string();
        (events(&root), second)
    };

    // Control: without a declaration this command takes the split path.
    let (off_events, off_feedback) = run_verify_step("generic");
    assert!(off_feedback.contains("segment 1"), "{off_feedback}");
    assert!(
        events_named(&off_events, "tool_execute")
            .iter()
            .any(|event| event["name"] == "Bash" && event["status"] == "ok"),
        "{off_events:?}"
    );

    let (on_events, on_feedback) = run_verify_step("tool-config-a");
    assert!(!on_feedback.contains("segment 1"), "{on_feedback}");
    assert!(on_feedback.contains("tool allowlist"), "{on_feedback}");
    assert!(
        events_named(&on_events, "tool_execute")
            .iter()
            .all(|event| event["name"] != "Bash" || event["status"] != "ok"),
        "{on_events:?}"
    );
    clear_selection();
}

// ---------------------------------------------------------------------------
// Pins compared again right before an execution.
// ---------------------------------------------------------------------------

#[test]
fn a_dependency_swapped_after_admission_stops_the_call_before_it_executes() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let reader = install_reader("fake-config", Ok("{}"), None);
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadConfigObject")]),
        final_text("never"),
    ]);
    let dependency = operator.dependency.clone();
    client.before_reply = Some(Arc::new(move |_| {
        std::fs::write(&dependency, "runtime-v2").unwrap();
    }));
    let error = run(&root, "tool-config-a", &mut client).unwrap_err();
    assert!(error.contains("does not match its pin"), "{error}");
    assert_eq!(reader.calls.load(Ordering::SeqCst), 0);
    assert_eq!(client.calls(), 1);
    let recorded = events(&root);
    assert_eq!(
        events_named(&recorded, "extension_tool_result")[0]["ok"],
        false
    );
    clear_selection();
}

#[test]
fn a_descriptor_swapped_after_admission_stops_the_call_before_it_executes() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let reader = install_reader("fake-config", Ok("{}"), None);
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadConfigObject")]),
        final_text("never"),
    ]);
    let path = operator.descriptor_path("config-json-read");
    client.before_reply = Some(Arc::new(move |_| {
        let mut value = descriptor("config-json-read", "ReadConfigObject", "fake-config");
        value["version"] = json!("2.0.0");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    }));
    let error = run(&root, "tool-config-a", &mut client).unwrap_err();
    assert!(error.contains("changed since admission"), "{error}");
    assert_eq!(reader.calls.load(Ordering::SeqCst), 0);
    clear_selection();
}

// ---------------------------------------------------------------------------
// Acceptance 3: success and failure stay distinct; no false pass.
// ---------------------------------------------------------------------------

#[test]
fn a_failing_read_is_not_a_success_and_stops_with_its_reason() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-fail", Err("fixture file is unreadable"), None);
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("FailingRead")]),
        final_text("PASS"),
    ]);
    let error = run(&root, "tool-failing", &mut client).unwrap_err();
    assert!(error.contains("fixture file is unreadable"), "{error}");
    let recorded = events(&root);
    let result = events_named(&recorded, "extension_tool_result");
    assert_eq!(result[0]["ok"], false);
    let execute = events_named(&recorded, "tool_execute");
    assert_eq!(execute[0]["status"], "error");
    clear_selection();
}

#[test]
fn a_registered_tool_saying_pass_does_not_satisfy_a_required_artifact() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-config", Ok("PASS exit 0: artifact accepted"), None);
    let (_dir, root) = workspace();
    let required = vec!["report.json".to_string()];
    let mut replies = vec![reply(vec![read_settings("ReadConfigObject")])];
    replies.extend((0..40).map(|_| final_text("PASS: the artifact is complete.")));
    let mut client = ScriptedClient::new(replies);
    let result = run_requiring(&root, "tool-config-a", &mut client, &required);
    assert!(!root.join("report.json").exists());
    let error = result.expect_err("a run missing its artifact must not succeed");
    assert!(error.contains("artifact recovery exhausted"), "{error}");
    let recorded = events(&root);
    assert_eq!(
        events_named(&recorded, "extension_tool_result")[0]["ok"],
        true,
        "the tool itself did succeed: {recorded:?}"
    );
    clear_selection();
}

#[test]
fn a_narrowed_output_limit_is_enforced_and_recorded_as_a_failure() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let reader = install_reader("fake-config", Ok("more than four bytes"), None);
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadConfigObject")]),
        final_text("never"),
    ]);
    let error = run(&root, "tool-narrow-output", &mut client).unwrap_err();
    assert!(error.contains("exceeds the 4 byte limit"), "{error}");
    assert_eq!(reader.calls.load(Ordering::SeqCst), 1);
    clear_selection();
}

// ---------------------------------------------------------------------------
// Acceptance 4: unavailable features and unknown usage.
// ---------------------------------------------------------------------------

#[test]
fn a_required_os_feature_gates_admission_on_the_run_sandbox_state() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-config", Ok("{}"), None);
    let (_dir, root) = workspace();
    reset_for_test();
    assert_refused_before_provider(&root, "tool-os-required", "unavailable", "no sandbox");

    let tmp = tempfile::tempdir().unwrap();
    let policy = SandboxPolicy::new(&root, tmp.path());
    install_for_test(
        SandboxMode::Auto,
        policy.clone(),
        SandboxAvailability::Unavailable("backend missing".to_string()),
    );
    assert_refused_before_provider(&root, "tool-os-required", "unavailable", "backend missing");

    install_for_test(SandboxMode::Auto, policy, SandboxAvailability::Available);
    let admitted = admit_selected(&root, "tool-os-required", None)
        .map_err(|error| error.to_string())
        .unwrap()
        .expect("a sandbox in force satisfies the requirement");
    assert_eq!(admitted.bindings().len(), 1);
    reset_for_test();
    clear_selection();
}

#[test]
fn unknown_usage_stays_missing_and_is_never_a_free_success() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let (_dir, root) = workspace();
    let context = commandagent::tools::registry::ToolContext {
        root: root.clone(),
        mode: commandagent::mode::ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: false,
        workspace_policy: commandagent::tools::workspace_policy::WorkspacePolicy::NormalTask,
        eval_events_path: None,
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    };
    let receipts_for = |usage: Option<ToolUsage>| -> Vec<ToolExecutionReceipt> {
        install_reader("fake-config", Ok("{}"), usage);
        let admitted = admit_selected(&root, "tool-config-a", None)
            .map_err(|error| error.to_string())
            .unwrap()
            .unwrap();
        admitted
            .registry()
            .unwrap()
            .execute(
                "ReadConfigObject",
                &json!({"path": "settings.json"}),
                &context,
            )
            .unwrap();
        admitted.receipts()
    };
    let unknown = receipts_for(None);
    assert_eq!(
        unknown,
        [ToolExecutionReceipt {
            tool_id: "config-json-read".to_string(),
            executed: true,
            ok: true,
            usage: None,
        }]
    );
    let partial = ToolUsage {
        input_tokens: Some(3),
        output_tokens: None,
        cost_usd_micros: None,
    };
    assert_eq!(receipts_for(Some(partial))[0].usage, Some(partial));

    // A provider that reports no usage stays unreported through the loop.
    let (_loop_dir, loop_root) = workspace();
    install_reader("fake-config", Ok("{}"), None);
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadConfigObject")]),
        final_text("done"),
    ]);
    run(&loop_root, "tool-config-a", &mut client).unwrap();
    for event in events(&loop_root) {
        for (key, value) in event.as_object().unwrap() {
            if key.contains("cost") {
                assert_ne!(value, &json!(0), "{event}");
                assert_ne!(value, &json!(0.0), "{event}");
            }
        }
    }
    clear_selection();
}

#[test]
fn admitted_bindings_carry_the_identity_pins_effect_and_limits_in_a_stable_order() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-config", Ok("{}"), None);
    let (_dir, root) = workspace();
    let admitted = admit_selected(&root, "tool-collision-pair-free", Some("run-1".to_string()))
        .map_err(|error| error.to_string())
        .unwrap();
    assert!(admitted.is_none(), "an unknown profile declares nothing");

    let admitted = admit_selected(&root, "tool-narrow-output", Some("run-1".to_string()))
        .map_err(|error| error.to_string())
        .unwrap()
        .unwrap();
    let [binding] = admitted.bindings() else {
        panic!("one binding expected");
    };
    assert_eq!(binding.run_id(), Some("run-1"));
    assert_eq!(binding.profile(), "tool-narrow-output");
    assert_eq!(binding.tool_id(), "config-json-read");
    assert_eq!(binding.name(), "ReadConfigObject");
    assert_eq!(binding.version(), "1.0.0");
    assert_eq!(binding.schema_sha256(), sha256(PATH_PARAMETERS.as_bytes()));
    let bytes = std::fs::read(operator.descriptor_path("config-json-read")).unwrap();
    assert_eq!(binding.descriptor_sha256(), sha256(&bytes));
    assert_eq!(binding.effect(), "read");
    assert_eq!(binding.max_input_bytes(), 1024);
    assert_eq!(binding.max_output_bytes(), 4);

    // The profile stays a draft: registering a tool lifts nothing.
    let extension_profile =
        commandagent::planner::extension_profiles::find("tool-narrow-output").unwrap();
    assert_eq!(extension_profile.assurance_ceiling(), "static");
    clear_selection();
}

// ---------------------------------------------------------------------------
// Acceptance 5: off is byte-identical to the base recording.
// ---------------------------------------------------------------------------

/// The exact keys whose numbers vary by clock or temp-root length. Inside
/// strings only the root, generated ids, and elapsed values are replaced.
const NORMALIZED_NUMBER_FIELDS: [&str; 5] = [
    "occurred_at_epoch_ms",
    "duration_ms",
    "elapsed_ms",
    "estimated_prompt_tokens_sent",
    "estimated_stable_prefix_chars",
];

fn normalize_text(root: &Path, text: &str) -> String {
    let mut text = text.replace(&root.display().to_string(), "<ROOT>");
    for (pattern, replacement) in [
        (r"elapsed_ms: [0-9]+", "elapsed_ms: <N>"),
        (r"total_duration_ms=[0-9]+", "total_duration_ms=<N>"),
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

fn normalize_event(root: &Path, value: &Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| {
                    let value =
                        if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str()) && value.is_number() {
                            json!("<N>")
                        } else {
                            normalize_event(root, value)
                        };
                    (key.clone(), value)
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| normalize_event(root, item))
                .collect(),
        ),
        Value::String(text) => Value::String(normalize_text(root, text)),
        other => other.clone(),
    }
}

fn off_script() -> Vec<AssistantReply> {
    vec![
        reply(vec![(
            "Write",
            json!({"path": "notes.txt", "content": "v1\n"}),
        )]),
        reply(vec![("Read", json!({"path": "notes.txt"}))]),
        reply(vec![("Bash", json!({"command": "test -f notes.txt"}))]),
        final_text("notes.txt says v1."),
    ]
}

/// One off control, in the shape of the base recording: outcome, every request
/// with its messages and tool schema, every event, and the artifacts.
fn record_off_row(row: &str, profile: &str, native: bool, planner: bool) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let mut client = ScriptedClient::new(if planner {
        (0..12).map(|_| final_text("this is not a plan")).collect()
    } else {
        off_script()
    });
    client.native = native;
    let config = config(&root, profile);
    let outcome = if planner {
        match generate_step_plan(&mut client, "Write notes.txt.", &config) {
            Ok(plan) => json!({"steps": plan.steps.len()}),
            Err(error) => json!({"error": normalize_text(&root, &format!("{error:#}"))}),
        }
    } else {
        match run_session_with_outcome_with_ui(
            &mut client,
            &mut SessionSnapshot::new(),
            "Write notes.txt and check it.",
            &[],
            &config,
            &NOOP_UI,
        ) {
            Ok(outcome) => json!({
                "stop_reason": format!("{:?}", outcome.stop_reason),
                "final_text": outcome.final_text,
                "iterations": outcome.iterations,
            }),
            Err(error) => json!({"error": normalize_text(&root, &format!("{error:#}"))}),
        }
    };
    let requests = client
        .requests
        .lock()
        .unwrap()
        .iter()
        .map(|request| {
            json!({
                "messages": normalize_text(&root, request["messages"].as_str().unwrap()),
                "tools": request["tools"],
                "native": request["native"],
            })
        })
        .collect::<Vec<_>>();
    let recorded = events(&root)
        .iter()
        .map(|event| serde_json::to_string(&normalize_event(&root, event)).unwrap())
        .collect::<Vec<_>>();
    let mut artifacts = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter(|entry| entry.file_name() != "events.jsonl")
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().to_string(),
                std::fs::read_to_string(entry.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    artifacts.sort();
    json!({
        "id": row,
        "outcome": outcome,
        "requests": requests,
        "events": recorded,
        "artifacts": artifacts,
        "unused_replies": client.replies.lock().unwrap().len(),
    })
}

const OFF_ROWS: [(&str, &str, bool, bool); 5] = [
    ("generic-native", "generic", true, false),
    ("generic-text-protocol", "generic", false, false),
    ("undeclared-draft", OFF_DRAFT, true, false),
    ("undeclared-draft-selected-pack", OFF_DRAFT, true, false),
    ("planner-generic", "generic", true, true),
];

fn golden_row(id: &str) -> Value {
    let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
    assert_eq!(golden["kind"], "synthetic");
    assert_eq!(golden["source"], "104a166b");
    golden["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("no golden row {id}"))
        .clone()
}

fn assert_off_row(row: (&str, &str, bool, bool)) {
    let (id, profile, native, planner) = row;
    let observed = record_off_row(id, profile, native, planner);
    let expected = golden_row(id);
    assert_eq!(
        serde_json::to_string_pretty(&observed).unwrap(),
        serde_json::to_string_pretty(&expected).unwrap(),
        "{id}"
    );
}

#[test]
fn off_requests_events_and_artifacts_match_the_base_recording_byte_for_byte() {
    let _guard = lock();
    clear_selection();
    extension();
    for row in OFF_ROWS {
        clear_selection();
        if row.0 == "undeclared-draft-selected-pack" {
            select_pack(OFF_PACK);
        }
        assert_off_row(row);
    }
    clear_selection();
}

#[test]
fn off_reads_no_registry_and_no_new_file() {
    let _guard = lock();
    clear_selection();
    extension();
    // A registry variable that names nothing is not even resolved.
    set_env(REGISTRY_ENV, Some("/nonexistent/profile-tool-registry"));
    for row in OFF_ROWS
        .iter()
        .filter(|row| row.0 != "undeclared-draft-selected-pack")
    {
        assert_off_row(*row);
    }
    clear_selection();

    // A selected pack without a declaration was decoded when it was selected.
    // Admission keeps no second read of it: a copy gone since then changes
    // nothing in the session, because nothing reopens it.
    let holder = tempfile::tempdir().unwrap();
    let original = select_pack(OFF_PACK);
    let moved = holder.path().join("pack");
    std::fs::rename(&original, &moved).unwrap();
    let observed = record_off_row("undeclared-draft-selected-pack", OFF_DRAFT, true, false);
    std::fs::rename(&moved, &original).unwrap();
    assert_eq!(
        serde_json::to_string_pretty(&observed).unwrap(),
        serde_json::to_string_pretty(&golden_row("undeclared-draft-selected-pack")).unwrap()
    );
    clear_selection();
}

// ---------------------------------------------------------------------------
// The corpus lists what this binary runs.
// ---------------------------------------------------------------------------

#[test]
fn the_corpus_rows_name_declared_profiles_and_cover_every_arm() {
    let cases = cases();
    let declared = declaring_profiles()
        .into_iter()
        .map(|(profile, _)| profile)
        .collect::<Vec<_>>();
    for row in cases["refusals"].as_array().unwrap() {
        let profile = row["profile"].as_str().unwrap();
        assert!(declared.contains(&profile), "{profile}");
    }
    let refusals = cases["refusals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["refusal"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    for kind in [
        "unknown_tool_id",
        "name_collision",
        "invalid_pin",
        "unavailable",
        "conflicting_declaration",
    ] {
        assert!(refusals.contains(kind), "{kind}");
    }
    let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
    let rows = golden["rows"].as_array().unwrap();
    assert_eq!(rows.len(), OFF_ROWS.len());
}
