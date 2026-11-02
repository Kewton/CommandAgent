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
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

const BAD_PACK: &str = "tool-undecodable-pack";

/// A declaration that does not decode: it smuggles an executable.
fn bad_pack_eval() -> String {
    pack_eval(
        BAD_PACK,
        PACK_HOST,
        &format!(
            "{}      executable: /bin/sh\n",
            pack_tools("Read", "doc-index-read")
        ),
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
        write_pack(&root, BAD_PACK, &bad_pack_eval());
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
}

// ---------------------------------------------------------------------------
// A pack's declaration comes from the selected, verified bytes.
// ---------------------------------------------------------------------------

const BOUND_PACK: &str = "tool-bound-pack";

fn bound_undeclared() -> String {
    pack_assist(BOUND_PACK, PACK_HOST, OFF_LITERAL)
}

fn bound_declared(tool: &str) -> String {
    pack_eval(BOUND_PACK, PACK_HOST, &pack_tools("Read", tool))
}

/// One copy of the same id and version, in its own directory, selected by nothing
/// yet. Returns the directory and its exact-byte hash.
fn bound_copy(label: &str, document: &str) -> (PathBuf, String) {
    let holder = extension().root.join(format!("bound-{label}"));
    write_pack(&holder, BOUND_PACK, document);
    let directory = holder.join(format!("packs/{BOUND_PACK}/1.0.0"));
    let hash = commandagent::planner::pack::load_directory(&directory)
        .unwrap()
        .hash;
    (directory, hash)
}

fn select_bound(copy: &(PathBuf, String)) {
    set_env(PACK_ENVS[0], Some(copy.0.to_str().unwrap()));
    set_env(PACK_ENVS[1], Some(BOUND_PACK));
    set_env(PACK_ENVS[2], Some("1.0.0"));
    set_env(PACK_ENVS[3], Some(&copy.1));
}

/// The selected declaration's allowlist holds: the published tools are its own, and
/// a removed Write or Bash executes nothing.
fn assert_allowlist_holds(label: &str) {
    let (_dir, root) = workspace();
    let mut client = ScriptedClient::new(vec![
        reply(vec![
            ("Write", json!({"path": "written.txt", "content": "no"})),
            ("Bash", json!({"command": "touch bash-ran"})),
        ]),
        final_text("done"),
    ]);
    run(&root, PACK_HOST, &mut client).unwrap();
    assert_eq!(client.tools_of(0), ["Read", "ReadDocIndex"], "{label}");
    for absent in ["written.txt", "bash-ran"] {
        assert!(!root.join(absent).exists(), "{label}: {absent} was created");
    }
}

fn assert_off(label: &str) {
    let (_dir, root) = workspace();
    assert!(
        admit_selected(&root, PACK_HOST, None)
            .map_err(|error| format!("{label}: {error}"))
            .unwrap()
            .is_none(),
        "{label}"
    );
}

#[test]
fn an_undeclared_copy_decoded_first_does_not_turn_the_selected_allowlist_off() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-index", Ok("{}"), None);
    let undeclared = bound_copy("first-undeclared", &bound_undeclared());
    let declared = bound_copy("then-declared", &bound_declared("doc-index-read"));
    assert_ne!(undeclared.1, declared.1);
    // Decoded, and even selected and verified, under the same id and version.
    commandagent::planner::pack::parse_bytes(Some(bound_undeclared().as_bytes()), None).unwrap();
    select_bound(&undeclared);
    assert_off("the undeclared copy itself");
    select_bound(&declared);
    assert_allowlist_holds("after an undeclared copy");
    clear_selection();
}

#[test]
fn a_declared_copy_decoded_first_does_not_refuse_a_selected_copy() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-index", Ok("{}"), None);
    let declared = bound_copy("first-declared", &bound_declared("doc-index-read"));
    // Another declaration of the same identity, decoded and never selected.
    commandagent::planner::pack::parse_bytes(
        None,
        Some(bound_declared("config-json-read").as_bytes()),
    )
    .unwrap();
    select_bound(&declared);
    assert_allowlist_holds("the declared copy");
    let undeclared = bound_copy("then-undeclared", &bound_undeclared());
    select_bound(&undeclared);
    assert_off("the undeclared copy");
    select_bound(&declared);
    assert_allowlist_holds("the declared copy again");
    clear_selection();
}

#[test]
fn a_failed_decode_does_not_affect_a_later_selection() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    install_reader("fake-index", Ok("{}"), None);
    let undecodable = pack_eval(
        BOUND_PACK,
        PACK_HOST,
        &format!(
            "{}      executable: /bin/sh\n",
            pack_tools("Read", "doc-index-read")
        ),
    );
    assert!(commandagent::planner::pack::parse_bytes(None, Some(undecodable.as_bytes())).is_err());
    let undeclared = bound_copy("after-failure-undeclared", &bound_undeclared());
    select_bound(&undeclared);
    assert_off("an undeclared copy after a failed decode");
    let declared = bound_copy("after-failure-declared", &bound_declared("doc-index-read"));
    select_bound(&declared);
    assert_allowlist_holds("a declared copy after a failed decode");
    clear_selection();
}

