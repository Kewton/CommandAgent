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
        BARE_TOOL_ID, CANDIDATE, Env, FIXED_INPUT, LOWER_OUTPUT, LOWER_TOOL, MARKER_NAME_PROFILE,
        NO_ARGUMENT_PROFILE, UPPER_OUTPUT, UPPER_TOOL, WALK_PROFILE, floor, lock,
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
                    "arguments": json!({"file_path": path}).to_string(),
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

    /// T-B (B2): an investigation ends at its own adjudication and has no final acceptance.
    #[test]
    fn an_investigation_that_leaves_a_stage_open_is_no_success() {
        let _guard = lock();
        let env = Env::new();
        env.write_walk_tools(Some(floor()));
        install_sandbox(&env);
        std::fs::create_dir_all(env.root.join("output")).unwrap();
        std::fs::write(env.root.join("output/diagnosis.md"), "# Diagnosis\n").unwrap();
        let reproduce = json!({"goal": "reproduce", "steps": [{
            "id": "step-reproduce",
            "kind": "verify",
            "expected_result": "fail",
            "instruction": "Reproduce the failure",
            "expected_paths": [],
            "verify": ["false"],
        }]});
        let report = json!({"goal": "report", "steps": [{
            "id": "step-setup",
            "kind": "setup",
            "expected_result": "pass",
            "instruction": "Submit the upper stage",
            "expected_paths": [],
            "verify": [],
        }]});
        let responder: Responder = Arc::new(move |_, body| {
            if !body["tools"].to_string().contains(UPPER_TOOL) {
                let wanted = if body.to_string().contains("Phase-report") {
                    &report
                } else {
                    &reproduce
                };
                return text_reply(&wanted.to_string());
            }
            if !has_tool_answer(body) {
                return tool_reply(UPPER_TOOL, "good", "initial");
            }
            text_reply("done")
        });
        let http = FakeHttp::start(responder);
        let mut config = config(&env, WALK_PROFILE, &http.url);
        config.intent_override = Some(IntentId::Investigate);
        let mut planner = client_from_config(&config, true).unwrap();
        let mut execution = client_from_config(&config, false).unwrap();
        let plan = UltraPlan {
            goal: "Find out why it fails".to_string(),
            profile: WALK_PROFILE.to_string(),
            style: "default".to_string(),
            intent: "investigate".to_string(),
            phases: vec![
                UltraPhase {
                    id: "reproduce".to_string(),
                    prompt: "Phase-reproduce: reproduce the failure.".to_string(),
                },
                UltraPhase {
                    id: "report".to_string(),
                    prompt: "Phase-report: submit the upper stage.".to_string(),
                },
            ],
        };
        let result = run_ultra_plan(planner.as_mut(), execution.as_mut(), &plan, &config);
        http.finish();
        reset_for_test();
        let error = result.expect_err("the lower stage was never submitted");
        assert!(error.to_string().contains("stage_incomplete"), "{error:#}");
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
