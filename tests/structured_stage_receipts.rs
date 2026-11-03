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
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
        let both = [UPPER_TOOL_ID, LOWER_TOOL_ID];
        let walk = [("upper", UPPER_TOOL_ID), ("lower", LOWER_TOOL_ID)];
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
    });
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
    use commandagent::providers::client_from_config;
    use commandagent::state::SessionSnapshot;
    use commandagent::tools::os_sandbox::{
        SandboxAvailability, SandboxMode, SandboxPolicy, install_for_test, reset_for_test,
    };
    use commandagent::tools::profile_tools::admit_selected;
    use commandagent::tui::NOOP_UI;
    use serde_json::{Value, json};

    use super::{
        BARE_TOOL_ID, CANDIDATE, Env, FIXED_INPUT, LOWER_OUTPUT, LOWER_TOOL, NO_ARGUMENT_PROFILE,
        UPPER_OUTPUT, UPPER_TOOL, WALK_PROFILE, floor, lock,
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