/// A selection set directly in the environment has no decoded value to use, so each
/// admission reads it. Once its bytes fail, the next admission refuses; nothing
/// remembered from an earlier admission turns that into off.
#[test]
fn a_directly_set_selection_is_read_by_each_admission_and_refused_once_it_fails() {
    let _guard = lock();
    clear_selection();
    extension();
    let operator = Operator::new();
    operator.install();
    let (_dir, root) = workspace();
    let each = bound_copy(
        "read-each",
        &pack_assist(BOUND_PACK, PACK_HOST, &format!("{OFF_LITERAL}# each\n")),
    );
    select_bound(&each);
    assert_off("the first admission");
    assert_off("the second admission");
    std::fs::write(each.0.join("assist.yaml"), "not: [a pack").unwrap();
    let error = admit_selected(&root, PACK_HOST, None)
        .map(|_| ())
        .unwrap_err()
        .to_string();
    assert!(error.contains("selected_pack_invalid"), "{error}");

    // Never admitted: its bytes are read, do not decode, and are refused here. The
    // binary's own gate is a test below: there a pack gate refuses first.
    let unread = bound_copy(
        "unread",
        &pack_assist(BOUND_PACK, PACK_HOST, &format!("{OFF_LITERAL}# unread\n")),
    );
    select_bound(&unread);
    std::fs::write(unread.0.join("assist.yaml"), "not: [a pack").unwrap();
    let error = admit_selected(&root, PACK_HOST, None)
        .map(|_| ())
        .unwrap_err()
        .to_string();
    assert!(error.contains("selected_pack_invalid"), "{error}");

    // A declared pack whose bytes change does not lose its allowlist.
    let declared = bound_copy("changes", &bound_declared("doc-index-read"));
    select_bound(&declared);
    install_reader("fake-index", Ok("{}"), None);
    assert_allowlist_holds("before the change");
    std::fs::write(declared.0.join("eval.yaml"), bound_undeclared()).unwrap();
    let error = admit_selected(&root, PACK_HOST, None)
        .map(|_| ())
        .unwrap_err()
        .to_string();
    assert!(error.contains("selected_pack_invalid"), "{error}");
    clear_selection();
}

