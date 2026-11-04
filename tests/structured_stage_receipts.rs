//! Issue #975 (G4): stages declared over propose-artifacts tools, through a real admission.
//!
//! What is decided around a call, on every OS, is in the unit tests of
//! `src/tools/profile_tools/effects.rs` and `src/planner/structured_stages/`. Here is the path
//! from a profile's manifest to an admitted set and, where the strict isolation exists, from a
//! provider's raw reply to a commit. The process is a fixed script for `/bin/bash`, written and
//! pinned here. It takes no shell text from a model and cannot fork, so it uses builtins only.
//! No model, key, external provider, or Jev call is used, and the only provider is a fake on
//! 127.0.0.1.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use commandagent::tools::profile_tools::{admit_selected, directory_pin};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";
const WALK_PROFILE: &str = "stage-walk";
const NO_FLOOR_PROFILE: &str = "stage-no-floor";
const UNKNOWN_TOOL_PROFILE: &str = "stage-unknown-tool";
const NO_ARGUMENT_PROFILE: &str = "stage-no-argument";
#[cfg(target_os = "macos")]
const MARKER_NAME_PROFILE: &str = "stage-marker-name";
#[cfg(target_os = "macos")]
const ENTRY_PROFILE: &str = "stage-entry";
const UPPER_TOOL_ID: &str = "stage-upper-tool";
const LOWER_TOOL_ID: &str = "stage-lower-tool";
const BARE_TOOL_ID: &str = "stage-bare-tool";
const UPPER_TOOL: &str = "ProposeUpper";
const LOWER_TOOL: &str = "ProposeLower";
const UPPER_OUTPUT: &str = "out/upper.json";
const LOWER_OUTPUT: &str = "out/lower.json";
const FIXED_INPUT: &str = "fixed/seed.json";
const CANDIDATE: &[u8] = b"{\"stage\":\"candidate\"}\n";

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn encode_base64(bytes: &[u8]) -> String {
    const SYMBOLS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
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

fn manifest(profile: &str, tools: &[&str], stages: &[(&str, &str)]) -> String {
    let declared = tools
        .iter()
        .map(|id| format!("  {{ id = \"{id}\", effect = \"propose-artifacts\" }},\n"))
        .collect::<String>();
    let staged = stages
        .iter()
        .map(|(stage, tool)| {
            format!(
                "  {{ id = \"{stage}\", targets = [{{ id = \"main\", tool_id = \"{tool}\" }}] }},\n"
            )
        })
        .collect::<String>();
    format!(
        "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Stage {profile}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/v2\"\nbuiltin_tools = [\"Read\"]\ntools = [\n{declared}]\n[profile_tools.stages]\nstages = [\n{staged}]\n"
    )
}

fn extension() {
    // Each time: the off control registers a root of its own, as the base it was recorded on has
    // to be able to read it.
    commandagent::planner::extension_profiles::register(extension_root()).unwrap();
}

fn extension_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        let both = [UPPER_TOOL_ID, LOWER_TOOL_ID];
        let walk = [("upper", UPPER_TOOL_ID), ("lower", LOWER_TOOL_ID)];
        #[cfg(target_os = "macos")]
        {
            // The history replaces a whole token holding a word like this one, so a commit's
            // answer naming the stage below it has to fall to its fixed form.
            let marked = [("upper", UPPER_TOOL_ID), ("api_key_stage", LOWER_TOOL_ID)];
            let directory = root.join(format!("profiles/{MARKER_NAME_PROFILE}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("manifest.toml"),
                manifest(MARKER_NAME_PROFILE, &both, &marked),
            )
            .unwrap();
            // A preset an entry can run as it is: an ultra plan needs two phases.
            let directory = root.join(format!("profiles/{ENTRY_PROFILE}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("manifest.toml"),
                manifest(ENTRY_PROFILE, &both, &walk).replace(
                    "phases = [\n  { id = \"implementation\", prompt = \"Write the notes for {goal}.\" },\n]",
                    "phases = [\n  { id = \"first\", prompt = \"Phase-first: submit the upper stage for {goal}.\" },\n  { id = \"second\", prompt = \"Phase-second: submit the lower stage for {goal}.\" },\n]",
                ),
            )
            .unwrap();
        }
        for (profile, text) in [
            (WALK_PROFILE, manifest(WALK_PROFILE, &both, &walk)),
            (NO_FLOOR_PROFILE, manifest(NO_FLOOR_PROFILE, &both, &walk)),
            (
                UNKNOWN_TOOL_PROFILE,
                manifest(
                    UNKNOWN_TOOL_PROFILE,
                    &[UPPER_TOOL_ID],
                    &[("upper", "stage-never-declared")],
                ),
            ),
            (
                NO_ARGUMENT_PROFILE,
                manifest(
                    NO_ARGUMENT_PROFILE,
                    &[BARE_TOOL_ID],
                    &[("bare", BARE_TOOL_ID)],
                ),
            ),
        ] {
            let directory = root.join(format!("profiles/{profile}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(directory.join("manifest.toml"), text).unwrap();
        }
        commandagent::planner::extension_profiles::register(&root).unwrap();
        root
    })
}

/// Passes unless the arguments ask it not to. Bash has no digest or base64 builtin, so both are
/// computed here and written into the fixed script.
fn tool_script() -> String {
    format!(
        r#"IFS= read -r request
take() {{ local pattern="\"$1\":\"([^\"]+)\""; [[ $request =~ $pattern ]] && printf -v "$2" '%s' "${{BASH_REMATCH[1]}}"; }}
take request_id rid; take tool_id tid; take runtime_sha256 run; take input_revision rev; take schema_sha256 sch
status=passed; said='[]'
if [[ $request == *'"mode":"bad"'* ]]; then status=failed; said='[{{"code":"shape","message":"the shape is off"}}]'; fi
printf '%s' "{{\"protocol\":\"profile-tool.result/v1\",\"request_id\":\"$rid\",\"tool_id\":\"$tid\",\"runtime_sha256\":\"$run\",\"input_revision\":\"$rev\",\"schema_sha256\":\"$sch\",\"checks\":[{{\"id\":\"shape\",\"status\":\"$status\"}}],\"outputs\":[{{\"id\":\"artifact\",\"sha256\":\"{digest}\",\"bytes_base64\":\"{encoded}\"}}],\"diagnostics\":$said}}"
"#,
        digest = sha256(CANDIDATE),
        encoded = encode_base64(CANDIDATE),
    )
}

fn stage_schema(inputs: &[&str]) -> Value {
    json!({
        "protocol": "profile-tool.schema/v1",
        "root": {
            "type": "object",
            "additionalProperties": false,
            "required": ["mode", "expected_revision"],
            "properties": {
                "mode": {"type": "string", "enum": ["good", "bad"]},
                "expected_revision": {"type": "string"},
            },
        },
        "inputs": inputs,
    })
}

struct Env {
    _workspace: tempfile::TempDir,
    _evidence: tempfile::TempDir,
    _operator: tempfile::TempDir,
    root: PathBuf,
    evidence: PathBuf,
    registry: PathBuf,
    tools: PathBuf,
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
        let (registry, tools) = (base.join("registry"), base.join("tools"));
        std::fs::create_dir_all(registry.join("profile-tools")).unwrap();
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::create_dir_all(root.join("fixed")).unwrap();
        std::fs::write(root.join(FIXED_INPUT), b"a fixed seed").unwrap();
        // SAFETY: every env mutation in this binary holds `LOCK`.
        unsafe { std::env::set_var(REGISTRY_ENV, &registry) };
        Self {
            _workspace: workspace,
            _evidence: evidence,
            _operator: operator,
            root,
            evidence: evidence_path,
            registry,
            tools,
        }
    }

    /// A later process over what an earlier one set up. The registry variable is already set.
    #[cfg(target_os = "macos")]
    fn over(root: PathBuf, evidence: PathBuf) -> Self {
        extension();
        let unused = || tempfile::tempdir().unwrap();
        Self {
            _workspace: unused(),
            _evidence: unused(),
            _operator: unused(),
            root,
            evidence,
            registry: PathBuf::new(),
            tools: PathBuf::new(),
        }
    }

    fn write_tool(&self, tool: (&str, &str, &str), schema: &Value, stage: Option<Value>) {
        let (tool_id, name, output) = tool;
        let directory = self.tools.join(tool_id);
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("tool.sh");
        std::fs::write(&script, tool_script()).unwrap();
        let shell = std::fs::canonicalize(if cfg!(target_os = "macos") {
            "/bin/bash"
        } else {
            "/bin/sh"
        })
        .unwrap();
        let mut descriptor = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": tool_id,
            "name": name,
            "version": "1.0.0",
            "description": "Propose one artifact for a synthetic fixture.",
            "effect": "propose-artifacts",
            "parameters": [],
            "schema_sha256": sha256(serde_json::to_string(schema).unwrap().as_bytes()),
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
        if let Some(stage) = stage {
            descriptor["stage"] = stage;
        }
        std::fs::write(
            self.registry.join(format!("profile-tools/{tool_id}.json")),
            descriptor.to_string(),
        )
        .unwrap();
    }

    /// The upper stage's output is an input of the lower one. It is not there yet: admission
    /// must not need it.
    fn write_walk_tools(&self, floor: Option<Value>) {
        self.write_tool(
            (UPPER_TOOL_ID, UPPER_TOOL, UPPER_OUTPUT),
            &stage_schema(&[FIXED_INPUT]),
            floor.clone(),
        );
        self.write_tool(
            (LOWER_TOOL_ID, LOWER_TOOL, LOWER_OUTPUT),
            &stage_schema(&[UPPER_OUTPUT, FIXED_INPUT]),
            floor,
        );
    }

    fn admission_error(&self, profile: &str) -> String {
        admit_selected(&self.root, profile, None)
            .err()
            .map(|refusal| refusal.to_string())
            .unwrap_or_else(|| panic!("{profile} was admitted"))
    }

    fn workspace(&self) -> &Path {
        &self.root
    }
}

fn floor() -> Value {
    json!({
        "required_checks": ["shape"],
        "attempts_per_target": 4,
        "attempts_per_run": 16,
        "deadline_seconds": 600,
    })
}

#[test]
fn a_stage_over_a_tool_nobody_declared_is_refused_at_admission() {
    let _guard = lock();
    let env = Env::new();
    env.write_walk_tools(Some(floor()));
    let error = env.admission_error(UNKNOWN_TOOL_PROFILE);
    assert!(
        error.starts_with("profile_tool_refused: stage_contract_refused:"),
        "{error}"
    );
}

#[test]
fn a_stage_tool_without_its_operators_floor_is_refused_at_admission() {
    let _guard = lock();
    let env = Env::new();
    env.write_walk_tools(None);
    let error = env.admission_error(NO_FLOOR_PROFILE);
    assert!(
        error.starts_with("profile_tool_refused: stage_contract_refused:"),
        "{error}"
    );
    assert!(error.contains("floor"), "{error}");
}

/// The off control: runs with no stage declaration, through the public session and plan entries,
/// against what the base commit's production code did with the same replies. It runs on every OS.
mod off_control {
    use std::collections::{BTreeMap, VecDeque};
    use std::io::{Read as _, Write as _};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    use commandagent::config::{
        Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
        PromptLayout, Provider, ToolProtocol,
    };
    use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
    use commandagent::planner::adjudication::contract::IntentId;
    use commandagent::planner::runner::{run_step_plan, run_ultra_plan};
    use commandagent::planner::step_plan::{PlanStep, StepPlan};
    use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan};
    use commandagent::providers::client_from_config;
    use commandagent::sensitive_data::{SecretCatalog, install_scope};
    use commandagent::state::SessionSnapshot;
    use commandagent::tools::profile_tools::{
        ProfileToolReadExecutor, ReadToolOutput, register_read_executor,
    };
    use commandagent::tui::NOOP_UI;
    use serde_json::{Value, json};

    use super::{REGISTRY_ENV, lock, sha256};

    const OFF_GOLDEN: &str =
        include_str!("corpus/apps/structured-stage-receipts/fixtures/off-golden.json");
    const CASES: &str = include_str!("corpus/apps/structured-stage-receipts/fixtures/cases.json");
    const MANIFEST: &str =
        include_str!("corpus/apps/structured-stage-receipts/source-manifest.json");
    const BASE_SHA: &str = "d2360125210564351f30e42bbc1a9f5e51f2b3cc";
    const DUMMY_SECRET: &str = "fake-dummy-credential-975-not-a-real-key";
    const FLAT_PROFILE: &str = "stage-off-flat";
    const FLAT_TOOL: &str = "stage-off-flat-read";
    const FLAT_RUNTIME: &str = "stage-off-flat-runtime";
    const FLAT_PARAMETERS: &str = r#"[{"name":"path","kind":"workspace_path","required":true,"description":"Workspace-relative file."}]"#;
    /// Next to the secret: the words this feature made fixed, as text a run can carry.
    const RESERVED_WORDS_PROMPT: &str = "Check fake-dummy-credential-975-not-a-real-key next to stage_incomplete structured_stage_incomplete structured_stage_refused structured_stage_unmet.";

    fn set_env(name: &str, value: Option<&str>) {
        // SAFETY: every env mutation in this binary holds `LOCK`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    /// The requests and replies of one thread of a run, in that thread's order.
    #[derive(Default)]
    struct Lane {
        requests: Mutex<Vec<String>>,
        replies: Mutex<VecDeque<String>>,
    }

    struct FakeHttp {
        url: String,
        main: Arc<Lane>,
        plans: Arc<Lane>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    struct Heard {
        requests: Vec<String>,
        plan_requests: Vec<String>,
        unused: usize,
    }

    impl FakeHttp {
        /// `plans`, when there are any, answer the requests that offer no tool: a phase's plan,
        /// which may come from a thread of its own. Which of two threads arrives first is not
        /// something a reply should depend on, or a recording show.
        fn start(replies: Vec<String>, plans: Vec<String>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let split = !plans.is_empty();
            let lane = |replies: Vec<String>| {
                Arc::new(Lane {
                    requests: Mutex::default(),
                    replies: Mutex::new(VecDeque::from(replies)),
                })
            };
            let (main, plans) = (lane(replies), lane(plans));
            let stop = Arc::new(AtomicBool::new(false));
            let thread = {
                let (main, plans, stop) = (main.clone(), plans.clone(), stop.clone());
                std::thread::spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((stream, _)) => serve(stream, &main, split.then_some(&*plans)),
                            Err(_) => std::thread::sleep(std::time::Duration::from_millis(2)),
                        }
                    }
                })
            };
            Self {
                url,
                main,
                plans,
                stop,
                thread: Some(thread),
            }
        }

        fn finish(mut self) -> Heard {
            self.stop.store(true, Ordering::SeqCst);
            self.thread.take().unwrap().join().unwrap();
            Heard {
                requests: self.main.requests.lock().unwrap().clone(),
                plan_requests: self.plans.requests.lock().unwrap().clone(),
                unused: self.main.replies.lock().unwrap().len()
                    + self.plans.replies.lock().unwrap().len(),
            }
        }
    }

    fn offers_no_tool(body: &str) -> bool {
        serde_json::from_str::<Value>(body)
            .ok()
            .is_some_and(|body| body["tools"].as_array().is_none_or(Vec::is_empty))
    }

    fn serve(mut stream: std::net::TcpStream, main: &Lane, plans: Option<&Lane>) {
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
        let lane = plans.filter(|_| offers_no_tool(&body)).unwrap_or(main);
        lane.requests.lock().unwrap().push(body);
        let reply = lane
            .replies
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

    fn tool_reply(name: &str, raw: &str) -> String {
        json!({"choices": [{"message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call-0",
                "type": "function",
                "function": {"name": name, "arguments": raw},
            }],
        }}]})
        .to_string()
    }

    fn write_reply() -> String {
        tool_reply("Write", r#"{"path":"notes.txt","content":"the notes\n"}"#)
    }

    fn step_plan_reply(goal: &str, id: &str) -> String {
        text_reply(
            &json!({"goal": goal, "steps": [{
                "id": id,
                "kind": "implement",
                "expected_result": "pass",
                "instruction": goal,
                "expected_paths": ["notes.txt"],
                "verify": [],
            }]})
            .to_string(),
        )
    }

    struct FlatReader;

    impl std::fmt::Debug for FlatReader {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("FlatReader")
        }
    }

    impl ProfileToolReadExecutor for FlatReader {
        fn read(&self, _root: &Path, arguments: &Value) -> anyhow::Result<ReadToolOutput> {
            Ok(ReadToolOutput {
                text: format!("flat read of {arguments}"),
                usage: None,
            })
        }
    }

    /// A root the base can read too: a tool set with no stage in it.
    fn register_off_extension() {
        static ROOT: OnceLock<PathBuf> = OnceLock::new();
        let root = ROOT.get_or_init(|| {
            let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
            let directory = root.join(format!("profiles/{FLAT_PROFILE}"));
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("manifest.toml"),
                format!(
                    "[metadata]\nid = \"{FLAT_PROFILE}\"\ndisplay_name = \"Stage off\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [\"Read\", \"Write\"]\ntools = [\n  {{ id = \"{FLAT_TOOL}\", effect = \"read\" }},\n]\n"
                ),
            )
            .unwrap();
            register_read_executor(FLAT_RUNTIME, Arc::new(FlatReader));
            root
        });
        commandagent::planner::extension_profiles::register(root).unwrap();
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
                        let value = if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str())
                            && value.is_number()
                        {
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

    #[derive(Clone, Copy)]
    enum Entry {
        Session,
        TextSession,
        StepPlan,
        Ultra,
        /// As an explicit profile runs: the next phase's plan is asked for on another thread.
        UltraSpeculating,
        Investigate,
        Fix,
    }

    struct OffRow {
        id: &'static str,
        profile: &'static str,
        entry: Entry,
        prompt: &'static str,
        replies: fn() -> Vec<String>,
    }

    /// For a row with two threads: what answers the requests that offer no tool.
    fn plans_of(row: &OffRow) -> Vec<String> {
        if matches!(row.entry, Entry::UltraSpeculating) {
            vec![
                step_plan_reply("Write the notes", "step-first"),
                step_plan_reply("Write the notes again", "step-second"),
            ]
        } else {
            Vec::new()
        }
    }

    /// What this feature changed the wiring of: a session, a plain plan, an ultra plan and its
    /// final acceptance, and the two ends that have none.
    const OFF_ROWS: [OffRow; 9] = [
        OffRow {
            id: "chat-native-write-and-read",
            profile: "generic",
            entry: Entry::Session,
            prompt: "Write notes.txt and read it.",
            replies: || {
                vec![
                    write_reply(),
                    tool_reply("Read", r#"{"path":"notes.txt"}"#),
                    text_reply("done"),
                ]
            },
        },
        OffRow {
            id: "text-xml-write",
            profile: "generic",
            entry: Entry::TextSession,
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
            id: "flat-extension-read",
            profile: FLAT_PROFILE,
            entry: Entry::Session,
            prompt: "Read the settings.",
            replies: || {
                vec![
                    tool_reply("ReadFlatConfig", r#"{"path":"settings.json"}"#),
                    text_reply("done"),
                ]
            },
        },
        OffRow {
            id: "dummy-secret-next-to-reserved-words",
            profile: "generic",
            entry: Entry::Session,
            prompt: RESERVED_WORDS_PROMPT,
            replies: || {
                vec![
                    tool_reply(
                        "Write",
                        r#"{"path":"notes.txt","content":"stage_incomplete fake-dummy-credential-975-not-a-real-key structured_stage_refused"}"#,
                    ),
                    text_reply("structured_stage_unmet done"),
                ]
            },
        },
        OffRow {
            id: "step-plan-write",
            profile: "generic",
            entry: Entry::StepPlan,
            prompt: "Write the notes.",
            replies: || vec![write_reply(), text_reply("done")],
        },
        OffRow {
            id: "ultra-plan-two-phases",
            profile: "generic",
            entry: Entry::Ultra,
            prompt: "Write the notes.",
            replies: || {
                vec![
                    step_plan_reply("Write the notes", "step-first"),
                    write_reply(),
                    text_reply("done"),
                    step_plan_reply("Write the notes again", "step-second"),
                    write_reply(),
                    text_reply("done"),
                ]
            },
        },
        OffRow {
            id: "ultra-plan-speculating",
            profile: "generic",
            entry: Entry::UltraSpeculating,
            prompt: "Write the notes.",
            replies: || {
                vec![
                    write_reply(),
                    text_reply("done"),
                    write_reply(),
                    text_reply("done"),
                ]
            },
        },
        OffRow {
            id: "investigate-end",
            profile: "generic",
            entry: Entry::Investigate,
            prompt: "Find out why it fails.",
            replies: || {
                vec![
                    text_reply(
                        &json!({"goal": "reproduce", "steps": [{
                            "id": "step-reproduce",
                            "kind": "verify",
                            "expected_result": "fail",
                            "instruction": "Reproduce the failure",
                            "expected_paths": [],
                            "verify": ["test -f no-such-marker"],
                        }]})
                        .to_string(),
                    ),
                    step_plan_reply("Write the notes", "step-report"),
                    write_reply(),
                    text_reply("done"),
                ]
            },
        },
        OffRow {
            id: "fix-end",
            profile: "generic",
            entry: Entry::Fix,
            prompt: "Fix the missing notes.",
            replies: || {
                vec![
                    text_reply(
                        &json!({"goal": "reproduce", "steps": [{
                            "id": "step-reproduce",
                            "kind": "verify",
                            "expected_result": "fail",
                            "instruction": "Reproduce the failure",
                            "expected_paths": [],
                            "verify": ["test -f notes.txt"],
                        }]})
                        .to_string(),
                    ),
                    step_plan_reply("Write the notes", "step-repair"),
                    write_reply(),
                    text_reply("done"),
                ]
            },
        },
    ];

    fn plan_of(row: &OffRow, intent: &str) -> UltraPlan {
        UltraPlan {
            goal: row.prompt.to_string(),
            profile: row.profile.to_string(),
            style: "default".to_string(),
            intent: intent.to_string(),
            phases: vec![
                UltraPhase {
                    id: "first".to_string(),
                    prompt: "Phase-first: do the first part.".to_string(),
                },
                UltraPhase {
                    id: "second".to_string(),
                    prompt: "Phase-second: do the second part.".to_string(),
                },
            ],
        }
    }

    fn outcome_of(row: &OffRow, config: &Config, paths: &[(&Path, &str)], url: &str) -> Value {
        let mut client = client_from_config(config, false).unwrap();
        let summary = |result: anyhow::Result<String>| match result {
            Ok(summary) => json!({"summary": normalize_text(paths, url, &summary)}),
            Err(error) => json!({"error": normalize_text(paths, url, &format!("{error:#}"))}),
        };
        match row.entry {
            Entry::Session | Entry::TextSession => match run_session_with_outcome_with_ui(
                client.as_mut(),
                &mut SessionSnapshot::new(),
                row.prompt,
                &[],
                config,
                &NOOP_UI,
            ) {
                Ok(outcome) => json!({
                    "stop_reason": format!("{:?}", outcome.stop_reason),
                    "final_text": outcome.final_text,
                    "iterations": outcome.iterations,
                    "tool_calls": outcome.tool_calls,
                    "changed_paths": outcome.changed_paths,
                }),
                Err(error) => json!({"error": normalize_text(paths, url, &format!("{error:#}"))}),
            },
            Entry::StepPlan => {
                let plan = StepPlan {
                    goal: row.prompt.to_string(),
                    steps: vec![PlanStep {
                        id: "step-write".to_string(),
                        kind: "implement".to_string(),
                        expected_result: "pass".to_string(),
                        instruction: "Write notes.txt".to_string(),
                        expected_paths: vec!["notes.txt".to_string()],
                        verify: Vec::new(),
                    }],
                };
                summary(run_step_plan(client.as_mut(), &plan, config))
            }
            Entry::Ultra | Entry::UltraSpeculating | Entry::Investigate | Entry::Fix => {
                let intent = match row.entry {
                    Entry::Investigate => "investigate",
                    Entry::Fix => "fix",
                    _ => "create",
                };
                let mut planner = client_from_config(config, true).unwrap();
                summary(run_ultra_plan(
                    planner.as_mut(),
                    client.as_mut(),
                    &plan_of(row, intent),
                    config,
                ))
            }
        }
    }

    fn deterministic_config(row: &OffRow, config: &mut Config) {
        // An ultra plan on an explicit profile plans its next phase on another thread, whose
        // provider events can land anywhere among this one's. A profile that may yet be promoted
        // plans in line, so this row has one order, compared whole. `UltraSpeculating` is the
        // other way, with `events_match`.
        if matches!(row.entry, Entry::Ultra) {
            config.profile_explicit = false;
        }
    }

    fn record_off_row(row: &OffRow) -> Value {
        let workspace = tempfile::tempdir().unwrap();
        let evidence = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        let evidence = std::fs::canonicalize(evidence.path()).unwrap();
        std::fs::write(root.join("settings.json"), "{\"mode\":\"fast\"}").unwrap();
        let http = FakeHttp::start((row.replies)(), plans_of(row));
        let url = http.url.clone();
        let mut config = base_config(&root, &evidence, row.profile, &url);
        match row.entry {
            Entry::TextSession => config.tool_protocol = Some(ToolProtocol::Text),
            Entry::Investigate => config.intent_override = Some(IntentId::Investigate),
            _ => {}
        }
        deterministic_config(row, &mut config);
        let registry = flat_registry();
        set_env(
            REGISTRY_ENV,
            (row.profile == FLAT_PROFILE).then(|| registry.path().to_str().unwrap()),
        );
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
            .unwrap();
        install_scope(catalog, Some(&root), config.eval_events_path.as_deref());
        let paths = [
            (root.as_path(), "<ROOT>"),
            (evidence.as_path(), "<EVIDENCE>"),
        ];
        let outcome = outcome_of(row, &config, &paths, &url);
        // T6: off makes no new sidecar, and nothing in the state directory is this feature's.
        assert!(
            !evidence.join("state/structured-stages").exists(),
            "{}: a run with no stage left a record",
            row.id
        );
        set_env(REGISTRY_ENV, None);
        let heard = http.finish();
        let normalized = |bodies: &[String]| {
            bodies
                .iter()
                .map(|body| normalize_text(&paths, &url, body))
                .collect::<Vec<_>>()
        };
        let requests = normalized(&heard.requests);
        let plan_requests = normalized(&heard.plan_requests);
        let unused = heard.unused;
        // One value after another, not one per line: two threads can leave two values on a line.
        let events = serde_json::Deserializer::from_str(
            &std::fs::read_to_string(evidence.join("events.jsonl")).unwrap_or_default(),
        )
        .into_iter::<Value>()
        .map(|value| normalize_value(&paths, &url, &value.unwrap()).to_string())
        .collect::<Vec<_>>();
        let mut recorded = json!({
            "id": row.id,
            "outcome": outcome,
            "requests": requests,
            "events": events,
            "artifacts": artifacts(&root)
                .into_iter()
                .map(|(path, text)| (
                    normalize_text(&paths, &url, &path),
                    normalize_text(&paths, &url, &text),
                ))
                .collect::<BTreeMap<_, _>>(),
            "unused_replies": unused,
        });
        // Each thread's requests in that thread's order, and only for a row with two threads.
        if !plan_requests.is_empty() {
            recorded["plan_requests"] = json!(plan_requests);
        }
        recorded
    }

    /// The two events a provider client writes itself. The thread that asks for the next phase's
    /// plan (`pipeline.rs`) has no events path of its own, but the client it clones does, so these
    /// two, and only these, come from that thread and land anywhere among the main thread's.
    const EVENTS_OF_THE_OTHER_THREAD: [&str; 2] = ["provider_request", "provider_response"];
    const OTHER_THREAD_STARTS: &str = "speculative_phase_plan_started";
    const OTHER_THREAD_ENDS: &str = "speculative_phase_plan_adopted";

    fn lines_of(events: &[Value]) -> Vec<&str> {
        events.iter().map(|line| line.as_str().unwrap()).collect()
    }

    fn event_name(line: &str) -> String {
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|value| value["event"].as_str().map(str::to_string))
            .unwrap_or_default()
    }

    fn position_of(events: &[&str], name: &str) -> Result<usize, String> {
        events
            .iter()
            .position(|line| event_name(line) == name)
            .ok_or_else(|| format!("no {name}"))
    }

    /// Byte for byte, in order, but for one thing: between the other thread's start and its
    /// adoption, the events it can write may fall anywhere among the others. There they are
    /// compared as a multiset of whole lines. Nothing is left out, and outside those two lines
    /// nothing is allowed to move.
    fn events_match(new: &[&str], old: &[&str]) -> Result<(), String> {
        let (new_start, new_end) = (
            position_of(new, OTHER_THREAD_STARTS)?,
            position_of(new, OTHER_THREAD_ENDS)?,
        );
        let (old_start, old_end) = (
            position_of(old, OTHER_THREAD_STARTS)?,
            position_of(old, OTHER_THREAD_ENDS)?,
        );
        if new[..new_start] != old[..old_start] {
            return Err("the events before the other thread's start differ".to_string());
        }
        if new[new_end..] != old[old_end..] {
            return Err("the events from the adoption on differ".to_string());
        }
        let parts = |range: &[&str]| {
            let (mut movable, mut fixed): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
            for line in range {
                if EVENTS_OF_THE_OTHER_THREAD.contains(&event_name(line).as_str()) {
                    movable.push((*line).to_string());
                } else {
                    fixed.push((*line).to_string());
                }
            }
            movable.sort();
            (movable, fixed)
        };
        let (new_movable, new_fixed) = parts(&new[new_start..new_end]);
        let (old_movable, old_fixed) = parts(&old[old_start..old_end]);
        if new_fixed != old_fixed {
            return Err("the main thread's events between them differ or moved".to_string());
        }
        if new_movable != old_movable {
            return Err("the provider events between them are not the same lines".to_string());
        }
        Ok(())
    }

    /// What the comparison above lets through, and what it does not.
    #[test]
    fn only_the_other_threads_events_may_move_and_only_within_their_range() {
        let line = |event: &str, note: &str| json!({"event": event, "note": note}).to_string();
        let start = line(OTHER_THREAD_STARTS, "");
        let end = line(OTHER_THREAD_ENDS, "");
        let (before, a, b, after) = (
            line("before", ""),
            line("main_a", ""),
            line("main_b", ""),
            line("after", ""),
        );
        let (request, response) = (
            line("provider_request", "p"),
            line("provider_response", "p"),
        );
        let old = [
            before.as_str(),
            start.as_str(),
            a.as_str(),
            request.as_str(),
            b.as_str(),
            response.as_str(),
            end.as_str(),
            after.as_str(),
        ];
        assert_eq!(events_match(&old, &old), Ok(()));

        // Its own events anywhere between the two lines.
        let moved = [
            before.as_str(),
            start.as_str(),
            request.as_str(),
            response.as_str(),
            a.as_str(),
            b.as_str(),
            end.as_str(),
            after.as_str(),
        ];
        assert_eq!(events_match(&moved, &old), Ok(()));

        // Any other event out of order.
        let swapped = [
            before.as_str(),
            start.as_str(),
            b.as_str(),
            request.as_str(),
            a.as_str(),
            response.as_str(),
            end.as_str(),
            after.as_str(),
        ];
        assert!(
            events_match(&swapped, &old).is_err(),
            "the main thread's order"
        );

        // Another content in one of its events.
        let changed_line = line("provider_request", "changed");
        let changed = [
            before.as_str(),
            start.as_str(),
            a.as_str(),
            changed_line.as_str(),
            b.as_str(),
            response.as_str(),
            end.as_str(),
            after.as_str(),
        ];
        assert!(events_match(&changed, &old).is_err(), "its content");

        // One missing: a count of lines is not all this looks at.
        let missing = [
            before.as_str(),
            start.as_str(),
            a.as_str(),
            b.as_str(),
            response.as_str(),
            end.as_str(),
            after.as_str(),
        ];
        assert!(events_match(&missing, &old).is_err(), "one fewer");

        // Out of its range, on either side.
        let early = [
            before.as_str(),
            request.as_str(),
            start.as_str(),
            a.as_str(),
            b.as_str(),
            response.as_str(),
            end.as_str(),
            after.as_str(),
        ];
        assert!(events_match(&early, &old).is_err(), "before the start");
        let late = [
            before.as_str(),
            start.as_str(),
            a.as_str(),
            request.as_str(),
            b.as_str(),
            end.as_str(),
            response.as_str(),
            after.as_str(),
        ];
        assert!(events_match(&late, &old).is_err(), "after the adoption");

        // And only those two names move.
        assert_eq!(
            EVENTS_OF_THE_OTHER_THREAD,
            ["provider_request", "provider_response"]
        );
    }

    /// Acceptance 6: the requests, events, outcome, and artifacts of the base.
    #[test]
    fn off_wire_events_and_artifacts_match_the_base_recording_byte_for_byte() {
        let _guard = lock();
        register_off_extension();
        let golden: Value = serde_json::from_str(OFF_GOLDEN).unwrap();
        for row in &OFF_ROWS {
            let recorded = record_off_row(row);
            let old = golden["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|old| old["id"] == row.id)
                .unwrap_or_else(|| panic!("no golden row {}", row.id));
            for key in [
                "outcome",
                "requests",
                "plan_requests",
                "artifacts",
                "unused_replies",
            ] {
                assert_eq!(recorded[key], old[key], "{}: {key}", row.id);
            }
            let (new, old) = (
                recorded["events"].as_array().unwrap(),
                old["events"].as_array().unwrap(),
            );
            if matches!(row.entry, Entry::UltraSpeculating) {
                assert_eq!(
                    events_match(&lines_of(new), &lines_of(old)),
                    Ok(()),
                    "{}",
                    row.id
                );
                continue;
            }
            for (index, (new, old)) in new.iter().zip(old).enumerate() {
                assert_eq!(new, old, "{}: event {index}", row.id);
            }
            assert_eq!(new.len(), old.len(), "{}: event count", row.id);
        }
    }

    #[test]
    fn the_corpus_manifest_pins_its_fixtures() {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(
            manifest["fixture_sha256"]["cases.json"],
            sha256(CASES.as_bytes())
        );
        assert_eq!(
            manifest["fixture_sha256"]["off-golden.json"],
            sha256(OFF_GOLDEN.as_bytes())
        );
        assert_eq!(manifest["base_sha"], BASE_SHA);
    }

    /// The corpus says which tests stand for each acceptance row. A renamed test fails here.
    #[test]
    fn the_corpus_names_what_runs() {
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

        let sources = [
            include_str!("structured_stage_receipts.rs"),
            include_str!("../src/tools/profile_tools/effects.rs"),
            include_str!("../src/planner/structured_stages/ledger.rs"),
            include_str!("../src/planner/structured_stages/state.rs"),
            include_str!("../src/planner/structured_stages/invalidation.rs"),
            include_str!("../src/planner/structured_stages/resume.rs"),
            include_str!("../src/planner/structured_stages/contract.rs"),
            include_str!("../src/planner/structured_stages/mod.rs"),
            include_str!(
                "../src/minimal_loop/loop_run/implementation_completion/structured_stage_completion.rs"
            ),
            include_str!("../src/tools/profile_tools/pre_run.rs"),
        ];
        let rows = cases["acceptance"].as_array().unwrap();
        assert_eq!(rows.len(), 6);
        for row in rows {
            let tests = row["tests"].as_array().unwrap();
            assert!(!tests.is_empty(), "{row}");
            for test in tests {
                let name = format!("fn {}(", test.as_str().unwrap());
                assert!(
                    sources.iter().any(|source| source.contains(&name)),
                    "{name} names nothing that runs"
                );
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn where_the_strict_isolation_does_not_exist_a_stage_tool_is_unavailable() {
    let _guard = lock();
    let env = Env::new();
    env.write_walk_tools(Some(floor()));
    let error = env.admission_error(WALK_PROFILE);
    assert!(
        error.starts_with("profile_tool_refused: unavailable:"),
        "{error}"
    );
    assert!(env.workspace().join(FIXED_INPUT).exists());
    assert!(!env.evidence.join("state").exists());
}

#[cfg(target_os = "macos")]
mod with_the_strict_isolation {
    use std::collections::VecDeque;
    use std::io::{Read as _, Write as _};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use commandagent::config::{
        Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
        PromptLayout, Provider,
    };
    use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
    use commandagent::planner::adjudication::contract::IntentId;
    use commandagent::planner::adjudication::investigate::InvestigationBindingEvidence;
    use commandagent::planner::runner::{run_step_plan, run_ultra_plan};
    use commandagent::planner::step_plan::{PlanStep, StepPlan};
    use commandagent::planner::ultra_plan::{UltraPhase, UltraPlan};
    use commandagent::providers::client_from_config;
    use commandagent::state::SessionSnapshot;
    use commandagent::tools::os_sandbox::{
        SandboxAvailability, SandboxMode, SandboxPolicy, install_for_test, reset_for_test,
    };
    use commandagent::tools::profile_tools::admit_selected;
    use commandagent::tui::NOOP_UI;
    use commandagent::tui::slash::handle_command;
    use serde_json::{Value, json};

    use super::{
        BARE_TOOL_ID, CANDIDATE, ENTRY_PROFILE, Env, FIXED_INPUT, LOWER_OUTPUT, LOWER_TOOL,
        MARKER_NAME_PROFILE, NO_ARGUMENT_PROFILE, UPPER_OUTPUT, UPPER_TOOL, WALK_PROFILE, floor,
        lock,
    };

    type Responder = Arc<dyn Fn(usize, &Value) -> String + Send + Sync>;

    struct FakeHttp {
        url: String,
        requests: Arc<Mutex<Vec<Value>>>,
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl FakeHttp {
        fn start(responder: Responder) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let requests: Arc<Mutex<Vec<Value>>> = Arc::default();
            let stop = Arc::new(AtomicBool::new(false));
            let thread = {
                let (requests, stop) = (requests.clone(), stop.clone());
                std::thread::spawn(move || {
                    while !stop.load(Ordering::SeqCst) {
                        match listener.accept() {
                            Ok((stream, _)) => serve(stream, &requests, &responder),
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

        fn finish(mut self) -> Vec<Value> {
            self.stop.store(true, Ordering::SeqCst);
            self.thread.take().unwrap().join().unwrap();
            self.requests.lock().unwrap().clone()
        }
    }

    fn serve(mut stream: std::net::TcpStream, requests: &Mutex<Vec<Value>>, responder: &Responder) {
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
        let body: Value = serde_json::from_slice(&bytes[body_start..]).unwrap_or(Value::Null);
        let index = {
            let mut requests = requests.lock().unwrap();
            requests.push(body.clone());
            requests.len()
        };
        let reply = responder(index, &body);
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
            reply.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }

    fn text_reply(content: &str) -> String {
        json!({"choices": [{"message": {"role": "assistant", "content": content}}]}).to_string()
    }

    fn tool_reply(name: &str, mode: &str, revision: &str) -> String {
        let arguments = json!({"mode": mode, "expected_revision": revision}).to_string();
        json!({"choices": [{"message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call-0",
                "type": "function",
                "function": {"name": name, "arguments": arguments},
            }],
        }}]})
        .to_string()
    }

    fn read_reply(path: &str) -> String {
        json!({"choices": [{"message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "id": "call-1",
                "type": "function",
                "function": {
                    "name": "Read",
                    "arguments": json!({"path": path}).to_string(),
                },
            }],
        }}]})
        .to_string()
    }

    fn has_tool_answer(body: &Value) -> bool {
        body["messages"]
            .as_array()
            .is_some_and(|messages| messages.iter().any(|message| message["role"] == "tool"))
    }

    /// What the host answered to the last call, as the model reads it.
    fn last_tool_answer(body: &Value) -> Value {
        let text = body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .find(|message| message["role"] == "tool")
            .and_then(|message| message["content"].as_str())
            .unwrap_or_else(|| panic!("no tool answer: {body}"));
        serde_json::from_str(text).unwrap_or_else(|_| panic!("not data: {text}"))
    }

    fn config(env: &Env, profile: &str, url: &str) -> Config {
        Config {
            workspace_root: env.root.clone(),
            state_dir: env.evidence.join("state"),
            eval_events_path: Some(env.evidence.join("events.jsonl")),
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

    fn install_sandbox(env: &Env) {
        let tmp = env.evidence.join("sandbox/run");
        std::fs::create_dir_all(&tmp).unwrap();
        install_for_test(
            SandboxMode::Auto,
            SandboxPolicy::new(&env.root, &tmp),
            SandboxAvailability::Available,
        );
    }

    fn run(env: &Env, profile: &str, responder: Responder) -> (Vec<Value>, Result<(), String>) {
        let http = FakeHttp::start(responder);
        let config = config(env, profile, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let result = run_session_with_outcome_with_ui(
            client.as_mut(),
            &mut SessionSnapshot::new(),
            "Submit both stages.",
            &[],
            &config,
            &NOOP_UI,
        )
        .map(|_| ())
        .map_err(|error| format!("{error:#}"));
        (http.finish(), result)
    }

    /// A failed check answered, a seal, and the stage below going through on the revision the
    /// seal's answer named. The input it reads did not exist when the tools were admitted.
    #[test]
    fn two_stages_go_through_real_tool_calls_and_the_lower_reads_what_the_upper_committed() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let upper_seen = Arc::new(Mutex::new(VecDeque::new()));
        let seen = upper_seen.clone();
        let responder: Responder = Arc::new(move |index, body| match index {
            1 => tool_reply(UPPER_TOOL, "bad", "initial"),
            2 => {
                seen.lock().unwrap().push_back(last_tool_answer(body));
                tool_reply(UPPER_TOOL, "good", "initial")
            }
            3 => {
                let answer = last_tool_answer(body);
                let revision = answer["stage"]["downstream"][0]["revision"]
                    .as_str()
                    .unwrap_or_else(|| panic!("no revision below: {answer}"))
                    .to_string();
                seen.lock().unwrap().push_back(answer);
                tool_reply(LOWER_TOOL, "good", &revision)
            }
            _ => {
                seen.lock().unwrap().push_back(last_tool_answer(body));
                text_reply("done")
            }
        });
        let (requests, result) = run(&env, WALK_PROFILE, responder);
        reset_for_test();
        result.unwrap();
        assert_eq!(requests.len(), 4);

        let seen = upper_seen.lock().unwrap();
        assert_eq!(seen[0]["status"], "resubmit", "{}", seen[0]);
        assert_eq!(seen[0]["code"], "check_failed");
        assert_eq!(seen[0]["diagnostics"][0]["message"], "the shape is off");
        assert_eq!(seen[1]["stage"]["status"], "sealed", "{}", seen[1]);
        assert_eq!(seen[2]["stage"]["status"], "sealed", "{}", seen[2]);

        assert_eq!(
            std::fs::read(env.root.join(UPPER_OUTPUT)).unwrap(),
            CANDIDATE
        );
        assert_eq!(
            std::fs::read(env.root.join(LOWER_OUTPUT)).unwrap(),
            CANDIDATE
        );
        assert!(env.workspace().join(FIXED_INPUT).exists());
        assert!(
            env.evidence.join("state/structured-stages").is_dir(),
            "the record is outside the workspace"
        );
        assert!(!env.root.join("structured-stages").exists());
    }

    fn assert_not_a_success(result: &Result<(), String>, label: &str) {
        let error = result
            .as_ref()
            .err()
            .unwrap_or_else(|| panic!("{label}: the run reported a success"));
        assert!(error.contains("stage_incomplete"), "{label}: {error}");
    }

    const CLAIM: &str = "All stages are sealed and every check passed.";

    /// What a model says about a stage is not what the host decides on.
    #[test]
    fn a_failed_upper_stage_is_no_success_whatever_the_model_says() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let responder: Responder = Arc::new(|index, _| match index {
            1 => tool_reply(UPPER_TOOL, "bad", "initial"),
            _ => text_reply(CLAIM),
        });
        let (requests, result) = run(&env, WALK_PROFILE, responder);
        reset_for_test();
        assert_not_a_success(&result, "upper failed");
        assert!(
            requests.len() >= 3,
            "the claim was answered: {}",
            requests.len()
        );
        assert!(!env.root.join(UPPER_OUTPUT).exists());
    }

    #[test]
    fn a_lower_stage_never_submitted_is_no_success_whatever_the_model_says() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let responder: Responder = Arc::new(|index, _| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            _ => text_reply(CLAIM),
        });
        let (_, result) = run(&env, WALK_PROFILE, responder);
        reset_for_test();
        assert_not_a_success(&result, "lower missing");
        assert_eq!(
            std::fs::read(env.root.join(UPPER_OUTPUT)).unwrap(),
            CANDIDATE
        );
        assert!(!env.root.join(LOWER_OUTPUT).exists());
    }

    /// The commit's answer reaches the model in its fixed form here, without the name below. The
    /// host's decision does not go through that text.
    #[test]
    fn a_name_the_history_redacts_changes_nothing_the_host_decides() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let seen: Arc<Mutex<Vec<Value>>> = Arc::default();
        let kept = seen.clone();
        let responder: Responder = Arc::new(move |index, body| {
            if index == 1 {
                return tool_reply(UPPER_TOOL, "good", "initial");
            }
            if index == 2 {
                kept.lock().unwrap().push(last_tool_answer(body));
            }
            text_reply(CLAIM)
        });
        let (_, result) = run(&env, MARKER_NAME_PROFILE, responder);
        reset_for_test();
        assert_not_a_success(&result, "marker name");
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0]["stage"]["status"], "sealed", "{}", seen[0]);
        assert_eq!(seen[0]["truncated"], true, "{}", seen[0]);
    }

    fn setup_step(instruction: &str) -> PlanStep {
        PlanStep {
            id: "step-setup".to_string(),
            kind: "setup".to_string(),
            expected_result: "pass".to_string(),
            instruction: instruction.to_string(),
            expected_paths: Vec::new(),
            verify: Vec::new(),
        }
    }

    /// T-A (B1): a plain step plan has no final acceptance. A step answers for what it called, so
    /// the plan's end is what answers for the rest.
    #[test]
    fn a_plan_that_never_calls_the_lower_stage_is_no_success() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let responder: Responder = Arc::new(|index, _| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            _ => text_reply("done"),
        });
        let http = FakeHttp::start(responder);
        let config = config(&env, WALK_PROFILE, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let plan = StepPlan {
            goal: "Submit the upper stage.".to_string(),
            steps: vec![setup_step("Submit the upper stage")],
        };
        let result = run_step_plan(client.as_mut(), &plan, &config);
        http.finish();
        reset_for_test();
        let error = result.expect_err("the lower stage was never submitted");
        assert!(error.to_string().contains("stage_incomplete"), "{error:#}");
        assert!(error.to_string().contains("lower/main"), "{error:#}");
    }

    /// The control for T-A: a plan that sealed everything is a success.
    #[test]
    fn a_plan_that_seals_every_stage_succeeds() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        // The second step's first call is written before it knows the revision below, and the
        // answer tells it.
        let responder: Responder = Arc::new(|index, body| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            2 => tool_reply(LOWER_TOOL, "good", "initial"),
            _ => {
                let answer = last_tool_answer(body);
                match answer["revision"].as_str() {
                    Some(revision) if answer["status"] == "stale" => {
                        tool_reply(LOWER_TOOL, "good", revision)
                    }
                    _ => text_reply("done"),
                }
            }
        });
        let http = FakeHttp::start(responder);
        let config = config(&env, WALK_PROFILE, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let mut second = setup_step("Submit the lower stage");
        second.id = "step-setup-lower".to_string();
        let plan = StepPlan {
            goal: "Submit both stages.".to_string(),
            steps: vec![setup_step("Submit the upper stage"), second],
        };
        let result = run_step_plan(client.as_mut(), &plan, &config);
        http.finish();
        reset_for_test();
        result.unwrap();
        assert_eq!(
            std::fs::read(env.root.join(LOWER_OUTPUT)).unwrap(),
            CANDIDATE
        );
    }

    /// With no step there is no session, so no stage was ever bound. Declared stages are still
    /// declared: nothing vouched for them.
    #[test]
    fn a_plan_with_no_session_answers_for_no_stage_and_is_no_success() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let http = FakeHttp::start(Arc::new(|_, _| text_reply("nobody asked")));
        let config = config(&env, WALK_PROFILE, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let plan = StepPlan {
            goal: "Do nothing.".to_string(),
            steps: Vec::new(),
        };
        let result = run_step_plan(client.as_mut(), &plan, &config);
        http.finish();
        reset_for_test();
        let error = result.expect_err("no stage was ever bound");
        assert!(
            error
                .to_string()
                .contains("stage_incomplete: no_session_bound_the_declared_stages"),
            "{error:#}"
        );
    }

    fn two_phase_plan(intent: &str, first: &str, second: &str) -> UltraPlan {
        UltraPlan {
            goal: "Submit the stages".to_string(),
            profile: WALK_PROFILE.to_string(),
            style: "default".to_string(),
            intent: intent.to_string(),
            phases: vec![
                UltraPhase {
                    id: "first".to_string(),
                    prompt: format!("Phase-first: {first}"),
                },
                UltraPhase {
                    id: "second".to_string(),
                    prompt: format!("Phase-second: {second}"),
                },
            ],
        }
    }

    fn one_step_plan(goal: &str) -> Value {
        json!({"goal": goal, "steps": [{
            "id": "step-setup",
            "kind": "setup",
            "expected_result": "pass",
            "instruction": goal,
            "expected_paths": [],
            "verify": [],
        }]})
    }

    /// Phases that hand out their plans by what they are asked, and a model that submits what its
    /// step says. The first answer that is stale gives the revision to go on with.
    fn ultra_responder(submit_lower: bool) -> Responder {
        Arc::new(move |_, body| {
            let text = body.to_string();
            if !body["tools"].to_string().contains(UPPER_TOOL) {
                let goal = if text.contains("Phase-second") {
                    "Submit the lower stage"
                } else {
                    "Submit the upper stage"
                };
                return text_reply(&one_step_plan(goal).to_string());
            }
            let wants_lower = text.contains("Submit the lower stage");
            if !wants_lower {
                return if has_tool_answer(body) {
                    text_reply("done")
                } else {
                    tool_reply(UPPER_TOOL, "good", "initial")
                };
            }
            if !submit_lower {
                // Something for the step to have done, that is no stage call.
                let answered = body["messages"]
                    .as_array()
                    .and_then(|messages| messages.last())
                    .is_some_and(|message| message["role"] == "tool");
                return if answered {
                    text_reply("done")
                } else {
                    read_reply(FIXED_INPUT)
                };
            }
            let answer = has_tool_answer(body).then(|| last_tool_answer(body));
            match answer {
                Some(answer) if answer["status"] == "stale" => {
                    tool_reply(LOWER_TOOL, "good", answer["revision"].as_str().unwrap())
                }
                Some(answer) if answer["stage"]["status"] == "sealed" => {
                    if answer["path"] == LOWER_OUTPUT {
                        text_reply("done")
                    } else {
                        tool_reply(LOWER_TOOL, "good", "initial")
                    }
                }
                _ => tool_reply(LOWER_TOOL, "good", "initial"),
            }
        })
    }

    fn run_two_phases(env: &Env, responder: Responder) -> (Vec<Value>, anyhow::Result<String>) {
        std::fs::write(env.root.join("notes.txt"), "the notes\n").unwrap();
        let http = FakeHttp::start(responder);
        let config = config(env, WALK_PROFILE, &http.url);
        let mut planner = client_from_config(&config, true).unwrap();
        let mut execution = client_from_config(&config, false).unwrap();
        let plan = two_phase_plan(
            "create",
            "submit the upper stage.",
            "submit the lower stage.",
        );
        let result = run_ultra_plan(planner.as_mut(), execution.as_mut(), &plan, &config);
        (http.finish(), result)
    }

    /// The control for final acceptance: it must not refuse a run that met every obligation. A
    /// session that did not leave its set for the gate to find would fail here.
    #[test]
    fn an_ultra_plan_that_seals_every_stage_succeeds() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let (_, result) = run_two_phases(&env, ultra_responder(true));
        reset_for_test();
        let summary = result.unwrap_or_else(|error| panic!("{error:#}"));
        assert!(summary.contains("complete"), "{summary}");
        assert_eq!(
            std::fs::read(env.root.join(LOWER_OUTPUT)).unwrap(),
            CANDIDATE
        );
    }

    /// Each step answers only for what it called, so only the final gate can see the stage nobody
    /// called. Its failure is in what acceptance reports.
    #[test]
    fn an_ultra_plan_that_leaves_a_stage_open_fails_at_final_acceptance() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let (_, result) = run_two_phases(&env, ultra_responder(false));
        reset_for_test();
        result.expect_err("the lower stage was never submitted");
        let events = std::fs::read_to_string(env.evidence.join("events.jsonl")).unwrap();
        let failed = events
            .lines()
            .find(|line| line.contains("\"event\":\"ultra_final_acceptance_failed\""))
            .expect("the steps passed and acceptance is what refused");
        assert!(failed.contains("stage_incomplete"), "{failed}");
        assert!(failed.contains("lower/main"), "{failed}");
        assert!(events.contains("structured_stage_incomplete"), "{events}");
    }

    /// `post_write_read_confirmation_completed` ends a run at a Read after a commit. With a stage
    /// open, that end would give the model no chance to go on: it is held back and asked again.
    #[test]
    fn a_read_after_a_commit_does_not_end_a_run_with_a_stage_open() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let responder: Responder = Arc::new(move |index, _| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            2 => read_reply(UPPER_OUTPUT),
            _ => text_reply("stopping"),
        });
        let (requests, result) = run(&env, WALK_PROFILE, responder);
        reset_for_test();
        assert_not_a_success(&result, "the lower stage is open");
        assert!(requests.len() > 2, "ended at the Read: {}", requests.len());
    }

    /// An implement step counts what starts no process as reading (S8), and a seal as a move. The
    /// loop's own stagnation notice is what tells the two apart.
    fn stagnation_notices(replies: Vec<&'static str>) -> (usize, String) {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let responder: Responder = Arc::new(move |index, _| match replies.get(index - 1) {
            Some(&"seal") => tool_reply(UPPER_TOOL, "good", "initial"),
            Some(&"check") => tool_reply(UPPER_TOOL, "bad", "initial"),
            Some(&"stale") => tool_reply(UPPER_TOOL, "bad", "not-a-revision"),
            _ => text_reply("stopping"),
        });
        let http = FakeHttp::start(responder);
        let config = config(&env, WALK_PROFILE, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let mut step = setup_step("Submit the upper stage");
        step.kind = "implement".to_string();
        // Never written, so no call here reduces what is missing: only the stage's note can.
        step.expected_paths = vec!["notes.txt".to_string()];
        let plan = StepPlan {
            goal: "Submit the upper stage.".to_string(),
            steps: vec![step],
        };
        let _ = run_step_plan(client.as_mut(), &plan, &config);
        http.finish();
        reset_for_test();
        let events = std::fs::read_to_string(env.evidence.join("events.jsonl")).unwrap();
        let notices = events
            .lines()
            .filter(|line| line.contains("\"event\":\"read_only_stagnation_feedback\""))
            .count();
        (notices, events)
    }

    /// `is_stage_tool`: a call that starts nothing counts as reading.
    #[test]
    fn repeated_calls_that_start_nothing_read_as_stagnation_to_the_loop() {
        let (notices, events) = stagnation_notices(vec!["stale", "stale", "stale", "stale"]);
        assert!(notices >= 1, "{events}");
    }

    /// `batch_all_read_only_tools = false`: a seal is a move, so the count starts over at it.
    #[test]
    fn a_seal_between_them_starts_the_count_over() {
        let (notices, events) =
            stagnation_notices(vec!["stale", "stale", "seal", "stale", "stale"]);
        assert_eq!(notices, 0, "{events}");
    }

    /// A plan of the given intent: a reproducer that fails first, the upper stage, and the lower
    /// stage if asked for. Everything its own adjudication asks for is in place, so what is left
    /// to say no is the stages. The controls with every stage sealed show that.
    fn run_intent(
        env: &Env,
        intent: &str,
        reproducer: &'static str,
        seal_lower: bool,
    ) -> anyhow::Result<String> {
        std::fs::write(env.root.join("notes.txt"), "the notes\n").unwrap();
        let responder: Responder = Arc::new(move |_, body| {
            let text = body.to_string();
            if !body["tools"].to_string().contains(UPPER_TOOL) {
                if text.contains("Phase-reproduce") {
                    return text_reply(
                        &json!({"goal": "reproduce", "steps": [{
                            "id": "step-reproduce",
                            "kind": "verify",
                            "expected_result": "fail",
                            "instruction": "Reproduce the failure",
                            "expected_paths": [],
                            "verify": [reproducer],
                        }]})
                        .to_string(),
                    );
                }
                let goal = if text.contains("Phase-lower") {
                    "Submit the lower stage"
                } else {
                    "Submit the upper stage"
                };
                return text_reply(&one_step_plan(goal).to_string());
            }
            if !text.contains("Submit the lower stage") {
                return if has_tool_answer(body) {
                    text_reply("done")
                } else {
                    tool_reply(UPPER_TOOL, "good", "initial")
                };
            }
            let answer = last_tool_answer(body);
            if answer["status"] == "stale" {
                tool_reply(LOWER_TOOL, "good", answer["revision"].as_str().unwrap())
            } else if answer["path"] == LOWER_OUTPUT {
                text_reply("done")
            } else {
                tool_reply(LOWER_TOOL, "good", "initial")
            }
        });
        let http = FakeHttp::start(responder);
        let mut config = config(env, WALK_PROFILE, &http.url);
        if intent == "investigate" {
            config.intent_override = Some(IntentId::Investigate);
        }
        let mut planner = client_from_config(&config, true).unwrap();
        let mut execution = client_from_config(&config, false).unwrap();
        let phase = |id: &str, prompt: &str| UltraPhase {
            id: id.to_string(),
            prompt: prompt.to_string(),
        };
        let mut phases = vec![
            phase("reproduce", "Phase-reproduce: reproduce the failure."),
            phase("upper", "Phase-upper: submit the upper stage."),
        ];
        if seal_lower {
            phases.push(phase("lower", "Phase-lower: submit the lower stage."));
        }
        let plan = UltraPlan {
            goal: "Find out why it fails".to_string(),
            profile: WALK_PROFILE.to_string(),
            style: "default".to_string(),
            intent: intent.to_string(),
            phases,
        };
        let result = run_ultra_plan(planner.as_mut(), execution.as_mut(), &plan, &config);
        http.finish();
        result
    }

    /// What an investigation's own adjudication takes as enough: a report and a binding with no
    /// claim in it, which it calls partial and lets through.
    fn write_investigation_evidence(env: &Env) {
        std::fs::create_dir_all(env.root.join("output")).unwrap();
        std::fs::write(env.root.join("output/diagnosis.md"), "# Diagnosis\n").unwrap();
        std::fs::create_dir_all(env.root.join("evidence")).unwrap();
        std::fs::write(
            env.root.join("evidence/investigation-binding.json"),
            serde_json::to_vec(&InvestigationBindingEvidence::new(Vec::new())).unwrap(),
        )
        .unwrap();
    }

    fn intent_env() -> Env {
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        env
    }

    /// T-B (B2): an investigation ends at its own adjudication and has no final acceptance.
    #[test]
    fn an_investigation_that_leaves_a_stage_open_is_no_success() {
        let _guard = lock();
        let env = intent_env();
        write_investigation_evidence(&env);
        let result = run_intent(&env, "investigate", "test -f no-such-marker", false);
        reset_for_test();
        let error = result.expect_err("the lower stage was never submitted");
        assert!(
            error.to_string().contains("stage_incomplete"),
            "another reason would say nothing about the stages: {error:#}"
        );
        assert!(error.to_string().contains("lower/main"), "{error:#}");
    }

    #[test]
    fn an_investigation_that_seals_every_stage_succeeds() {
        let _guard = lock();
        let env = intent_env();
        write_investigation_evidence(&env);
        let result = run_intent(&env, "investigate", "test -f no-such-marker", true);
        reset_for_test();
        let summary = result.unwrap_or_else(|error| panic!("{error:#}"));
        assert!(summary.contains("complete"), "{summary}");
    }

    /// T-B (B2): a fix ends at its own adjudication too. The reproducer asks for the upper stage's
    /// output, so sealing that stage is what passes it.
    #[test]
    fn a_fix_that_leaves_a_stage_open_is_no_success() {
        let _guard = lock();
        let env = intent_env();
        let result = run_intent(&env, "fix", "test -f out/upper.json", false);
        reset_for_test();
        let error = result.expect_err("the lower stage was never submitted");
        assert!(
            error.to_string().contains("stage_incomplete"),
            "another reason would say nothing about the stages: {error:#}"
        );
        assert!(error.to_string().contains("lower/main"), "{error:#}");
    }

    #[test]
    fn a_fix_that_seals_every_stage_succeeds() {
        let _guard = lock();
        let env = intent_env();
        let result = run_intent(&env, "fix", "test -f out/upper.json", true);
        reset_for_test();
        let summary = result.unwrap_or_else(|error| panic!("{error:#}"));
        assert!(summary.contains("complete"), "{summary}");
    }

    /// T1: a fix's own commands run after the steps. A reproducer that rewrites what a stage
    /// sealed leaves bytes the seal does not stand for.
    #[test]
    fn a_fix_whose_reproducer_rewrites_a_sealed_output_is_no_success() {
        let _guard = lock();
        let env = intent_env();
        std::fs::write(
            env.root.join("rewrite.sh"),
            "test -f out/upper.json || exit 1\n[ ! -f out/lower.json ] || printf x >> out/lower.json\n",
        )
        .unwrap();
        let result = run_intent(&env, "fix", "sh rewrite.sh", true);
        reset_for_test();
        let error = result.expect_err("the seal no longer stands for the bytes");
        assert!(error.to_string().contains("stage_incomplete"), "{error:#}");
        assert!(error.to_string().contains("lower/main"), "{error:#}");
    }

    /// T7: `--plan-run` with a plan of two steps seals both.
    #[test]
    fn the_binary_on_plan_run_succeeds_with_every_stage_sealed() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        let step = |id: &str, what: &str| {
            json!({"id": id, "kind": "setup", "expected_result": "pass", "instruction": what,
                   "expected_paths": [], "verify": []})
        };
        let plan = json!({"goal": "Submit the stages", "steps": [
            step("step-upper", "Submit the upper stage"),
            step("step-lower", "Submit the lower stage"),
        ]});
        let http = FakeHttp::start(Arc::new(move |index, body| match index {
            1 => text_reply(&plan.to_string()),
            2 => tool_reply(UPPER_TOOL, "good", "initial"),
            3 => tool_reply(LOWER_TOOL, "good", "initial"),
            _ => {
                let answer = last_tool_answer(body);
                match answer["revision"].as_str() {
                    Some(revision) if answer["status"] == "stale" => {
                        tool_reply(LOWER_TOOL, "good", revision)
                    }
                    _ => text_reply("done"),
                }
            }
        }));
        let output = binary(&env, &http.url, &["--plan-run", "Submit the stages"]);
        http.finish();
        assert_entry("--plan-run", true, &output, &env);
    }

    /// T8: what a stage set with automatic recovery spends before it is refused: the phase's plan,
    /// as the known limit says, and no treatment and no second try.
    #[test]
    fn a_generated_plan_that_may_recover_by_itself_makes_no_treatment() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        std::fs::write(env.root.join("notes.txt"), "the notes\n").unwrap();
        let http = FakeHttp::start(entry_responder(true));
        let output = binary(
            &env,
            &http.url,
            &[
                "--recovery-plan-auto-runs",
                "1",
                "--ultra-plan-run",
                "Submit the stages",
            ],
        );
        let requests = http.finish();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{stderr}");
        assert!(stderr.contains("RecoversByItself"), "{stderr}");
        assert!(!env.root.join(".commandagent/recovery-treatments").exists());
        assert_eq!(requests.len(), 1, "spent: {}", requests.len());
    }

    /// The child takes only what is named here, and `output` waits for it.
    fn binary(env: &Env, url: &str, arguments: &[&str]) -> std::process::Output {
        let home = tempfile::tempdir().unwrap();
        std::process::Command::new(env!("CARGO_BIN_EXE_commandagent"))
            .arg("--extension-root")
            .arg(super::extension_root())
            .args(["--model", "m", "--chat-retries", "0"])
            .args(["--provider", "lm-studio", "--lm-studio-host", url, "--yes"])
            .args(["--max-iterations", "8"])
            .arg("--state-dir")
            .arg(env.evidence.join("state"))
            .args(["--profile", ENTRY_PROFILE])
            .args(arguments)
            .current_dir(&env.root)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home.path())
            .env(super::REGISTRY_ENV, &env.registry)
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap()
    }

    /// An entry's plan and a model that submits what its step says. `seal_lower` false leaves
    /// the lower stage to nobody. The request for the plan is the one that offers no tool.
    fn entry_responder(seal_lower: bool) -> Responder {
        Arc::new(move |_, body| {
            let text = body.to_string();
            if !body["tools"].to_string().contains(UPPER_TOOL) {
                let goal = if text.contains("Phase-second") {
                    "Submit the lower stage"
                } else {
                    "Submit the upper stage"
                };
                return text_reply(&one_step_plan(goal).to_string());
            }
            ultra_submission(seal_lower, &text, body)
        })
    }

    fn ultra_submission(seal_lower: bool, text: &str, body: &Value) -> String {
        if !text.contains("Submit the lower stage") {
            return if has_tool_answer(body) {
                text_reply("done")
            } else {
                tool_reply(UPPER_TOOL, "good", "initial")
            };
        }
        if !seal_lower {
            let answered = body["messages"]
                .as_array()
                .and_then(|messages| messages.last())
                .is_some_and(|message| message["role"] == "tool");
            return if answered {
                text_reply("done")
            } else {
                read_reply(FIXED_INPUT)
            };
        }
        let answer = last_tool_answer(body);
        if answer["status"] == "stale" {
            tool_reply(LOWER_TOOL, "good", answer["revision"].as_str().unwrap())
        } else if answer["path"] == LOWER_OUTPUT {
            text_reply("done")
        } else {
            tool_reply(LOWER_TOOL, "good", "initial")
        }
    }

    /// `--prompt` on the binary: the model submits both stages, or stops after the first.
    #[test]
    fn the_binary_on_prompt_succeeds_only_with_every_stage_sealed() {
        let _guard = lock();
        for seal_lower in [true, false] {
            let env = Env::new();
            env.write_walk_tools(Some(floor()));
            let http = FakeHttp::start(Arc::new(move |index, body| match index {
                1 => tool_reply(UPPER_TOOL, "good", "initial"),
                2 if seal_lower => {
                    let answer = last_tool_answer(body);
                    let revision = answer["stage"]["downstream"][0]["revision"]
                        .as_str()
                        .unwrap()
                        .to_string();
                    tool_reply(LOWER_TOOL, "good", &revision)
                }
                _ => text_reply("All stages are sealed and every check passed."),
            }));
            let output = binary(&env, &http.url, &["--prompt", "Submit both stages."]);
            http.finish();
            assert_entry("--prompt", seal_lower, &output, &env);
        }
    }

    fn assert_entry(entry: &str, sealed: bool, output: &std::process::Output, env: &Env) {
        let said = format!(
            "{entry} sealed={sealed}: {}\n--- stdout\n{}\n--- stderr\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(output.status.success(), sealed, "{said}");
        if !sealed {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("stage_incomplete")
                    || String::from_utf8_lossy(&output.stdout).contains("stage_incomplete"),
                "another reason would say nothing about the stages: {said}"
            );
        }
        assert!(env.root.join(UPPER_OUTPUT).exists(), "{said}");
        assert_eq!(env.root.join(LOWER_OUTPUT).exists(), sealed, "{said}");
    }

    /// `--ultra-plan-run` on the binary.
    #[test]
    fn the_binary_on_generated_plans_succeeds_only_with_every_stage_sealed() {
        let _guard = lock();
        for seal_lower in [true, false] {
            let env = Env::new();
            env.write_walk_tools(Some(floor()));
            std::fs::write(env.root.join("notes.txt"), "the notes\n").unwrap();
            let http = FakeHttp::start(entry_responder(seal_lower));
            let output = binary(&env, &http.url, &["--ultra-plan-run", "Submit the stages"]);
            http.finish();
            assert_entry("--ultra-plan-run", seal_lower, &output, &env);
        }
    }

    /// `--plan-run` generates one plan, so its one step seals what it can. The stage below it stays
    /// open, and the plan's end says so: its control with both sealed is `/run-plan` below.
    #[test]
    fn the_binary_on_plan_run_refuses_a_stage_left_open() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        let http = FakeHttp::start(entry_responder(false));
        let output = binary(&env, &http.url, &["--plan-run", "Submit the upper stage"]);
        http.finish();
        assert_entry("--plan-run", false, &output, &env);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("stage_incomplete"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// `--run-plan` on the binary, and `/run-plan` through the slash command it shares.
    #[test]
    fn a_plan_file_succeeds_only_with_every_stage_sealed() {
        let _guard = lock();
        let two_steps: Responder = Arc::new(|index, body| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            2 => tool_reply(LOWER_TOOL, "good", "initial"),
            _ => {
                let answer = last_tool_answer(body);
                match answer["revision"].as_str() {
                    Some(revision) if answer["status"] == "stale" => {
                        tool_reply(LOWER_TOOL, "good", revision)
                    }
                    _ => text_reply("done"),
                }
            }
        });
        let one_step: Responder = Arc::new(|index, _| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            _ => text_reply("done"),
        });
        let step_yaml = |id: &str, what: &str| {
            format!(
                "  - id: \"{id}\"\n    kind: \"setup\"\n    expected_result: \"pass\"\n    instruction: \"{what}\"\n    expected_paths: []\n    verify: []\n"
            )
        };
        let upper = step_yaml("step-upper", "Submit the upper stage");
        let lower = step_yaml("step-lower", "Submit the lower stage");
        for (sealed, responder, steps) in [
            (true, two_steps, format!("{upper}{lower}")),
            (false, one_step, upper.clone()),
        ] {
            // The binary.
            let env = Env::new();
            env.write_walk_tools(Some(floor()));
            let plan = format!("goal: \"Submit the stages\"\nsteps:\n{steps}");
            std::fs::write(env.root.join("steps.yaml"), &plan).unwrap();
            let http = FakeHttp::start(responder.clone());
            let output = binary(&env, &http.url, &["--run-plan", "steps.yaml"]);
            http.finish();
            assert_entry("--run-plan", sealed, &output, &env);

            // The slash command.
            let env = Env::new();
            env.write_walk_tools(Some(floor()));
            install_sandbox(&env);
            std::fs::write(env.root.join("steps.yaml"), &plan).unwrap();
            let http = FakeHttp::start(responder);
            let config = config(&env, WALK_PROFILE, &http.url);
            let mut planner = client_from_config(&config, true).unwrap();
            let mut execution = client_from_config(&config, false).unwrap();
            let result = handle_command(
                "/run-plan steps.yaml",
                &config,
                planner.as_mut(),
                execution.as_mut(),
                &NOOP_UI,
            );
            http.finish();
            reset_for_test();
            assert_eq!(result.is_ok(), sealed, "/run-plan: {result:?}");
            assert_eq!(env.root.join(LOWER_OUTPUT).exists(), sealed, "/run-plan");
        }
    }

    /// `/ultra-plan-run` is the command a REPL's `/confirm` dispatches once Gate 1 is through.
    #[test]
    fn the_command_a_confirmation_dispatches_succeeds_only_with_every_stage_sealed() {
        let _guard = lock();
        for seal_lower in [true, false] {
            let env = Env::new();
            env.write_walk_tools(Some(floor()));
            install_sandbox(&env);
            std::fs::write(env.root.join("notes.txt"), "the notes\n").unwrap();
            let http = FakeHttp::start(entry_responder(seal_lower));
            let mut config = config(&env, ENTRY_PROFILE, &http.url);
            config.plan_preset = PlanPreset::Profile;
            let mut planner = client_from_config(&config, true).unwrap();
            let mut execution = client_from_config(&config, false).unwrap();
            let command = format!("/ultra-plan-run --profile {ENTRY_PROFILE} Submit the stages");
            let result = handle_command(
                &command,
                &config,
                planner.as_mut(),
                execution.as_mut(),
                &NOOP_UI,
            );
            http.finish();
            reset_for_test();
            assert_eq!(result.is_ok(), seal_lower, "{result:?}");
        }
    }

    /// B4: a treatment runs in a workspace of its own. The run is refused before any call.
    #[test]
    fn a_session_that_may_recover_by_itself_gets_no_stages_and_no_provider_call() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let http = FakeHttp::start(Arc::new(|_, _| text_reply("a reply nobody asked for")));
        let mut config = config(&env, WALK_PROFILE, &http.url);
        config.recovery_plan_auto_runs = 1;
        let mut client = client_from_config(&config, false).unwrap();
        let result = run_session_with_outcome_with_ui(
            client.as_mut(),
            &mut SessionSnapshot::new(),
            "Submit both stages.",
            &[],
            &config,
            &NOOP_UI,
        )
        .map(|_| ())
        .map_err(|error| format!("{error:#}"));
        let requests = http.finish();
        reset_for_test();
        let error = result.unwrap_err();
        assert!(error.contains("stage_contract_refused"), "{error}");
        assert!(error.contains("RecoversByItself"), "{error}");
        assert!(requests.is_empty());
        assert!(!env.evidence.join("state/structured-stages").exists());
    }

    /// F5: a plan's setup step ends at a tool result. A failed check must not end it unread.
    #[test]
    fn a_failed_check_in_a_setup_step_goes_back_to_the_model() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        let responder: Responder = Arc::new(|index, body| match index {
            1 => tool_reply(UPPER_TOOL, "bad", "initial"),
            2 => {
                let answer = last_tool_answer(body);
                assert_eq!(answer["status"], "resubmit", "{answer}");
                tool_reply(UPPER_TOOL, "good", "initial")
            }
            _ => text_reply("done"),
        });
        let http = FakeHttp::start(responder);
        let config = config(&env, WALK_PROFILE, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let plan = StepPlan {
            goal: "Submit the upper stage.".to_string(),
            steps: vec![PlanStep {
                id: "step-setup".to_string(),
                kind: "setup".to_string(),
                expected_result: "pass".to_string(),
                instruction: "Submit the upper stage".to_string(),
                expected_paths: Vec::new(),
                verify: Vec::new(),
            }],
        };
        let result = run_step_plan(client.as_mut(), &plan, &config);
        let requests = http.finish();
        reset_for_test();
        assert!(requests.len() >= 2, "no second request: {result:?}");
        assert_eq!(
            std::fs::read(env.root.join(UPPER_OUTPUT)).unwrap(),
            CANDIDATE,
            "{result:?}"
        );
        let events = std::fs::read_to_string(env.evidence.join("events.jsonl")).unwrap();
        let held = events
            .lines()
            .position(|line| line.contains("\"reason\":\"step_tool_observation_completed\""));
        let second = events
            .lines()
            .filter(|line| line.contains("\"event\":\"tool_execute\""))
            .count();
        assert_eq!(second, 2, "both calls ran");
        assert!(
            held.is_some(),
            "the step ends once the stage it touched is sealed: {result:?}"
        );
    }

    const PREVIOUS_ROOT: &str = "STAGE_RESUME_PREVIOUS_ROOT";
    const PREVIOUS_EVIDENCE: &str = "STAGE_RESUME_PREVIOUS_EVIDENCE";

    /// Not a test by itself: the parent below starts this in a process of its own, so that the
    /// ledger it leaves is another process's and its lock is gone when it ends.
    #[test]
    fn previous_process_of_the_resume_test() {
        let (Some(root), Some(evidence)) = (
            std::env::var_os(PREVIOUS_ROOT),
            std::env::var_os(PREVIOUS_EVIDENCE),
        ) else {
            return;
        };
        let _guard = lock();
        let env = Env::over(root.into(), evidence.into());
        install_sandbox(&env);
        let responder: Responder = Arc::new(|index, _| match index {
            1 => tool_reply(UPPER_TOOL, "good", "initial"),
            _ => text_reply("that is all for now"),
        });
        let (_, result) = run(&env, WALK_PROFILE, responder);
        reset_for_test();
        assert_not_a_success(&result, "the previous process left the lower stage open");
    }

    fn saved_attempts(env: &Env) -> u64 {
        let leaf = env.evidence.join("state/structured-stages");
        let directory = std::fs::read_dir(leaf)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let saved: Value =
            serde_json::from_slice(&std::fs::read(directory.join("ledger.json")).unwrap()).unwrap();
        saved["body"]["attempts_used"].as_u64().unwrap()
    }

    /// `/resume` in a new REPL goes on with the run: the revision the first process started from
    /// is old now, where a new start would have taken it as current and run the tool.
    #[test]
    fn resume_in_a_new_process_goes_on_with_the_run_and_gives_no_attempt_back() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        let mut previous = std::process::Command::new(std::env::current_exe().unwrap());
        previous
            .args([
                "--exact",
                "with_the_strict_isolation::previous_process_of_the_resume_test",
                "--test-threads=1",
            ])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &env.evidence)
            .env(super::REGISTRY_ENV, &env.registry)
            .env(PREVIOUS_ROOT, &env.root)
            .env(PREVIOUS_EVIDENCE, &env.evidence);
        // `status` waits for it, so nothing of it is left to stop.
        assert!(previous.status().unwrap().success());
        assert_eq!(saved_attempts(&env), 1);

        install_sandbox(&env);
        let plans = env.root.join(".anvil/plans");
        std::fs::create_dir_all(&plans).unwrap();
        std::fs::write(
            plans.join("recovery.yaml"),
            format!(
                "goal: \"finish\"\nprofile: \"{WALK_PROFILE}\"\nphases:\n  - id: \"finish\"\n    prompt: \"Submit the upper stage.\"\n  - id: \"after\"\n    prompt: \"Submit the lower stage.\"\n"
            ),
        )
        .unwrap();
        let phase_plan = json!({"goal": "finish", "steps": [{
            "id": "step-setup",
            "kind": "setup",
            "expected_result": "pass",
            "instruction": "Submit the upper stage",
            "expected_paths": [],
            "verify": [],
        }]});
        let responder: Responder = Arc::new(move |_, body| {
            if !body["tools"].to_string().contains(UPPER_TOOL) {
                // A phase asks for its plan first, and that request offers no tool.
                return text_reply(&phase_plan.to_string());
            }
            if !has_tool_answer(body) {
                return tool_reply(UPPER_TOOL, "good", "initial");
            }
            text_reply("stopping here")
        });
        let http = FakeHttp::start(responder);
        let config = config(&env, WALK_PROFILE, &http.url);
        let mut planner = client_from_config(&config, true).unwrap();
        let mut execution = client_from_config(&config, false).unwrap();
        let result = handle_command(
            "/resume .anvil/plans/recovery.yaml",
            &config,
            planner.as_mut(),
            execution.as_mut(),
            &NOOP_UI,
        );
        let requests = http.finish();
        reset_for_test();

        let first = requests
            .iter()
            .find(|body| has_tool_answer(body))
            .map(last_tool_answer)
            .unwrap_or_else(|| panic!("no tool answer: {result:?}"));
        assert_eq!(first["status"], "stale", "{first}");
        assert_ne!(first["revision"], "initial", "{first}");
        assert_eq!(
            saved_attempts(&env),
            1,
            "no attempt came back and none was spent"
        );
        assert!(
            result.is_err(),
            "the obligations are still open: {result:?}"
        );
    }

    #[test]
    fn a_session_with_no_os_boundary_gets_no_stages_and_no_provider_call() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        reset_for_test();
        let (requests, result) = run(
            &env,
            WALK_PROFILE,
            Arc::new(|_, _| text_reply("a reply nobody asked for")),
        );
        let error = result.unwrap_err();
        assert!(error.contains("stage_contract_refused"), "{error}");
        assert!(error.contains("OsSandboxNotEnforced"), "{error}");
        assert!(requests.is_empty());
        assert!(!env.evidence.join("state/structured-stages").exists());
    }

    /// Admitting opens nothing: a preflight takes no lock and leaves no file.
    #[test]
    fn admitting_a_stage_set_touches_no_ledger() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        assert!(
            admit_selected(&env.root, WALK_PROFILE, None)
                .unwrap()
                .is_some()
        );
        assert!(!env.evidence.join("state").exists());
        assert!(!env.root.join(UPPER_OUTPUT).exists());
    }

    #[test]
    fn a_stage_tool_must_take_the_revision_argument() {
        let _guard = lock();
        let env = Env::new();
        let schema = json!({
            "protocol": "profile-tool.schema/v1",
            "root": {
                "type": "object",
                "additionalProperties": false,
                "required": ["mode"],
                "properties": {"mode": {"type": "string"}},
            },
        });
        env.write_tool(
            (BARE_TOOL_ID, "ProposeBare", UPPER_OUTPUT),
            &schema,
            Some(floor()),
        );
        let error = env.admission_error(NO_ARGUMENT_PROFILE);
        assert!(error.contains("stage_contract_refused"), "{error}");
        assert!(error.contains("expected_revision"), "{error}");
    }
}