#[test]
fn the_profile_tool_transport_cannot_be_registered_as_a_secret() {
    use commandagent::sensitive_data::{
        RegistrationRefusal, ReservedKind, SecretCatalog, collides_with_fixed_identifier,
        is_fixed_identifier,
    };
    assert!(is_fixed_identifier("profile_tool"));
    assert!(is_fixed_identifier("selected_pack_invalid"));
    assert!(is_fixed_identifier("profile_tool_refused"));
    assert!(is_fixed_identifier("unknown_tool_id"));
    assert!(is_fixed_identifier("workspace_descriptor"));
    for value in [
        "profile_tool",
        "file_tool",
        "selected_pack_invalid",
        "pack_invalid",
        "profile_tool_refused",
        "tool_refused",
        "unknown_tool_id",
        "workspace_descriptor",
    ] {
        assert!(collides_with_fixed_identifier(value), "{value}");
        let mut catalog = SecretCatalog::new();
        let refusal = catalog
            .register_credential("DUMMY_972_TOKEN", value)
            .unwrap_err();
        assert!(
            matches!(
                refusal,
                RegistrationRefusal::Reserved {
                    kind: ReservedKind::FixedIdentifier,
                    ..
                }
            ),
            "{value}: {refusal:?}"
        );
        assert!(catalog.is_empty(), "{value}");
    }
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

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// What a swap replaces, and with what. The bytes stay the same in every case, so
/// a comparison of the bytes alone would pass each one.
#[derive(Clone, Copy)]
enum Swap {
    RegistryToSymlinkIntoWorkspace,
    RegistryToSameBytesDirectory,
    RootToSameBytesDirectory,
    RootToSymlinkToTheOriginal,
    DependencyDirectoryToSymlinkIntoWorkspace,
    DependencyDirectoryToSymlinkOutside,
    DependencyDirectoryToSameBytesDirectory,
}

impl Swap {
    fn apply(self, operator_root: &Path, workspace: &Path, holding: &Path) {
        let registry = operator_root.join("profile-tools");
        let dependencies = operator_root.join("deps");
        let moved = holding.join("moved");
        match self {
            Self::RegistryToSymlinkIntoWorkspace => {
                copy_tree(&registry, &workspace.join("shadow"));
                std::fs::rename(&registry, &moved).unwrap();
                std::os::unix::fs::symlink(workspace.join("shadow"), &registry).unwrap();
            }
            Self::RegistryToSameBytesDirectory => {
                std::fs::rename(&registry, &moved).unwrap();
                copy_tree(&moved, &registry);
            }
            Self::RootToSameBytesDirectory => {
                std::fs::rename(operator_root, &moved).unwrap();
                copy_tree(&moved, operator_root);
            }
            Self::RootToSymlinkToTheOriginal => {
                std::fs::rename(operator_root, &moved).unwrap();
                std::os::unix::fs::symlink(&moved, operator_root).unwrap();
            }
            Self::DependencyDirectoryToSymlinkIntoWorkspace => {
                copy_tree(&dependencies, &workspace.join("shadow"));
                std::fs::rename(&dependencies, &moved).unwrap();
                std::os::unix::fs::symlink(workspace.join("shadow"), &dependencies).unwrap();
            }
            Self::DependencyDirectoryToSymlinkOutside => {
                copy_tree(&dependencies, &holding.join("copy"));
                std::fs::rename(&dependencies, &moved).unwrap();
                std::os::unix::fs::symlink(holding.join("copy"), &dependencies).unwrap();
            }
            Self::DependencyDirectoryToSameBytesDirectory => {
                std::fs::rename(&dependencies, &moved).unwrap();
                copy_tree(&moved, &dependencies);
            }
        }
    }
}

/// A swap between admission and the call stops the call. The control is the same
/// run with no swap: the tool executes through the same bindings.
fn run_with_swap(swap: Option<Swap>) -> (Result<String, String>, usize) {
    let operator = Operator::new();
    operator.install();
    let dependency_directory = operator.root.join("deps");
    std::fs::create_dir_all(&dependency_directory).unwrap();
    let dependency = dependency_directory.join("runtime.bin");
    std::fs::write(&dependency, "runtime-v1").unwrap();
    operator.write("config-json-read", {
        let mut value = descriptor("config-json-read", "ReadConfigObject", "fake-config");
        value["dependencies"] = json!([{"path": dependency, "sha256": sha256(b"runtime-v1")}]);
        value
    });
    let reader = install_reader("fake-config", Ok("{}"), None);
    let (_dir, root) = workspace();
    let holding = tempfile::tempdir().unwrap();
    let holding_path = std::fs::canonicalize(holding.path()).unwrap();
    let mut client = ScriptedClient::new(vec![
        reply(vec![read_settings("ReadConfigObject")]),
        final_text("done"),
    ]);
    let (operator_root, workspace_root) = (operator.root.clone(), root.clone());
    let done = AtomicBool::new(false);
    client.before_reply = Some(Arc::new(move |_| {
        if let Some(swap) = swap
            && !done.swap(true, Ordering::SeqCst)
        {
            swap.apply(&operator_root, &workspace_root, &holding_path);
        }
    }));
    let result = run(&root, "tool-config-a", &mut client);
    (result, reader.calls.load(Ordering::SeqCst))
}

#[test]
fn a_directory_swapped_after_admission_stops_the_call_with_the_same_bytes() {
    let _guard = lock();
    clear_selection();
    extension();
    let (control, executions) = run_with_swap(None);
    assert!(control.is_ok(), "{control:?}");
    assert_eq!(executions, 1, "the control did not execute the tool");
    const MOVED: &str = "no longer reached from the directories it was admitted from";
    for (swap, label, expected) in [
        (
            Swap::RegistryToSymlinkIntoWorkspace,
            "registry to a symlink into the workspace",
            MOVED,
        ),
        (
            Swap::RegistryToSameBytesDirectory,
            "registry to another directory",
            MOVED,
        ),
        (
            Swap::RootToSameBytesDirectory,
            "root to another directory",
            MOVED,
        ),
        (
            Swap::RootToSymlinkToTheOriginal,
            "root to a symlink to the original",
            MOVED,
        ),
        (
            Swap::DependencyDirectoryToSymlinkIntoWorkspace,
            "dependency directory to a symlink into the workspace",
            "must lie outside the workspace",
        ),
        (
            Swap::DependencyDirectoryToSymlinkOutside,
            "dependency directory to a symlink outside",
            "must be canonical",
        ),
        (
            Swap::DependencyDirectoryToSameBytesDirectory,
            "dependency directory to another directory",
            "dependency directory changed since admission",
        ),
    ] {
        let (result, executions) = run_with_swap(Some(swap));
        let error = result.expect_err(label);
        assert!(error.contains(expected), "{label}: {error}");
        assert_eq!(executions, 0, "{label}: the executor ran");
    }
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

    // The sandbox wraps spawned commands. An in-process executor spawns none, so
    // a sandbox in force is not isolation for it, and nothing runs.
    install_for_test(SandboxMode::Auto, policy, SandboxAvailability::Available);
    assert!(commandagent::tools::os_sandbox::is_applied());
    let reader = install_reader("fake-config", Ok("{}"), None);
    assert_refused_before_provider(&root, "tool-os-required", "unavailable", "sandbox applied");
    let error = admit_selected(&root, "tool-os-required", None)
        .map(|_| ())
        .unwrap_err()
        .to_string();
    assert!(error.contains("in_process"), "{error}");
    assert_eq!(reader.calls.load(Ordering::SeqCst), 0);
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
    for row in OFF_ROWS {
        clear_selection();
        if row.0 == "undeclared-draft-selected-pack" {
            select_pack(OFF_PACK);
        }
        assert_off_row(row);
    }
    clear_selection();
}

/// The library path: the environment names the pack and nothing decoded it, so
/// admission reads it. The read changes no byte of the run. That it is a read shows
/// when the pack is gone. A CLI selection reads nothing here; see the binary tests.
#[test]
fn a_directly_set_undeclared_pack_is_read_and_changes_no_byte_of_the_run() {
    let _guard = lock();
    clear_selection();
    extension();
    select_pack(OFF_PACK);
    let row = OFF_ROWS
        .into_iter()
        .find(|row| row.0 == "undeclared-draft-selected-pack")
        .unwrap();
    assert_off_row(row);
    assert_off_row(row);

    let copy = bound_copy(
        "direct-read",
        &pack_assist(BOUND_PACK, PACK_HOST, &format!("{OFF_LITERAL}# direct\n")),
    );
    select_bound(&copy);
    let (_dir, root) = workspace();
    let holder = tempfile::tempdir().unwrap();
    std::fs::rename(&copy.0, holder.path().join("pack")).unwrap();
    let error = admit_selected(&root, PACK_HOST, None)
        .map(|_| ())
        .unwrap_err()
        .to_string();
    assert!(error.contains("selected_pack_invalid"), "{error}");
    clear_selection();
}

// ---------------------------------------------------------------------------
// The binary: pre-run refusal before everything else.
// ---------------------------------------------------------------------------

/// A loopback endpoint that counts connections and closes each one.
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

struct CliRun {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    provider_calls: usize,
    jev_calls: usize,
    evidence: String,
}

/// All text below `root`: run evidence, wherever the run wrote it.
fn collect_text(root: &Path) -> String {
    let mut text = String::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if let Ok(content) = std::fs::read_to_string(&path) {
                text.push_str(&content);
                text.push('\n');
            }
        }
    }
    text
}

/// Runs the binary with `--request-pattern jev`, a dummy key, and every outbound
/// HTTPS request sent to a counting fake proxy, so a jev call reaches only that
/// fake. The provider is a counting fake as well.
fn cli_run(
    workspace: &Path,
    registry: &Path,
    arguments: &[&str],
    environment: &[(&str, &str)],
) -> CliRun {
    let provider = CountingEndpoint::start();
    let jev = CountingEndpoint::start();
    let home = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .arg("--extension-root")
        .arg(&extension().root)
        .args(["--ollama-host", &provider.url, "--model", "m"])
        .args(["--request-pattern", "jev", "--chat-retries", "0"])
        .args(arguments)
        .current_dir(workspace)
        .env_clear()
        .env("HOME", home.path())
        .env(REGISTRY_ENV, registry)
        .env("JEV_API_KEY", "dummy-972-not-a-real-credential")
        .env("HTTPS_PROXY", &jev.url);
    for (name, value) in environment {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    CliRun {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        provider_calls: provider.finish(),
        jev_calls: jev.finish(),
        evidence: collect_text(workspace),
    }
}

fn assert_pre_run_refusal(run: &CliRun, id: &str) {
    assert_eq!(run.code, Some(2), "{id}: {}", run.stderr);
    assert!(
        run.stderr.starts_with("error: profile_tool_refused: "),
        "{id}: {}",
        run.stderr
    );
    assert_eq!(
        run.stderr.trim_end().lines().count(),
        1,
        "{id}: {}",
        run.stderr
    );
    assert_eq!(run.stdout, "", "{id}");
    assert_eq!(run.provider_calls, 0, "{id}: provider contacted");
    assert_eq!(run.jev_calls, 0, "{id}: jev contacted");
    for marker in [
        "run_start",
        "request_pattern_resolved",
        "bash_sandbox_status",
    ] {
        assert!(!run.evidence.contains(marker), "{id}: {marker} recorded");
    }
}

#[test]
fn every_refusal_exits_2_through_the_binary_before_jev_the_provider_and_run_start() {
    let cases = cases();
    for row in cases["refusals"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        let operator = Operator::new();
        let (_dir, root) = workspace();
        let run = cli_run(
            &root,
            &operator.root,
            &[
                "--summary-json",
                "--profile",
                row["profile"].as_str().unwrap(),
                "--prompt",
                "hi",
            ],
            &[],
        );
        assert_pre_run_refusal(&run, id);
    }
}

#[test]
fn a_registry_inside_the_workspace_is_refused_by_the_binary_too() {
    let (_dir, root) = workspace();
    std::fs::create_dir_all(root.join("operator/profile-tools")).unwrap();
    let run = cli_run(
        &root,
        &root.join("operator"),
        &["--profile", "tool-config-a", "--prompt", "hi"],
        &[],
    );
    assert_pre_run_refusal(&run, "registry-in-workspace");
    assert!(
        run.stderr.contains("workspace_descriptor"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_selected_or_inherited_declaring_pack_is_refused_by_the_binary() {
    let operator = Operator::new();
    let (_dir, root) = workspace();
    let selected = cli_run(
        &root,
        &operator.root,
        &[
            "--profile",
            PACK_HOST,
            "--pack",
            &format!("{TOOL_PACK}@1.0.0"),
            "--prompt",
            "hi",
        ],
        &[],
    );
    assert_pre_run_refusal(&selected, "selected-pack");

    let directory = extension().root.join(format!("packs/{TOOL_PACK}/1.0.0"));
    let hash = commandagent::planner::pack::load_directory(&directory)
        .unwrap()
        .hash;
    let (_inherited_dir, inherited_root) = workspace();
    let inherited = cli_run(
        &inherited_root,
        &operator.root,
        &["--profile", PACK_HOST, "--prompt", "hi"],
        &[
            (PACK_ENVS[0], directory.to_str().unwrap()),
            (PACK_ENVS[1], TOOL_PACK),
            (PACK_ENVS[2], "1.0.0"),
            (PACK_ENVS[3], &hash),
        ],
    );
    assert_pre_run_refusal(&inherited, "inherited-pack");
}

/// Starting the interactive shell with a declaring profile takes the same gate.
#[test]
fn starting_the_repl_with_a_refused_profile_exits_2_before_it_opens() {
    let operator = Operator::new();
    let (_dir, root) = workspace();
    let run = cli_run(
        &root,
        &operator.root,
        &["--profile", "tool-unknown-id"],
        &[],
    );
    assert_pre_run_refusal(&run, "repl-start");
}

/// The fakes count real traffic: the same command without a refusal reaches both,
/// and records what a refusal must not.
#[test]
fn the_counting_fakes_see_a_run_that_is_not_refused() {
    let operator = Operator::new();
    for profile in [OFF_DRAFT, "tool-keeps-write"] {
        let (_dir, root) = workspace();
        let run = cli_run(
            &root,
            &operator.root,
            &["--profile", profile, "--prompt", "hi"],
            &[],
        );
        assert!(run.provider_calls > 0, "{profile}: {}", run.stderr);
        assert_eq!(run.jev_calls, 1, "{profile}: {}", run.stderr);
        assert!(run.evidence.contains("run_start"), "{profile}");
        assert!(
            run.evidence.contains("request_pattern_resolved"),
            "{profile}"
        );
    }
}

/// The same exit, stream shape, and evidence as another pre-run rejection that
/// reaches `cli_argument_error` or the pack error at its existing place.
#[test]
fn a_refusal_has_the_form_of_the_existing_pre_run_rejections() {
    let operator = Operator::new();
    let (_dir, root) = workspace();
    let existing = cli_run(
        &root,
        &operator.root,
        &["--summary-json", "--pack-hash", "x", "--prompt", "hi"],
        &[],
    );
    assert_eq!(existing.code, Some(2), "{}", existing.stderr);
    assert!(
        existing.stderr.starts_with("error: "),
        "{}",
        existing.stderr
    );
    assert_eq!(existing.stdout, "");
    assert_eq!(existing.provider_calls, 0);
    assert!(!existing.evidence.contains("run_start"));

    let (_refused_dir, refused_root) = workspace();
    let refused = cli_run(
        &refused_root,
        &operator.root,
        &[
            "--summary-json",
            "--profile",
            "tool-unknown-id",
            "--prompt",
            "hi",
        ],
        &[],
    );
    assert_pre_run_refusal(&refused, "unknown-id");
}

/// A pack whose declaration does not decode is not known to opt in, so the existing
/// pack gates refuse it, selected or inherited, still before any call or run_start.
#[test]
fn an_undecodable_declaring_pack_is_refused_by_the_existing_pack_gates() {
    let operator = Operator::new();
    let (_dir, root) = workspace();
    let selected = cli_run(
        &root,
        &operator.root,
        &[
            "--profile",
            PACK_HOST,
            "--pack",
            &format!("{BAD_PACK}@1.0.0"),
            "--prompt",
            "hi",
        ],
        &[],
    );
    let directory = extension().root.join(format!("packs/{BAD_PACK}/1.0.0"));
    let eval = bad_pack_eval();
    let hash = exact_byte_hash(None, Some(eval.as_bytes()));
    let (_inherited_dir, inherited_root) = workspace();
    let inherited = cli_run(
        &inherited_root,
        &operator.root,
        &["--profile", PACK_HOST, "--prompt", "hi"],
        &[
            (PACK_ENVS[0], directory.to_str().unwrap()),
            (PACK_ENVS[1], BAD_PACK),
            (PACK_ENVS[2], "1.0.0"),
            (PACK_ENVS[3], &hash),
        ],
    );
    for (label, run) in [("selected", &selected), ("inherited", &inherited)] {
        assert_eq!(run.code, Some(2), "{label}: {}", run.stderr);
        assert!(run.stderr.contains("executable"), "{label}: {}", run.stderr);
        assert!(
            !run.stderr.contains("profile_tool_refused"),
            "{label}: {}",
            run.stderr
        );
        assert_eq!(run.provider_calls, 0, "{label}");
        assert_eq!(run.jev_calls, 0, "{label}");
        assert!(!run.evidence.contains("run_start"), "{label}");
        assert!(
            !run.evidence.contains("request_pattern_resolved"),
            "{label}"
        );
    }
}

/// Without a declaration a pack that cannot resolve keeps refusing at its
/// existing place, after the run's pre-run setup, with its own message.
#[test]
fn an_off_run_keeps_the_existing_pack_rejection_message() {
    let operator = Operator::new();
    let (_dir, root) = workspace();
    let run = cli_run(
        &root,
        &operator.root,
        &["--pack", "no-such-pack@1.0.0", "--prompt", "hi"],
        &[],
    );
    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        !run.stderr.contains("profile_tool_refused"),
        "{}",
        run.stderr
    );
    assert_eq!(run.provider_calls, 0);
}

/// A provider that answers, and records what it was asked.
struct AnsweringProvider {
    url: String,
    bodies: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl AnsweringProvider {
    fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (bodies, stop) = (bodies.clone(), stop.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::SeqCst) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        std::thread::sleep(std::time::Duration::from_millis(2));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    Self::answer(&mut stream, &bodies);
                }
            })
        };
        Self {
            url,
            bodies,
            stop,
            thread: Some(thread),
        }
    }

    fn answer(stream: &mut std::net::TcpStream, bodies: &Mutex<Vec<String>>) {
        use std::io::{Read, Write};
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 4096];
        let (head, length) = loop {
            let Ok(count) = stream.read(&mut chunk) else {
                return;
            };
            if count == 0 {
                return;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&bytes[..end]).to_string();
                let length = head
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                bytes.drain(..end + 4);
                break (head, length);
            }
        };
        while bytes.len() < length {
            let Ok(count) = stream.read(&mut chunk) else {
                return;
            };
            if count == 0 {
                return;
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        let response = if head.starts_with("POST /api/chat ") {
            bodies
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&bytes).to_string());
            json!({
                "model": "m",
                "message": {"role": "assistant", "content": "Done."},
                "done": true,
            })
            .to_string()
        } else if head.starts_with("GET /api/tags ") {
            json!({"models": [{"name": "m"}]}).to_string()
        } else {
            json!({"capabilities": ["completion", "tools"]}).to_string()
        };
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        );
        let _ = stream.flush();
    }

    fn finish(mut self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.bodies.lock().unwrap().clone()
    }
}

/// A fake proxy for the jev call. It acts while the run waits for the answer: after
/// the pack gates and before the session's admission.
fn jev_proxy_acting(act: Box<dyn FnOnce() + Send>) -> (String, std::thread::JoinHandle<usize>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let thread = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            if let Ok(connection) = listener.accept() {
                act();
                drop(connection);
                return 1;
            }
            if std::time::Instant::now() > deadline {
                return 0;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    });
    (url, thread)
}

#[derive(Debug, PartialEq, Eq)]
struct OffRun {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    requests: Vec<String>,
    events: Vec<String>,
}

/// One run with a provider that answers. Everything that differs by construction
/// between two runs is normalized: the private directories, ports, and durations.
fn off_cli_run(
    pack: &MovablePack,
    arguments: &[&str],
    environment: &[(&str, &str)],
    settings: Option<&str>,
    act: Box<dyn FnOnce() + Send>,
) -> OffRun {
    let (_dir, root) = workspace();
    if let Some(settings) = settings {
        let config = root.join(".commandagent/config.toml");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, settings).unwrap();
    }
    let provider = AnsweringProvider::start();
    let (proxy, jev) = jev_proxy_acting(act);
    let home = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command
        .arg("--extension-root")
        .arg(&pack.root)
        .args(["--ollama-host", &provider.url, "--model", "m"])
        .args(["--request-pattern", "jev", "--chat-retries", "0"])
        .args(arguments)
        .current_dir(&root)
        .env_clear()
        .env("HOME", home.path())
        .env("JEV_API_KEY", "dummy-972-not-a-real-credential")
        .env("HTTPS_PROXY", &proxy);
    for (name, value) in environment {
        command.env(name, value);
    }
    let output = command.output().unwrap();
    let provider_url = provider.url.clone();
    let requests = provider.finish();
    assert_eq!(
        jev.join().unwrap(),
        1,
        "the jev call did not happen: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let durations =
        regex::Regex::new(r#"("[a-z_]+_(?:ms|secs|seconds)":)[0-9.]+|\([0-9]+s\)"#).unwrap();
    let stable = |text: &str| {
        let text = normalize_text(&root, text)
            .replace(&home.path().display().to_string(), "<HOME>")
            .replace(&provider_url, "<PROVIDER>")
            .replace(&proxy, "<PROXY>");
        durations.replace_all(&text, "${1}<N>").into_owned()
    };
    let mut events = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if entry.file_name() == "events.jsonl" {
                for line in std::fs::read_to_string(&path).unwrap().lines() {
                    let event = normalize_event(&root, &serde_json::from_str(line).unwrap());
                    // Recorded only when the run lasted a millisecond or more.
                    if event["event"] != "time_profile" {
                        events.push(stable(&event.to_string()));
                    }
                }
            }
        }
    }
    OffRun {
        code: output.status.code(),
        stdout: stable(&String::from_utf8_lossy(&output.stdout)),
        stderr: stable(&String::from_utf8_lossy(&output.stderr)),
        requests: requests.iter().map(|body| stable(body)).collect(),
        events,
    }
}

const MOVABLE_PACK: &str = "data-assist@1.0.0";

/// This repository's undeclared `data-assist` pack, in an extension root private to
/// one test, so that test may move it.
struct MovablePack {
    _dir: tempfile::TempDir,
    root: PathBuf,
    directory: PathBuf,
    moved: PathBuf,
    hash: String,
}

impl MovablePack {
    fn new() -> Self {
        fn copy(from: &Path, to: &Path) {
            std::fs::create_dir_all(to).unwrap();
            for entry in std::fs::read_dir(from).unwrap().flatten() {
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    std::fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let directory = root.join("packs/data-assist/1.0.0");
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("packs/data-assist/1.0.0"),
            &directory,
        );
        let hash = commandagent::planner::pack::load_directory(&directory)
            .unwrap()
            .hash;
        Self {
            _dir: dir,
            moved: root.join("moved"),
            root,
            directory,
            hash,
        }
    }

    fn moving(&self) -> Box<dyn FnOnce() + Send> {
        let (from, to) = (self.directory.clone(), self.moved.clone());
        Box::new(move || std::fs::rename(from, to).unwrap())
    }

    fn put_back(&self) {
        std::fs::rename(&self.moved, &self.directory).unwrap();
    }
}

fn nothing() -> Box<dyn FnOnce() + Send> {
    Box::new(|| {})
}

fn assert_off_run_answered(label: &str, run: &OffRun) {
    assert_eq!(run.code, Some(0), "{label}: {}", run.stderr);
    assert!(!run.requests.is_empty(), "{label}: no provider request");
    assert!(
        run.events.iter().any(|event| event.contains("run_start")),
        "{label}: {:?}",
        run.events
    );
}

/// `--pack` and a preset's pack are decoded once, by the CLI. The pack can be gone
/// when the session is admitted, and no byte of the run changes.
#[test]
fn a_cli_selected_off_pack_is_not_read_by_admission_and_no_byte_changes() {
    let pack = MovablePack::new();
    let flag = [
        "--profile",
        "data",
        "--pack",
        MOVABLE_PACK,
        "--prompt",
        "hi",
    ];
    let control = off_cli_run(&pack, &flag, &[], None, nothing());
    assert_off_run_answered("--pack", &control);
    let run = off_cli_run(&pack, &flag, &[], None, pack.moving());
    pack.put_back();
    assert_eq!(run, control);

    let preset = format!("[preset.moved]\npack = \"{MOVABLE_PACK}\"\n");
    let named = ["--profile", "data", "--preset", "moved", "--prompt", "hi"];
    let control = off_cli_run(&pack, &named, &[], Some(&preset), nothing());
    assert_off_run_answered("preset", &control);
    let run = off_cli_run(&pack, &named, &[], Some(&preset), pack.moving());
    pack.put_back();
    assert_eq!(run, control);
}

/// A selection the process inherited was not decoded by the CLI, so admission reads
/// it. The read changes no byte of the run. That it is a read, and that the proxy
/// moves the pack before admission at all, shows when the same move refuses.
#[test]
fn an_inherited_off_pack_is_read_by_admission_and_no_byte_changes() {
    let pack = MovablePack::new();
    let selection = [
        (PACK_ENVS[0], pack.directory.to_str().unwrap()),
        (PACK_ENVS[1], "data-assist"),
        (PACK_ENVS[2], "1.0.0"),
        (PACK_ENVS[3], pack.hash.as_str()),
    ];
    let inherited = ["--profile", "data", "--prompt", "hi"];
    let flag = [
        "--profile",
        "data",
        "--pack",
        MOVABLE_PACK,
        "--prompt",
        "hi",
    ];
    let control = off_cli_run(&pack, &flag, &[], None, nothing());
    assert_off_run_answered("--pack", &control);
    let run = off_cli_run(&pack, &inherited, &selection, None, nothing());
    assert_eq!(run, control);

    let refused = off_cli_run(&pack, &inherited, &selection, None, pack.moving());
    pack.put_back();
    assert_ne!(refused.code, Some(0), "{}", refused.stderr);
    assert!(
        refused.stderr.contains("selected_pack_invalid"),
        "{}",
        refused.stderr
    );
    assert_eq!(refused.requests, Vec::<String>::new());
}

/// A declaring pack whose pin is a symlink: the decode the CLI's resolution shares
/// with admission refuses the member, before everything a refusal precedes.
#[cfg(unix)]
#[test]
fn a_declaring_pack_with_a_symlinked_pin_is_refused_before_any_call() {
    let id = "tool-symlink-pin-pack";
    let root = &extension().root;
    write_pack(
        root,
        id,
        &pack_eval(id, PACK_HOST, &pack_tools("Read", "doc-index-read")),
    );
    let pin = root.join(format!("packs/{id}/1.0.0/pack.sha256"));
    let target = root.join(format!("pin-of-{id}"));
    std::fs::rename(&pin, &target).unwrap();
    std::os::unix::fs::symlink(&target, &pin).unwrap();
    let operator = Operator::new();
    let (_dir, workspace_root) = workspace();
    let selector = format!("{id}@1.0.0");
    let run = cli_run(
        &workspace_root,
        &operator.root,
        &[
            "--profile",
            PACK_HOST,
            "--pack",
            &selector,
            "--prompt",
            "hi",
        ],
        &[],
    );
    assert_eq!(run.code, Some(2), "{}", run.stderr);
    assert!(
        run.stderr.contains("error: load selected pack ")
            && run.stderr.contains("is not a regular file"),
        "{}",
        run.stderr
    );
    assert!(
        !run.stderr.contains("profile_tool_refused"),
        "{}",
        run.stderr
    );
    assert_eq!(run.stdout, "");
    assert_eq!(run.provider_calls, 0);
    assert_eq!(run.jev_calls, 0);
    for marker in [
        "run_start",
        "request_pattern_resolved",
        "bash_sandbox_status",
    ] {
        assert!(!run.evidence.contains(marker), "{marker} recorded");
    }
}

#[test]
fn the_pre_run_refusal_is_wired_before_everything_it_must_precede() {
    let source = include_str!("../src/lib.rs");
    let position = |needle: &str| {
        source
            .find(needle)
            .unwrap_or_else(|| panic!("`{needle}` is not in src/lib.rs"))
    };
    let refusal = position("refuse_before_cli_run(");
    for later in [
        "request_pattern::install(",
        "tools::os_sandbox::install_with(",
        "RuntimeEnvironmentGuard::install(",
        "artifact_contract_checks::preflight(",
        "run_resolved_config_with_summary(config, summary_source",
    ] {
        assert!(refusal < position(later), "{later} runs before the refusal");
    }
    let node_refusal = position("refuse_before_workflow_node(");
    assert!(
        node_refusal < position("artifact_contract_checks::preflight_for_workflow_node("),
        "the workflow child refuses after its artifact preflight"
    );
}

#[test]
fn a_workflow_child_is_refused_before_its_run_start_and_any_provider_call() {
    let root = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(root.path()).unwrap();
    let outer = root.join("outer");
    let origin = root.join("origin");
    std::fs::create_dir_all(&outer).unwrap();
    let run_dir = origin.join(".anvil/runs/origin-run");
    std::fs::create_dir_all(&run_dir).unwrap();
    std::fs::create_dir_all(origin.join(".anvil/plans")).unwrap();
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
    let existing_run_starts = collect_text(&origin).matches("\"run_start\"").count();
    let workflow = root.join("workflow.yaml");
    std::fs::write(
        &workflow,
        "workflow: profile-tool-child\nversion: 0.2\nentry: create\nnodes:\n  create: { intent: create, profile: generic }\n  rebuild: { intent: create, profile: tool-unknown-id }\nroutes:\n  - { from: create, on: failed, when: recovery_yaml_present, to: rebuild, carry: [workspace, recovery_yaml] }\nterminal: {}\n",
    )
    .unwrap();
    let operator = Operator::new();
    let run = cli_run(
        &outer,
        &operator.root,
        &[
            "--workflow",
            workflow.to_str().unwrap(),
            "--origin",
            origin.to_str().unwrap(),
        ],
        &[],
    );
    let evidence = collect_text(&origin);
    assert!(
        evidence.contains("profile_tool_refused: unknown_tool_id"),
        "status={:?}\nstderr={}\nevidence={evidence}",
        run.code,
        run.stderr
    );
    assert!(evidence.contains("node_failed:rebuild"), "{evidence}");
    assert_eq!(run.provider_calls, 0, "{}", run.stderr);
    assert_eq!(run.jev_calls, 0);
    assert_eq!(
        evidence.matches("\"run_start\"").count(),
        existing_run_starts,
        "the refused child recorded a run_start: {evidence}"
    );
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
