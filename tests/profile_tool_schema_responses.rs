//! Issue #1005: the Responses off recording for the profile-tool-schema corpus.
//!
//! A profile tool with no schema (G2 not selected) runs on the Responses API through the
//! OpenAI-compatible entry, against a disposable fake on 127.0.0.1. No model, key, external
//! provider, or Jev call is used. `responses_off::records_the_responses_off_golden` writes the
//! recording only when `RESPONSES_OFF_OUT` names a file, and does nothing otherwise.
//!
//! `responses_off` is the recorder's own code, unchanged; its comparison test reads the golden
//! recorded with it on the base. `g2` runs the schema tools on the Responses API, each against the
//! same calls on Chat Completions as the control.

use std::sync::{Mutex, MutexGuard};

use sha2::{Digest, Sha256};

#[path = "support/responses_schema_fixture.rs"]
mod responses_schema_fixture;

const REGISTRY_ENV: &str = "COMMANDAGENT_ADAPTER_CONFIG_ROOT";

static LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Everything the Responses off recording needs, in one place: the paths, the fake and its
/// replies, the observation, and the normalization.
mod responses_off {
    use std::collections::{BTreeMap, VecDeque};
    use std::io::{Read as _, Write as _};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use commandagent::config::{
        Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
        PromptLayout, Provider,
    };
    use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
    use commandagent::providers::client_from_config;
    use commandagent::sensitive_data::{SecretCatalog, install_scope};
    use commandagent::state::SessionSnapshot;
    use commandagent::tools::profile_tools::{
        ProfileToolReadExecutor, ReadToolOutput, register_read_executor,
    };
    use commandagent::tui::NOOP_UI;
    use serde_json::{Value, json};

    use super::{REGISTRY_ENV, lock, sha256};

    // -----------------------------------------------------------------------------------------
    // The corpus's own synthetic inputs (profile-tool-schema: the flat extension off row).
    // -----------------------------------------------------------------------------------------

    pub(super) const SOURCE: &str = "9b4197a5cf624d4f2470eecdd6407643c78d31ab";
    const DUMMY_SECRET: &str = "fake-dummy-credential-973-not-a-real-key";
    const FLAT_PROFILE: &str = "schema-off-flat";
    const FLAT_TOOL: &str = "off-flat-read";
    const FLAT_RUNTIME: &str = "off-flat-runtime";
    const FLAT_NAME: &str = "ReadFlatConfig";
    const FLAT_PARAMETERS: &str = r#"[{"name":"path","kind":"workspace_path","required":true,"description":"Workspace-relative file."}]"#;
    const FLAT_ARGUMENTS: &str = r#"{"path":"settings.json"}"#;
    const FLAT_PROMPT: &str = "Read the settings.";

    /// The manifest of the off rows' flat profile, as the corpus's tests write it.
    fn flat_manifest() -> String {
        format!(
            "[metadata]\nid = \"{FLAT_PROFILE}\"\ndisplay_name = \"Schema {FLAT_PROFILE}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [\"Read\", \"Write\"]\ntools = [\n  {{ id = \"{FLAT_TOOL}\", effect = \"read\" }},\n]\n"
        )
    }

    // -----------------------------------------------------------------------------------------
    // The dummy values the replies carry. None is a real secret.
    // -----------------------------------------------------------------------------------------

    const SUMMARY: &str =
        "dummy-summary-SECRET-sk-test-0000 fake-dummy-credential-973-not-a-real-key";
    const ENCRYPTED: &str = "opaque-dummy-encrypted-content-0000";
    const PREAMBLE: &str = "Reading the settings first. fake-dummy-credential-973-not-a-real-key";
    const UNKNOWN_VALUE: &str =
        "dummy-unknown-SECRET-0000 fake-dummy-credential-973-not-a-real-key";
    const SAME_TEXT: &str = "Reading the settings.";

    fn response(id: &str, output: Vec<Value>) -> Value {
        json!({
            "id": id,
            "object": "response",
            "model": "m",
            "status": "completed",
            "output": output,
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15},
        })
    }

    fn message(id: &str, text: &str) -> Value {
        json!({
            "type": "message",
            "id": id,
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": text, "annotations": []}],
        })
    }

    fn function_call(item_id: &str, call_id: &str, name: &str, arguments: &str) -> Value {
        json!({
            "type": "function_call",
            "id": item_id,
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
            "status": "completed",
        })
    }

    fn reasoning(id: &str) -> Value {
        json!({
            "type": "reasoning",
            "id": id,
            "summary": [{"type": "summary_text", "text": SUMMARY}],
            "encrypted_content": ENCRYPTED,
        })
    }

    fn unknown(id: &str) -> Value {
        json!({
            "type": "dummy_unknown_item",
            "id": id,
            "dummy_field": UNKNOWN_VALUE,
            "dummy_object": {"dummy_nested": "dummy-unknown-nested-0000"},
        })
    }

    fn done(id: &str) -> Value {
        response(id, vec![message("msg_done", "done")])
    }

    fn flat_call(item_id: &str, call_id: &str) -> Value {
        function_call(item_id, call_id, FLAT_NAME, FLAT_ARGUMENTS)
    }

    struct ResponsesPath {
        id: &'static str,
        profile: &'static str,
        prompt: &'static str,
        replies: fn() -> Vec<Value>,
    }

    /// Each path's item.id and call_id differ (`fc_item_N` / `call_N`).
    const PATHS: [ResponsesPath; 6] = [
        ResponsesPath {
            id: "round-trip",
            profile: FLAT_PROFILE,
            prompt: FLAT_PROMPT,
            replies: || {
                vec![
                    response("resp_1", vec![flat_call("fc_item_1", "call_1")]),
                    done("resp_2"),
                ]
            },
        },
        ResponsesPath {
            id: "text-and-call",
            profile: FLAT_PROFILE,
            prompt: FLAT_PROMPT,
            replies: || {
                vec![
                    response(
                        "resp_1",
                        vec![message("msg_1", PREAMBLE), flat_call("fc_item_1", "call_1")],
                    ),
                    done("resp_2"),
                ]
            },
        },
        ResponsesPath {
            id: "reasoning-item",
            profile: FLAT_PROFILE,
            prompt: FLAT_PROMPT,
            replies: || {
                vec![
                    response(
                        "resp_1",
                        vec![reasoning("rs_1"), flat_call("fc_item_1", "call_1")],
                    ),
                    done("resp_2"),
                ]
            },
        },
        ResponsesPath {
            id: "unknown-item",
            profile: FLAT_PROFILE,
            prompt: FLAT_PROMPT,
            replies: || {
                vec![
                    response(
                        "resp_1",
                        vec![unknown("unk_1"), flat_call("fc_item_1", "call_1")],
                    ),
                    done("resp_2"),
                ]
            },
        },
        ResponsesPath {
            id: "all-items-in-one-response",
            profile: FLAT_PROFILE,
            prompt: FLAT_PROMPT,
            replies: || {
                vec![
                    response(
                        "resp_1",
                        vec![
                            reasoning("rs_1"),
                            message("msg_1", PREAMBLE),
                            unknown("unk_1"),
                            flat_call("fc_item_1", "call_1"),
                        ],
                    ),
                    done("resp_2"),
                ]
            },
        },
        ResponsesPath {
            id: "same-text-two-call-ids",
            profile: FLAT_PROFILE,
            prompt: FLAT_PROMPT,
            replies: || {
                vec![
                    response(
                        "resp_1",
                        vec![
                            reasoning("rs_1"),
                            message("msg_1", SAME_TEXT),
                            flat_call("fc_item_1", "call_1"),
                        ],
                    ),
                    response(
                        "resp_2",
                        vec![
                            reasoning("rs_2"),
                            message("msg_2", SAME_TEXT),
                            flat_call("fc_item_2", "call_2"),
                        ],
                    ),
                    done("resp_3"),
                ]
            },
        },
    ];

    // -----------------------------------------------------------------------------------------
    // The operator side: the flat executor, its descriptor, and the extension root.
    // -----------------------------------------------------------------------------------------

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

    fn register_extension(root: &Path) {
        let directory = root.join(format!("profiles/{FLAT_PROFILE}"));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("manifest.toml"), flat_manifest()).unwrap();
        register_read_executor(FLAT_RUNTIME, Arc::new(FlatReader));
        commandagent::planner::extension_profiles::register(root).unwrap();
    }

    fn write_flat_registry(registry: &Path) {
        let directory = registry.join("profile-tools");
        std::fs::create_dir_all(&directory).unwrap();
        let descriptor = json!({
            "protocol": "profile-tool.descriptor/v1",
            "tool_id": FLAT_TOOL,
            "name": FLAT_NAME,
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
    }

    fn set_env(name: &str, value: Option<&std::ffi::OsStr>) {
        // SAFETY: every env mutation in this binary holds `LOCK`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // The fake provider: 127.0.0.1, one scripted Responses reply per request, bodies kept.
    // -----------------------------------------------------------------------------------------

    struct FakeHttp {
        url: String,
        requests: Arc<Mutex<Vec<String>>>,
        replies: Arc<Mutex<VecDeque<String>>>,
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
                let (requests, replies, stop) = (requests.clone(), replies.clone(), stop.clone());
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
        // Only the body is kept; no header (Authorization among them) is recorded.
        let body = String::from_utf8_lossy(&bytes[body_start..]).to_string();
        requests.lock().unwrap().push(body);
        let reply = replies.lock().unwrap().pop_front().unwrap_or_else(|| {
            response(
                "resp_exhausted",
                vec![message("msg_exhausted", "script exhausted")],
            )
            .to_string()
        });
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
            reply.len()
        );
        let _ = stream.write_all(response.as_bytes());
    }

    // -----------------------------------------------------------------------------------------
    // The configuration: the off rows' fields, with the executor on the OpenAI-compatible
    // Responses entry pointed at the fake.
    // -----------------------------------------------------------------------------------------

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

    /// `--provider openai-compatible --base-url <fake>/v1 --api responses`, parsed the way the
    /// binary parses it. The caller has pointed HOME at an empty temporary directory.
    fn responses_config(root: &Path, evidence: &Path, profile: &str, url: &str) -> Config {
        let base_url = format!("{url}/v1");
        let parsed = commandagent::provider_cli::parse_from([
            "commandagent",
            "--cwd",
            root.to_str().unwrap(),
            "--state-dir",
            evidence.join("state").to_str().unwrap(),
            "--model",
            "m",
            "--planner-model",
            "m",
            "--provider",
            "openai-compatible",
            "--base-url",
            &base_url,
            "--api",
            "responses",
        ])
        .unwrap();
        let parsed =
            Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options).unwrap();
        let mut config = base_config(root, evidence, profile, url);
        config.provider = parsed.provider;
        config.openai_api = parsed.openai_api;
        config.openai_compatible = parsed.openai_compatible;
        assert_eq!(config.openai_api, OpenAiApi::Responses);
        assert!(config.openai_compatible.is_some());
        config
    }

    // -----------------------------------------------------------------------------------------
    // The normalization: exactly the rules in the recording's `normalization`.
    // -----------------------------------------------------------------------------------------

    pub(super) const NORMALIZATION: [&str; 6] = [
        "temporary roots (workspace, evidence, registry, extension root, HOME; canonical and as created; longest first) -> <ROOT>",
        "the fake's loopback URL http://127.0.0.1:<port>, then 127.0.0.1:<port> -> <LOOPBACK>",
        "a JSON number under epoch, occurred_at_epoch_ms, duration_ms, elapsed_ms, estimated_prompt_tokens_sent, estimated_stable_prefix_chars -> \"<N>\" (the existing off comparison's NORMALIZED_NUMBER_FIELDS)",
        "inside a string, epoch|occurred_at_epoch_ms|duration_ms|elapsed_ms then [\"]? spaces [:=] spaces digits -> the prefix and <TIME>; an ISO 8601 date-time inside a string -> <TIME>",
        "the string value of run_nonce or process_nonce, wherever it appears -> <NONCE-n>; a UUID -> <UUID-n>; n by first appearance within a path",
        "a revision's 16 lowercase hex digits as a whole word -> <REVISION-n>; n by first appearance within a path",
    ];

    /// The same keys as the existing off comparison (`tests/structured_stage_receipts.rs`).
    const NORMALIZED_NUMBER_FIELDS: [&str; 6] = [
        "epoch",
        "occurred_at_epoch_ms",
        "duration_ms",
        "elapsed_ms",
        "estimated_prompt_tokens_sent",
        "estimated_stable_prefix_chars",
    ];
    const NONCE_KEYS: [&str; 2] = ["run_nonce", "process_nonce"];

    pub(super) struct Normalizer {
        url: String,
        host: String,
        roots: Vec<String>,
        nonces: Vec<String>,
        uuids: Vec<String>,
        revisions: Vec<String>,
        section: String,
        /// Replacements per `<top-level observation key>/<kind>`.
        pub(super) counts: BTreeMap<String, usize>,
    }

    impl Normalizer {
        fn new(url: &str, roots: &[PathBuf]) -> Self {
            let mut spelled = Vec::new();
            for root in roots {
                spelled.push(root.display().to_string());
                if let Ok(canonical) = std::fs::canonicalize(root) {
                    spelled.push(canonical.display().to_string());
                }
            }
            spelled.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
            spelled.dedup();
            Self {
                url: url.to_string(),
                host: url.trim_start_matches("http://").to_string(),
                roots: spelled,
                nonces: Vec::new(),
                uuids: Vec::new(),
                revisions: Vec::new(),
                section: String::new(),
                counts: BTreeMap::new(),
            }
        }

        fn count(&mut self, kind: &'static str, times: usize) {
            if times > 0 {
                *self
                    .counts
                    .entry(format!("{}/{kind}", self.section))
                    .or_default() += times;
            }
        }

        fn collect_nonces(&mut self, value: &Value) {
            match value {
                Value::Object(object) => {
                    for (key, value) in object {
                        if NONCE_KEYS.contains(&key.as_str())
                            && let Some(text) = value.as_str()
                            && !self.nonces.iter().any(|known| known == text)
                        {
                            self.nonces.push(text.to_string());
                        }
                        self.collect_nonces(value);
                    }
                }
                Value::Array(items) => items.iter().for_each(|item| self.collect_nonces(item)),
                _ => {}
            }
        }

        fn value(&mut self, value: &Value) -> Value {
            match value {
                Value::Object(object) => Value::Object(
                    object
                        .iter()
                        .map(|(key, value)| {
                            let value = if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str())
                                && value.is_number()
                            {
                                self.count("number", 1);
                                json!("<N>")
                            } else {
                                self.value(value)
                            };
                            (key.clone(), value)
                        })
                        .collect(),
                ),
                Value::Array(items) => {
                    Value::Array(items.iter().map(|item| self.value(item)).collect())
                }
                Value::String(text) => Value::String(self.text(text)),
                other => other.clone(),
            }
        }

        fn numbered(
            list: &mut Vec<String>,
            prefix: &str,
            regex: &regex::Regex,
            text: &str,
        ) -> (String, usize) {
            let mut times = 0;
            let replaced = regex
                .replace_all(text, |captures: &regex::Captures<'_>| {
                    let found = captures[0].to_string();
                    let index = match list.iter().position(|known| *known == found) {
                        Some(index) => index,
                        None => {
                            list.push(found);
                            list.len() - 1
                        }
                    };
                    times += 1;
                    format!("<{prefix}-{}>", index + 1)
                })
                .into_owned();
            (replaced, times)
        }

        fn text(&mut self, text: &str) -> String {
            let mut text = text.to_string();
            let times = text.matches(&self.url).count();
            text = text.replace(&self.url, "<LOOPBACK>");
            let host_times = text.matches(&self.host).count();
            text = text.replace(&self.host, "<LOOPBACK>");
            self.count("loopback", times + host_times);
            for index in 0..self.roots.len() {
                let root = self.roots[index].clone();
                let times = text.matches(&root).count();
                text = text.replace(&root, "<ROOT>");
                self.count("root", times);
            }
            for index in 0..self.nonces.len() {
                let nonce = self.nonces[index].clone();
                let times = text.matches(&nonce).count();
                text = text.replace(&nonce, &format!("<NONCE-{}>", index + 1));
                self.count("nonce", times);
            }
            let iso = regex::Regex::new(
                r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})?",
            )
            .unwrap();
            let times = iso.find_iter(&text).count();
            text = iso.replace_all(&text, "<TIME>").into_owned();
            self.count("time", times);
            let named = regex::Regex::new(
                r#"(epoch|occurred_at_epoch_ms|duration_ms|elapsed_ms)("? *[:=] *)[0-9]+"#,
            )
            .unwrap();
            let times = named.find_iter(&text).count();
            text = named.replace_all(&text, "${1}${2}<TIME>").into_owned();
            self.count("time", times);
            let uuid = regex::Regex::new(
                r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
            )
            .unwrap();
            let (replaced, times) = Self::numbered(&mut self.uuids, "UUID", &uuid, &text);
            text = replaced;
            self.count("uuid", times);
            let revision = regex::Regex::new(r"\b[0-9a-f]{16}\b").unwrap();
            let (replaced, times) =
                Self::numbered(&mut self.revisions, "REVISION", &revision, &text);
            text = replaced;
            self.count("revision", times);
            text
        }

        /// The same walk as `value`, with each top-level key named in the counts.
        pub(super) fn normalize(&mut self, value: &Value) -> Value {
            self.collect_nonces(value);
            let Value::Object(object) = value else {
                return self.value(value);
            };
            let mut normalized = serde_json::Map::new();
            for (key, value) in object {
                self.section = key.clone();
                let value = if NORMALIZED_NUMBER_FIELDS.contains(&key.as_str()) && value.is_number()
                {
                    self.count("number", 1);
                    json!("<N>")
                } else {
                    self.value(value)
                };
                normalized.insert(key.clone(), value);
            }
            Value::Object(normalized)
        }
    }

    // -----------------------------------------------------------------------------------------
    // The observation of one path.
    // -----------------------------------------------------------------------------------------

    fn files_under(root: &Path, skip: &[&str]) -> BTreeMap<String, String> {
        let mut files = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(directory) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let relative = path.strip_prefix(root).unwrap().display().to_string();
                    if skip.contains(&relative.as_str()) {
                        continue;
                    }
                    let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap()).to_string();
                    files.insert(relative, text);
                }
            }
        }
        files
    }

    fn as_json(text: &str) -> Value {
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
    }

    /// The raw observation of one path, then the same value normalized, and what was replaced.
    fn observe(path: &ResponsesPath) -> (Value, BTreeMap<String, usize>) {
        let home = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let evidence_dir = tempfile::tempdir().unwrap();
        let registry = tempfile::tempdir().unwrap();
        let extension = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        let evidence = std::fs::canonicalize(evidence_dir.path()).unwrap();
        std::fs::write(root.join("settings.json"), "{\"mode\":\"fast\"}").unwrap();
        register_extension(&std::fs::canonicalize(extension.path()).unwrap());
        write_flat_registry(registry.path());

        let saved_home = std::env::var_os("HOME");
        set_env("HOME", Some(home.path().as_os_str()));
        set_env(
            "COMMANDAGENT_COMPATIBLE_REASONING_EFFORT",
            None::<&std::ffi::OsStr>,
        );
        set_env(
            REGISTRY_ENV,
            (path.profile == FLAT_PROFILE).then(|| registry.path().as_os_str()),
        );

        let replies = (path.replies)();
        let http = FakeHttp::start(replies.iter().map(Value::to_string).collect());
        let url = http.url.clone();
        let config = responses_config(&root, &evidence, path.profile, &url);
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
            .unwrap();
        install_scope(catalog, Some(&root), config.eval_events_path.as_deref());
        let mut client = client_from_config(&config, false).unwrap();
        let mut snapshot = SessionSnapshot::new();
        let outcome = run_session_with_outcome_with_ui(
            client.as_mut(),
            &mut snapshot,
            path.prompt,
            &[],
            &config,
            &NOOP_UI,
        );
        drop(client);
        let (requests, unused) = http.finish();
        set_env(REGISTRY_ENV, None::<&std::ffi::OsStr>);
        set_env("HOME", saved_home.as_deref());

        let outcome = match outcome {
            Ok(outcome) => json!({"ok": {
                "stop_reason": format!("{:?}", outcome.stop_reason),
                "final_text": outcome.final_text,
                "iterations": outcome.iterations,
                "tool_calls": outcome.tool_calls,
                "changed_paths": outcome.changed_paths,
            }}),
            Err(error) => json!({"err": format!("{error:#}")}),
        };
        let events = serde_json::Deserializer::from_str(
            &std::fs::read_to_string(evidence.join("events.jsonl")).unwrap_or_default(),
        )
        .into_iter::<Value>()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
        let event_names = events
            .iter()
            .map(|event| event["event"].clone())
            .collect::<Vec<_>>();
        let raw = json!({
            "replies": replies,
            "requests": requests.iter().map(|body| as_json(body)).collect::<Vec<_>>(),
            "request_count": requests.len(),
            "unused_replies": unused,
            "event_names": event_names,
            "events": events,
            "history": serde_json::to_value(&snapshot).unwrap(),
            "state_files": files_under(&evidence, &["events.jsonl"])
                .into_iter()
                .map(|(name, text)| (name, as_json(&text)))
                .collect::<BTreeMap<_, _>>(),
            "artifacts": files_under(&root, &[]),
            "outcome": outcome,
        });
        let roots = [
            home.path().to_path_buf(),
            workspace.path().to_path_buf(),
            evidence_dir.path().to_path_buf(),
            registry.path().to_path_buf(),
            extension.path().to_path_buf(),
        ];
        let mut normalizer = Normalizer::new(&url, &roots);
        let normalized = normalizer.normalize(&raw);
        (normalized, normalizer.counts)
    }

    /// The whole recording, and the replacements per path.
    pub(super) fn golden() -> (Value, Value) {
        let mut paths = Vec::new();
        let mut counts = serde_json::Map::new();
        for path in &PATHS {
            let (observation, replaced) = observe(path);
            counts.insert(path.id.to_string(), json!(replaced));
            paths.push(json!({"id": path.id, "observation": observation}));
        }
        let document = json!({
            "issue": "#1005",
            "kind": "responses-off",
            "source": SOURCE,
            "normalization": NORMALIZATION,
            "paths": paths,
        });
        (document, Value::Object(counts))
    }

    #[test]
    fn records_the_responses_off_golden() {
        let Some(out) = std::env::var_os("RESPONSES_OFF_OUT") else {
            return;
        };
        let _guard = lock();
        let (document, counts) = golden();
        let out = PathBuf::from(out);
        std::fs::write(
            &out,
            format!("{}\n", serde_json::to_string_pretty(&document).unwrap()),
        )
        .unwrap();
        let mut counts_path = out.into_os_string();
        counts_path.push(".counts.json");
        std::fs::write(
            counts_path,
            format!("{}\n", serde_json::to_string_pretty(&counts).unwrap()),
        )
        .unwrap();
    }
}

/// The golden recorded on the base with `responses_off`, compared with what the same code
/// observes now. Nothing is normalized here beyond the recorder's own rules.
mod responses_off_comparison {
    use serde_json::Value;

    use super::responses_off::{SOURCE, golden};
    use super::{lock, sha256};

    const GOLDEN: &str =
        include_str!("corpus/apps/profile-tool-schema/fixtures/responses-off-golden.json");
    const MANIFEST: &str = include_str!("corpus/apps/profile-tool-schema/source-manifest.json");

    #[test]
    fn the_responses_off_paths_match_the_base_recording_byte_for_byte() {
        let _guard = lock();
        let recorded: Value = serde_json::from_str(GOLDEN).unwrap();
        let (now, _) = golden();
        let (old_paths, new_paths) = (
            recorded["paths"].as_array().unwrap(),
            now["paths"].as_array().unwrap(),
        );
        assert_eq!(new_paths.len(), old_paths.len());
        for (new, old) in new_paths.iter().zip(old_paths) {
            assert_eq!(new["id"], old["id"]);
            for (key, value) in old["observation"].as_object().unwrap() {
                assert_eq!(&new["observation"][key], value, "{} {key}", old["id"]);
            }
            assert_eq!(new["observation"], old["observation"], "{}", old["id"]);
        }
        let text = format!("{}\n", serde_json::to_string_pretty(&now).unwrap());
        assert!(text == GOLDEN, "the bytes differ from the base recording");
    }

    #[test]
    fn the_manifest_pins_the_responses_off_golden_and_its_source() {
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(manifest["responses_off_base_sha"], SOURCE);
        assert_eq!(
            manifest["fixture_sha256"]["responses-off-golden.json"],
            sha256(GOLDEN.as_bytes())
        );
        let recorded: Value = serde_json::from_str(GOLDEN).unwrap();
        assert_eq!(recorded["source"], SOURCE);
        assert_eq!(recorded["kind"], "responses-off");
    }
}

/// Issue #1005: schema tools on the Responses API. Every run here goes through the real LM
/// Studio client (the shared Responses transport) against a disposable fake on 127.0.0.1, and
/// the expectation of each is the same calls on Chat Completions, run the same way, as the
/// control. No model, key, external provider, or Jev call is used.
mod g2 {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, OnceLock};

    use commandagent::config::{
        Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, PlanPreset, PromptLayout,
        Provider,
    };
    use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
    use commandagent::providers::client_from_config;
    use commandagent::sensitive_data::{SecretCatalog, install_scope, set_current};
    use commandagent::state::SessionSnapshot;
    use commandagent::tools::profile_tools::{
        ProfileToolReadExecutor, ProfileToolSchemaCompiler, ReadToolOutput, SchemaCompileInput,
        admit_selected, register_read_executor, register_schema_compiler,
    };
    use commandagent::tui::NOOP_UI;
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::responses_schema_fixture::{
        APIS, Api, FakeHttp, Responder, function_call, holds_anywhere, message, reasoning,
        responses_reply, scripted, tool_outputs, unknown, view,
    };
    use super::{REGISTRY_ENV, lock, sha256};

    const CASES: &str = include_str!("corpus/apps/profile-tool-schema/fixtures/cases.json");
    const RESPONSES_CASES: &str =
        include_str!("corpus/apps/profile-tool-schema/fixtures/responses-cases.json");
    const MANIFEST: &str = include_str!("corpus/apps/profile-tool-schema/source-manifest.json");
    const DUMMY_SECRET: &str = "fake-dummy-credential-973-not-a-real-key";
    const CONFIG_PROFILE: &str = "schema-config";
    const INDEX_PROFILE: &str = "schema-index";
    const CONFIG_TOOL_ID: &str = "config-submit";
    const INDEX_TOOL_ID: &str = "index-submit";
    const CONFIG_TOOL: &str = "SubmitConfig";
    const INDEX_TOOL: &str = "SubmitIndex";
    const RUNTIME: &str = "schema-responses-runtime";
    const COMPILER: &str = "schema-responses-compiler";
    const ACCEPTED: &str = "accepted by the fake";
    const PROMPT: &str = "Submit the object.";
    const VALID_CONFIG: &str = r#"{"mode":"fast","limits":{"retries":3,"ratio":0.5},"targets":[{"host":"ab","port":1},null],"name":"n","tool":"t","path":"a/../b","arguments":{"name":"inner","args":"it's"},"enabled":false}"#;
    const VALID_INDEX: &str =
        r#"{"documents":[{"id":"doc-a","refs":["doc-b"]},{"id":"doc-b","refs":[]}]}"#;
    const INVALID_CONFIG: &str = r#"{"mode":"slow","limits":{"retries":1},"targets":[null]}"#;

    fn set_env(name: &str, value: Option<&str>) {
        // SAFETY: every env mutation in this binary holds `LOCK`.
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }

    fn parsed(raw: &str) -> Value {
        serde_json::from_str(raw).unwrap()
    }

    fn body(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    // -----------------------------------------------------------------------------------------
    // The operator: a recording executor, compilers, descriptors, and two generic profiles.
    // -----------------------------------------------------------------------------------------

    #[derive(Debug, Default)]
    struct RecordingReader {
        calls: Mutex<Vec<Value>>,
    }

    impl ProfileToolReadExecutor for RecordingReader {
        fn read(&self, _root: &Path, arguments: &Value) -> anyhow::Result<ReadToolOutput> {
            self.calls.lock().unwrap().push(arguments.clone());
            Ok(ReadToolOutput {
                text: ACCEPTED.to_string(),
                usage: None,
            })
        }
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

    fn schema_declaration(root: Value) -> Value {
        json!({"protocol": "profile-tool.schema/v1", "root": root})
    }

    fn write_descriptor(registry: &Path, tool: (&str, &str), schema: &Value, version: &str) {
        let (tool_id, name) = tool;
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
            "requires": [],
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

    fn profile_manifest(profile: &str, tool_id: &str) -> String {
        format!(
            "[metadata]\nid = \"{profile}\"\ndisplay_name = \"Schema {profile}\"\nschema_version = \"v2\"\ntask_family = \"generic\"\n[plan]\nintent = \"create\"\nphases = [\n  {{ id = \"implementation\", prompt = \"Write the notes for {{goal}}.\" }},\n]\n[artifacts]\nrequired = [\"notes.txt\"]\n[guidance.variants.contract]\ntriggers = [{{ condition = \"always\" }}]\nmessages = {{ instruction = \"Write notes.txt.\" }}\n[[checks.final]]\nid = \"scaffold_files_present\"\nparams = {{ files = [\"notes.txt\"] }}\n[profile_tools]\nschema_version = \"profile-tool.declaration/v1\"\nbuiltin_tools = [\"Read\"]\ntools = [\n  {{ id = \"{tool_id}\", effect = \"read\" }},\n]\n"
        )
    }

    fn extension() -> &'static Path {
        static ROOT: OnceLock<PathBuf> = OnceLock::new();
        ROOT.get_or_init(|| {
            let root = std::fs::canonicalize(tempfile::tempdir().unwrap().keep()).unwrap();
            for (profile, tool_id) in [
                (CONFIG_PROFILE, CONFIG_TOOL_ID),
                (INDEX_PROFILE, INDEX_TOOL_ID),
            ] {
                let directory = root.join(format!("profiles/{profile}"));
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(
                    directory.join("manifest.toml"),
                    profile_manifest(profile, tool_id),
                )
                .unwrap();
            }
            root
        })
    }

    // -----------------------------------------------------------------------------------------
    // One workspace, evidence directory, and operator registry, and one session on either API.
    // -----------------------------------------------------------------------------------------

    struct Env {
        _workspace: tempfile::TempDir,
        _evidence: tempfile::TempDir,
        registry: tempfile::TempDir,
        root: PathBuf,
        evidence: PathBuf,
    }

    impl Env {
        /// Registers this binary's extension root again, since the off recorder registers its
        /// own, and sets the registry variable; the caller holds `LOCK`.
        fn new() -> Self {
            commandagent::planner::extension_profiles::register(extension()).unwrap();
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

        fn with_config(root: Value) -> Self {
            let env = Self::new();
            env.write_config_tool(schema_declaration(root));
            env
        }

        fn write_config_tool(&self, schema: Value) {
            write_descriptor(
                self.registry.path(),
                (CONFIG_TOOL_ID, CONFIG_TOOL),
                &schema,
                "1.0.0",
            );
        }

        fn write_index_tool(&self) {
            write_descriptor(
                self.registry.path(),
                (INDEX_TOOL_ID, INDEX_TOOL),
                &schema_declaration(index_root()),
                "1.0.0",
            );
        }

        fn config(&self, api: Api, profile: &str, url: &str) -> Config {
            Config {
                workspace_root: self.root.clone(),
                state_dir: self.evidence.join("state"),
                eval_events_path: Some(self.evidence.join("events.jsonl")),
                completion_contract_path: None,
                yes: true,
                offline: false,
                context_budget: 100_000,
                model: "m".to_string(),
                provider: Provider::LmStudio,
                tool_protocol: None,
                openai_api: api.openai_api(),
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

        fn events(&self) -> Vec<Value> {
            std::fs::read_to_string(self.evidence.join("events.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        }

        fn event_names(&self) -> Vec<String> {
            self.events()
                .iter()
                .map(|event| event["event"].as_str().unwrap_or_default().to_string())
                .collect()
        }

        /// The corpus's dummy credential, in this workspace's secret scope.
        fn install_dummy_secret(&self) {
            let mut catalog = SecretCatalog::new();
            catalog
                .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
                .unwrap();
            let events = self.evidence.join("events.jsonl");
            install_scope(catalog, Some(&self.root), Some(&events));
        }
    }

    struct Run {
        executed: Vec<Value>,
        requests: Vec<String>,
        result: Result<String, String>,
        history: Value,
    }

    impl Run {
        fn request(&self, index: usize) -> Value {
            body(&self.requests[index])
        }

        /// The tool outputs of request `index`, each `(call ID, content)`.
        fn outputs(&self, index: usize) -> Vec<(String, String)> {
            tool_outputs(&self.request(index))
        }

        /// The content of the last tool output of request `index`.
        fn last_output(&self, index: usize) -> String {
            self.outputs(index).pop().map(|(_, text)| text).unwrap()
        }
    }

    fn run_profile(env: &Env, api: Api, profile: &str, responder: Responder) -> Run {
        let reader = Arc::new(RecordingReader::default());
        register_read_executor(RUNTIME, reader.clone());
        let http = FakeHttp::start(responder);
        let config = env.config(api, profile, &http.url);
        let mut client = client_from_config(&config, false).unwrap();
        let mut snapshot = SessionSnapshot::new();
        let result = run_session_with_outcome_with_ui(
            client.as_mut(),
            &mut snapshot,
            PROMPT,
            &[],
            &config,
            &NOOP_UI,
        )
        .map(|outcome| outcome.final_text)
        .map_err(|error| format!("{error:#}"));
        drop(client);
        let requests = http.finish();
        let executed = reader.calls.lock().unwrap().clone();
        Run {
            executed,
            requests,
            result,
            history: serde_json::to_value(&snapshot).unwrap(),
        }
    }

    fn session(env: &Env, api: Api, replies: Vec<String>) -> Run {
        run_profile(env, api, CONFIG_PROFILE, scripted(api, replies))
    }

    /// Request `index`'s calls, as the Chat Completions view reads them, in order.
    fn sent_calls(run: &Run, index: usize) -> Vec<Value> {
        view(&run.request(index))
            .iter()
            .flat_map(|message| {
                message["tool_calls"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }

    // -----------------------------------------------------------------------------------------
    // Acceptance 1: the sent schema.
    // -----------------------------------------------------------------------------------------

    /// The corpus's two accepted rows, `config-object-profile` and `document-index-profile`, with
    /// Chat Completions as the control: the same definitions with the same parameters, which are
    /// the adopted schema, and `strict: false` on the schema tool alone.
    #[test]
    fn the_responses_request_carries_the_adopted_schema_with_strict_false_on_the_schema_tool_only()
    {
        let _guard = lock();
        let cases: Value = serde_json::from_str(CASES).unwrap();
        let rows = cases["accepted"].as_array().unwrap();
        assert_eq!(rows[0]["id"], "config-object-profile");
        assert_eq!(rows[0]["raw"], VALID_CONFIG);
        assert_eq!(rows[1]["id"], "document-index-profile");
        assert_eq!(rows[1]["raw"], VALID_INDEX);
        for (row, root) in rows.iter().zip([config_root(), index_root()]) {
            let (profile, tool, raw) = (
                row["profile"].as_str().unwrap(),
                row["tool"].as_str().unwrap(),
                row["raw"].as_str().unwrap(),
            );
            let mut seen = Vec::new();
            for api in APIS {
                let env = Env::new();
                if profile == INDEX_PROFILE {
                    env.write_index_tool();
                } else {
                    env.write_config_tool(schema_declaration(root.clone()));
                }
                let run = run_profile(
                    &env,
                    api,
                    profile,
                    scripted(api, vec![api.calls(&[("call_1", tool, raw)])]),
                );
                assert_eq!(run.result.as_deref(), Ok("done"), "{api:?}");
                assert_eq!(run.executed, vec![parsed(raw)], "{api:?}");
                let admitted = admit_selected(&env.root, profile, None).unwrap().unwrap();
                let binding = &admitted.bindings()[0];
                seen.push((
                    run.request(0)["tools"].as_array().unwrap().clone(),
                    env.event_names(),
                    (
                        binding.input_revision().map(str::to_string),
                        binding.adopted_schema_sha256().map(str::to_string),
                    ),
                ));
            }
            let (chat, responses) = (&seen[0], &seen[1]);
            let chat_definitions = chat
                .0
                .iter()
                .map(|tool| {
                    assert!(
                        tool.get("strict").is_none() && tool["function"].get("strict").is_none()
                    );
                    (
                        tool["function"]["name"].clone(),
                        tool["function"]["parameters"].clone(),
                    )
                })
                .collect::<Vec<_>>();
            let responses_definitions = responses
                .0
                .iter()
                .map(|tool| (tool["name"].clone(), tool["parameters"].clone()))
                .collect::<Vec<_>>();
            assert_eq!(responses_definitions, chat_definitions, "{}", row["id"]);
            for definition in &responses.0 {
                assert_eq!(definition["type"], "function");
                assert!(definition.get("function").is_none(), "{definition}");
                if definition["name"] == tool {
                    assert_eq!(definition["strict"], json!(false), "{definition}");
                    assert_eq!(definition["parameters"], root, "{definition}");
                } else {
                    assert!(definition.get("strict").is_none(), "{definition}");
                }
            }
            // The same events, revision, and adopted schema hash on both wires.
            assert_eq!(responses.1, chat.1, "{}", row["id"]);
            assert_eq!(responses.2, chat.2, "{}", row["id"]);
        }
        set_env(REGISTRY_ENV, None);
    }

    /// A schema the provider sanitizer or the secret scrub would change is refused before any
    /// Responses request, as on Chat Completions.
    #[test]
    fn a_schema_a_shared_step_would_change_is_refused_before_any_responses_request() {
        let _guard = lock();
        for (label, root, kind) in [
            (
                "sanitizer",
                closed(json!({"$id": {"type": "string"}})),
                "schema_unsupported",
            ),
            (
                "scrub",
                closed(json!({DUMMY_SECRET: {"type": "string"}})),
                "schema_scrubbed",
            ),
        ] {
            for api in APIS {
                let env = Env::with_config(root.clone());
                env.install_dummy_secret();
                let run = session(&env, api, Vec::new());
                let error = run.result.unwrap_err();
                assert!(
                    error.contains(&format!("profile_tool_refused: {kind}")),
                    "{label} {api:?}: {error}"
                );
                assert!(run.requests.is_empty(), "{label} {api:?}");
                assert!(run.executed.is_empty(), "{label} {api:?}");
            }
        }
        set_current(None);
        set_env(REGISTRY_ENV, None);
    }

    // -----------------------------------------------------------------------------------------
    // Acceptance 2 and 3: what is adopted, and how a refusal reads.
    // -----------------------------------------------------------------------------------------

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

    fn string_of_bytes(bytes: usize) -> String {
        format!(r#"{{"s":"{}"}}"#, "a".repeat(bytes - r#"{"s":""}"#.len()))
    }

    fn elements(count: usize) -> String {
        format!(r#"{{"a":[{}]}}"#, vec!["1"; count - 2].join(","))
    }

    /// `(label, schema root, raw)`: values at the edges of the limits, which must pass.
    fn at_the_limits() -> Vec<(&'static str, Value, String)> {
        vec![
            ("config object", config_root(), VALID_CONFIG.to_string()),
            ("depth 32", nested_schema(32), nested_raw(32)),
            (
                "64 KiB",
                closed(json!({"s": {"type": "string"}})),
                string_of_bytes(64 * 1024),
            ),
            (
                "512 elements",
                closed(json!({"a": {"type": "array", "items": {"type": "integer"}}})),
                elements(512),
            ),
        ]
    }

    /// `(label, schema root, raw, code)`: the corpus's rejected rows, then one past each limit.
    fn rejected() -> Vec<(String, Value, String, String)> {
        let cases: Value = serde_json::from_str(CASES).unwrap();
        let mut rows = cases["rejected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    row["id"].as_str().unwrap().to_string(),
                    config_root(),
                    row["raw"].as_str().unwrap().to_string(),
                    row["code"].as_str().unwrap().to_string(),
                )
            })
            .collect::<Vec<_>>();
        rows.extend([
            (
                "depth 33".to_string(),
                nested_schema(32),
                nested_raw(33),
                "arguments_too_deep".to_string(),
            ),
            (
                "64 KiB + 1".to_string(),
                closed(json!({"s": {"type": "string"}})),
                string_of_bytes(64 * 1024 + 1),
                "arguments_too_large".to_string(),
            ),
            (
                "513 elements".to_string(),
                closed(json!({"a": {"type": "array", "items": {"type": "integer"}}})),
                elements(513),
                "too_many_elements".to_string(),
            ),
        ]);
        rows
    }

    #[test]
    fn valid_values_and_values_at_the_limits_reach_the_executor_unchanged_as_on_chat() {
        let _guard = lock();
        for (label, root, raw) in at_the_limits() {
            for api in APIS {
                let env = Env::with_config(root.clone());
                let run = session(&env, api, vec![api.calls(&[("call_1", CONFIG_TOOL, &raw)])]);
                assert_eq!(run.result.as_deref(), Ok("done"), "{label} {api:?}");
                assert_eq!(run.executed, vec![parsed(&raw)], "{label} {api:?}");
                assert!(
                    !env.event_names()
                        .iter()
                        .any(|name| name == "tool_args_recovered"),
                    "{label} {api:?}"
                );
            }
        }
        set_env(REGISTRY_ENV, None);
    }

    /// Every rejected row of the corpus, and one past each limit: nothing runs, the run goes on,
    /// the model reads the same refusal as on Chat Completions, the call goes back with `"{}"`,
    /// and the raw text is nowhere in the next request, decoded or not.
    #[test]
    fn every_rejected_argument_runs_nothing_and_reads_as_on_chat() {
        let _guard = lock();
        for (label, root, raw, code) in rejected() {
            let mut answers = Vec::new();
            for api in APIS {
                let env = Env::with_config(root.clone());
                let run = session(&env, api, vec![api.calls(&[("call_1", CONFIG_TOOL, &raw)])]);
                assert!(run.executed.is_empty(), "{label} {api:?}");
                assert_eq!(run.result.as_deref(), Ok("done"), "{label} {api:?}");
                assert_eq!(run.requests.len(), 2, "{label} {api:?}");
                let answer = run.last_output(1);
                assert!(answer.contains("schema_violation"), "{label}: {answer}");
                assert!(
                    answer.contains("\"stage\":\"received_native\""),
                    "{label}: {answer}"
                );
                assert!(answer.contains(&format!("\"code\":\"{code}\"")), "{label}");
                let calls = sent_calls(&run, 1);
                assert_eq!(calls.len(), 1, "{label} {api:?}");
                assert_eq!(calls[0]["function"]["arguments"], "{}", "{label} {api:?}");
                assert!(
                    !holds_anywhere(&run.requests[1], &raw),
                    "{label} {api:?}: the raw arguments came back"
                );
                assert!(run.request(1)["tools"].is_array(), "{label} {api:?}");
                assert!(
                    !env.event_names()
                        .iter()
                        .any(|name| name == "fallback_decision"),
                    "{label} {api:?}"
                );
                answers.push(answer);
            }
            assert_eq!(answers[1], answers[0], "{label}");
        }
        set_env(REGISTRY_ENV, None);
    }

    /// A repeated key anywhere in the body, outside the calls too, refuses every schema call of
    /// that reply, on both wires.
    #[test]
    fn a_repeated_key_outside_the_calls_refuses_every_schema_call_of_the_reply() {
        let _guard = lock();
        let calls = [
            ("call_1", CONFIG_TOOL, VALID_CONFIG),
            ("call_2", CONFIG_TOOL, VALID_CONFIG),
        ];
        for place in ["usage", "reasoning"] {
            let mut answers = Vec::new();
            for api in APIS {
                let clean = api.calls(&calls);
                let repeated = match (api, place) {
                    (_, "usage") => clean.replacen(
                        '{',
                        r#"{"usage":{"total_tokens":1},"usage":{"total_tokens":2},"#,
                        1,
                    ),
                    (Api::Chat, _) => clean.replacen(
                        r#""role":"assistant""#,
                        r#""role":"assistant","reasoning_content":"a","reasoning_content":"b""#,
                        1,
                    ),
                    (Api::Responses, _) => clean.replacen(
                        r#""output":["#,
                        r#""output":[{"type":"reasoning","id":"rs_1","summary":[],"summary":[]},"#,
                        1,
                    ),
                };
                assert_ne!(repeated, clean, "{place} {api:?}");
                let env = Env::with_config(config_root());
                let run = session(&env, api, vec![repeated]);
                assert!(run.executed.is_empty(), "{place} {api:?}");
                let outputs = run.outputs(1);
                assert_eq!(outputs.len(), 2, "{place} {api:?}");
                for (_, answer) in &outputs {
                    assert!(answer.contains("\"code\":\"duplicate_key\""), "{answer}");
                }
                answers.push(outputs);
            }
            assert_eq!(answers[1], answers[0], "{place}");
        }
        set_env(REGISTRY_ENV, None);
    }

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
        env.write_config_tool(json!({
            "protocol": "profile-tool.schema/v1",
            "root": revision_root(),
            "inputs": ["inputs/ids.txt"],
            "compiler": COMPILER,
            "revision_argument": "input_revision",
        }));
        std::fs::create_dir_all(env.root.join("inputs")).unwrap();
        std::fs::write(env.root.join("inputs/ids.txt"), "alpha\nbeta\n").unwrap();
        expected_revision(&[("inputs/ids.txt", b"alpha\nbeta\n")])
    }

    fn bound_call(revision: &str, id: &str) -> String {
        json!({"input_revision": revision, "id": id}).to_string()
    }

    /// An old revision is refused, and self-declared acceptance or hash is data only, as on Chat
    /// Completions: it neither passes a check nor moves the adoption.
    #[test]
    fn an_old_revision_is_refused_and_self_declared_values_do_nothing_as_on_chat() {
        let _guard = lock();
        let old = expected_revision(&[("inputs/ids.txt", b"alpha\n")]);
        let mut answers = Vec::new();
        for api in APIS {
            let env = Env::new();
            compiled_tool(&env);
            let run = session(
                &env,
                api,
                vec![api.calls(&[("call_1", CONFIG_TOOL, &bound_call(&old, "alpha"))])],
            );
            assert!(run.executed.is_empty(), "{api:?}");
            let answer = run.last_output(1);
            assert!(answer.contains("schema_violation"), "{answer}");
            answers.push(answer);

            let env = Env::new();
            let revision = compiled_tool(&env);
            let forged = json!({
                "input_revision": revision,
                "id": "alpha",
                "accepted": true,
                "schema_sha256": "0".repeat(64),
            })
            .to_string();
            let run = session(
                &env,
                api,
                vec![api.calls(&[("call_1", CONFIG_TOOL, &forged)])],
            );
            assert_eq!(run.executed, vec![parsed(&forged)], "{api:?}");
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
        assert_eq!(answers[1], answers[0]);
        set_env(REGISTRY_ENV, None);
    }

    /// A tool pin, a compiler, or an input that changed during the run is refused at receipt
    /// with nothing executed, as on Chat Completions.
    #[test]
    fn a_pin_compiler_or_input_changed_during_the_run_is_refused_as_on_chat() {
        let _guard = lock();
        for (label, expected) in [
            (
                "pin",
                "profile_tool_refused: invalid_pin: stage=received_call",
            ),
            (
                "compiler",
                "profile_tool_refused: schema_mismatch: stage=received_call",
            ),
            ("input", "profile_tool_refused: input_revision_mismatch"),
        ] {
            for api in APIS {
                let env = Env::new();
                let revision = compiled_tool(&env);
                let registry = env.registry.path().to_path_buf();
                let input = env.root.join("inputs/ids.txt");
                let call = api.calls(&[("call_1", CONFIG_TOOL, &bound_call(&revision, "alpha"))]);
                let responder: Responder = Arc::new(move |index, _| {
                    if index == 1 {
                        match label {
                            "pin" => write_descriptor(
                                &registry,
                                (CONFIG_TOOL_ID, CONFIG_TOOL),
                                &schema_declaration(revision_root()),
                                "2.0.0",
                            ),
                            "compiler" => {
                                register_schema_compiler(COMPILER, Arc::new(ReplacedCompiler));
                            }
                            _ => std::fs::write(&input, "alpha\nbeta\ngamma\n").unwrap(),
                        }
                        return call.clone();
                    }
                    api.text("done")
                });
                let run = run_profile(&env, api, CONFIG_PROFILE, responder);
                assert!(run.executed.is_empty(), "{label} {api:?}");
                let error = run.result.unwrap_err();
                assert!(error.contains(expected), "{label} {api:?}: {error}");
                register_schema_compiler(COMPILER, Arc::new(LinesCompiler));
            }
        }
        set_env(REGISTRY_ENV, None);
    }

    // -----------------------------------------------------------------------------------------
    // Acceptance 4: the round trip, and call IDs.
    // -----------------------------------------------------------------------------------------

    /// The call ID goes back with the tool's output, not the item ID, and the output is the
    /// tool message's text as Chat Completions carries it.
    #[test]
    fn the_output_goes_back_under_the_call_id_with_the_tool_message_text() {
        let _guard = lock();
        let mut outputs = Vec::new();
        for api in APIS {
            let env = Env::with_config(config_root());
            let reply = match api {
                Api::Chat => api.calls(&[("call_9", CONFIG_TOOL, VALID_CONFIG)]),
                Api::Responses => responses_reply(vec![function_call(
                    "fc_item_9",
                    "call_9",
                    CONFIG_TOOL,
                    VALID_CONFIG,
                )]),
            };
            let run = session(&env, api, vec![reply]);
            assert_eq!(run.executed, vec![parsed(VALID_CONFIG)], "{api:?}");
            let sent = run.outputs(1);
            assert_eq!(sent.len(), 1, "{api:?}");
            assert_eq!(sent[0].0, "call_9", "{api:?}");
            assert!(sent[0].1.contains(ACCEPTED), "{api:?}: {}", sent[0].1);
            outputs.push(sent);
        }
        assert_eq!(outputs[1], outputs[0]);
        set_env(REGISTRY_ENV, None);
    }

    fn call_item(call_id: Value, name: &str, raw: &str) -> Value {
        json!({
            "type": "function_call",
            "id": "fc_bad",
            "call_id": call_id,
            "name": name,
            "arguments": raw,
            "status": "completed",
        })
    }

    fn without_call_id(name: &str, raw: &str) -> Value {
        let mut item = call_item(json!(null), name, raw);
        item.as_object_mut().unwrap().remove("call_id");
        item
    }

    const READ_NOTES: &str = r#"{"path":"notes.txt"}"#;

    struct IdCase {
        label: &'static str,
        /// The replies before the bad one, each a good reply with new call IDs.
        before: Vec<String>,
        bad: String,
        /// What ran before the bad reply.
        executed_before: usize,
    }

    fn valid(call_id: &str) -> String {
        responses_reply(vec![function_call(
            &format!("fc_{call_id}"),
            call_id,
            CONFIG_TOOL,
            VALID_CONFIG,
        )])
    }

    fn read(call_id: &str) -> String {
        responses_reply(vec![function_call(
            &format!("fc_{call_id}"),
            call_id,
            "Read",
            READ_NOTES,
        )])
    }

    fn id_cases() -> Vec<IdCase> {
        let one = |label, items: Vec<Value>| IdCase {
            label,
            before: Vec::new(),
            bad: responses_reply(items),
            executed_before: 0,
        };
        vec![
            one("missing", vec![without_call_id(CONFIG_TOOL, VALID_CONFIG)]),
            one(
                "empty",
                vec![call_item(json!(""), CONFIG_TOOL, VALID_CONFIG)],
            ),
            one(
                "not a string",
                vec![call_item(json!(7), CONFIG_TOOL, VALID_CONFIG)],
            ),
            one(
                "twice in the reply",
                vec![
                    call_item(json!("call_x"), CONFIG_TOOL, VALID_CONFIG),
                    call_item(json!("call_x"), CONFIG_TOOL, VALID_CONFIG),
                ],
            ),
            one(
                "a schema call and another call in the reply",
                vec![
                    call_item(json!("call_x"), "Read", READ_NOTES),
                    call_item(json!("call_x"), CONFIG_TOOL, VALID_CONFIG),
                ],
            ),
            one(
                "two other calls in the reply",
                vec![
                    call_item(json!("call_x"), "Read", READ_NOTES),
                    call_item(json!("call_x"), "Read", READ_NOTES),
                ],
            ),
            one(
                "a missing one after a good one",
                vec![
                    call_item(json!("call_y"), CONFIG_TOOL, VALID_CONFIG),
                    without_call_id("Read", READ_NOTES),
                ],
            ),
            IdCase {
                label: "a schema call held before",
                before: vec![valid("call_a")],
                bad: valid("call_a"),
                executed_before: 1,
            },
            IdCase {
                label: "another call held before, then a schema call",
                before: vec![read("call_r")],
                bad: valid("call_r"),
                executed_before: 0,
            },
            IdCase {
                label: "another call held before, then another call",
                before: vec![read("call_r")],
                bad: read("call_r"),
                executed_before: 0,
            },
        ]
    }

    /// A bad call ID fails the parse of the whole reply as a malformed native call: nothing of it
    /// runs, is kept, or is sent back, the loop retries with its feedback, and a good reply after
    /// it goes through as if the bad one had not come.
    #[test]
    fn a_bad_call_id_retries_and_nothing_of_that_reply_runs_or_stays() {
        let _guard = lock();
        for case in id_cases() {
            let env = Env::with_config(config_root());
            std::fs::write(env.root.join("notes.txt"), "text").unwrap();
            let bad_at = case.before.len();
            let mut replies = case.before.clone();
            replies.push(case.bad.clone());
            replies.push(valid("call_good"));
            let run = session(&env, Api::Responses, replies);
            let label = case.label;
            assert_eq!(run.result.as_deref(), Ok("done"), "{label}");
            assert_eq!(
                run.executed,
                vec![parsed(VALID_CONFIG); case.executed_before + 1],
                "{label}"
            );
            let retries = env
                .events()
                .into_iter()
                .filter(|event| event["event"] == "native_tool_parse_retry_feedback")
                .collect::<Vec<_>>();
            assert_eq!(retries.len(), 1, "{label}");
            let reason = retries[0]["reason"].as_str().unwrap();
            assert!(
                reason.contains("OpenAI function_call call_id"),
                "{label}: {reason}"
            );
            assert!(!reason.contains("call_x") && !reason.contains(VALID_CONFIG));
            assert!(
                !env.event_names()
                    .iter()
                    .any(|name| name == "fallback_decision"),
                "{label}"
            );
            // The request after the bad reply holds the calls of the one before it, and no more.
            assert_eq!(
                sent_calls(&run, bad_at + 1),
                sent_calls(&run, bad_at),
                "{label}"
            );
            assert!(run.request(bad_at + 1)["tools"].is_array(), "{label}");
            let history = run.history.to_string();
            assert!(
                !history.contains("call_x") && !history.contains("fc_bad"),
                "{label}"
            );
        }
        set_env(REGISTRY_ENV, None);
    }

    /// Bad call IDs in a row: the retry feedback up to the existing limit, then the run ends with
    /// the error, never on XML, with the same events as malformed native calls on Chat
    /// Completions in a schema session.
    #[test]
    fn bad_call_ids_in_a_row_end_the_run_at_the_retry_limit_as_malformed_calls_do_on_chat() {
        let _guard = lock();
        let env = Env::with_config(config_root());
        let bad = responses_reply(vec![without_call_id(CONFIG_TOOL, VALID_CONFIG)]);
        let responses = session(&env, Api::Responses, vec![bad.clone(), bad.clone(), bad]);
        let responses_events = env.event_names();
        assert!(responses.executed.is_empty());
        assert_eq!(responses.requests.len(), 3);
        let error = responses.result.unwrap_err();
        assert!(
            error.contains("OpenAI function_call call_id missing"),
            "{error}"
        );
        for request in &responses.requests {
            assert!(body(request)["tools"].is_array());
        }

        let control = Env::with_config(config_root());
        let broken = Api::Chat.calls(&[("call_1", "Read", "{not json")]);
        let chat = session(
            &control,
            Api::Chat,
            vec![broken.clone(), broken.clone(), broken],
        );
        assert_eq!(chat.requests.len(), 3);
        assert!(chat.result.is_err());
        assert_eq!(responses_events, control.event_names());
        assert_eq!(
            responses_events
                .iter()
                .filter(|name| *name == "native_tool_parse_retry_feedback")
                .count(),
            2
        );
        for event in env.events() {
            assert_ne!(event["allowed"], json!(true), "{event}");
        }
        set_env(REGISTRY_ENV, None);
    }

    // -----------------------------------------------------------------------------------------
    // Acceptance 5 (receipts): several calls in one reply keep their order and their receipts.
    // -----------------------------------------------------------------------------------------

    #[test]
    fn several_calls_in_one_reply_keep_their_order_and_their_own_receipts_as_on_chat() {
        let _guard = lock();
        let rows: [&[(&str, &str, &str)]; 4] = [
            &[
                ("call_1", CONFIG_TOOL, INVALID_CONFIG),
                ("call_2", CONFIG_TOOL, VALID_CONFIG),
            ],
            &[
                ("call_1", CONFIG_TOOL, VALID_CONFIG),
                ("call_2", CONFIG_TOOL, INVALID_CONFIG),
            ],
            &[
                ("call_1", CONFIG_TOOL, VALID_CONFIG),
                ("call_2", CONFIG_TOOL, INVALID_CONFIG),
                ("call_3", CONFIG_TOOL, VALID_CONFIG),
                ("call_4", CONFIG_TOOL, INVALID_CONFIG),
            ],
            &[
                ("call_1", "Read", READ_NOTES),
                ("call_2", CONFIG_TOOL, INVALID_CONFIG),
                ("call_3", CONFIG_TOOL, VALID_CONFIG),
            ],
        ];
        for calls in rows {
            let mut seen = Vec::new();
            for api in APIS {
                let env = Env::with_config(config_root());
                std::fs::write(env.root.join("notes.txt"), "text").unwrap();
                let run = session(&env, api, vec![api.calls(calls)]);
                let valid = calls
                    .iter()
                    .filter(|(_, name, raw)| *name == CONFIG_TOOL && *raw == VALID_CONFIG)
                    .count();
                assert_eq!(run.executed, vec![parsed(VALID_CONFIG); valid], "{api:?}");
                let outputs = run.outputs(1);
                assert_eq!(
                    outputs
                        .iter()
                        .map(|(id, _)| id.as_str())
                        .collect::<Vec<_>>(),
                    calls.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
                    "{api:?}"
                );
                for ((_, name, raw), (_, answer)) in calls.iter().zip(&outputs) {
                    if *name == CONFIG_TOOL {
                        assert_eq!(
                            answer.contains("enum_mismatch"),
                            *raw == INVALID_CONFIG,
                            "{api:?}: {answer}"
                        );
                    }
                }
                seen.push((outputs, sent_calls(&run, 1)));
            }
            assert_eq!(seen[1], seen[0]);
        }
        set_env(REGISTRY_ENV, None);
    }

    // -----------------------------------------------------------------------------------------
    // Acceptance 6: a turn goes back raw only when it is safe; otherwise it is rebuilt whole.
    // -----------------------------------------------------------------------------------------

    const CLEAN_SUMMARY: &str = "dummy-summary-clean-0000";
    const ENCRYPTED: &str = "opaque-dummy-encrypted-content-1005";
    const CLEAN_PREAMBLE: &str = "Submitting the settings.";

    fn input(run: &Run, index: usize) -> Vec<Value> {
        run.request(index)["input"].as_array().unwrap().clone()
    }

    /// The items of request 2 after its user message, up to the tool output.
    fn replayed(run: &Run) -> Vec<Value> {
        input(run, 1)
            .into_iter()
            .skip_while(|item| item.get("role").is_some())
            .take_while(|item| item["type"] != "function_call_output")
            .collect()
    }

    /// What the host's fallback sends for one call: the call alone, from the host's values.
    fn rebuilt(call_id: &str, name: &str, arguments: &str) -> Vec<Value> {
        vec![json!({
            "type": "function_call",
            "call_id": call_id,
            "name": name,
            "arguments": arguments,
        })]
    }

    #[test]
    fn a_safe_turn_goes_back_raw_once_with_its_reasoning_preamble_and_order() {
        let _guard = lock();
        let env = Env::with_config(config_root());
        env.install_dummy_secret();
        let items = vec![
            reasoning("rs_1", CLEAN_SUMMARY, ENCRYPTED),
            message("msg_1", CLEAN_PREAMBLE),
            unknown("unk_1", "dummy-unknown-clean-0000"),
            function_call("fc_1", "call_1", CONFIG_TOOL, VALID_CONFIG),
        ];
        let run = session(&env, Api::Responses, vec![responses_reply(items.clone())]);
        assert_eq!(run.executed, vec![parsed(VALID_CONFIG)]);
        assert_eq!(replayed(&run), items);
        let sent = run.requests[1].clone();
        assert_eq!(
            sent.matches("\"call_1\"").count(),
            2,
            "the call and its output"
        );
        assert_eq!(sent.matches(CLEAN_PREAMBLE).count(), 1);
        // The opaque content is sent back as it came and is shown nowhere.
        assert!(!run.history.to_string().contains(ENCRYPTED));
        assert!(
            !env.events()
                .iter()
                .any(|event| event.to_string().contains(ENCRYPTED)),
            "an event shows the encrypted content"
        );
        set_current(None);
        set_env(REGISTRY_ENV, None);
    }

    /// Each of the three conditions taken away on its own: the whole turn is rebuilt from the
    /// host's values, with no reasoning, preamble, or unknown item, and the run goes on.
    #[test]
    fn a_turn_failing_one_condition_is_rebuilt_whole_from_the_host() {
        let _guard = lock();
        let wrapped = r#"{"arguments":{"path":"notes.txt"}}"#;
        let rows: Vec<(&str, Value, Vec<Value>)> = vec![
            (
                "a refused schema call",
                function_call("fc_1", "call_1", CONFIG_TOOL, INVALID_CONFIG),
                rebuilt("call_1", CONFIG_TOOL, "{}"),
            ),
            (
                "a call the host reshaped",
                function_call("fc_1", "call_1", "Read", wrapped),
                rebuilt("call_1", "Read", READ_NOTES),
            ),
        ];
        for (label, call, expected) in rows {
            let env = Env::with_config(config_root());
            std::fs::write(env.root.join("notes.txt"), "text").unwrap();
            let items = vec![
                reasoning("rs_1", CLEAN_SUMMARY, ENCRYPTED),
                message("msg_1", CLEAN_PREAMBLE),
                call,
            ];
            let run = session(&env, Api::Responses, vec![responses_reply(items)]);
            assert_eq!(run.result.as_deref(), Ok("done"), "{label}");
            assert_eq!(replayed(&run), expected, "{label}");
            for absent in [ENCRYPTED, CLEAN_SUMMARY, CLEAN_PREAMBLE] {
                assert!(
                    !holds_anywhere(&run.requests[1], absent),
                    "{label}: {absent}"
                );
            }
        }

        let secret_items = [
            (
                "a secret in the preamble",
                message("msg_1", &format!("Submitting. {DUMMY_SECRET}")),
            ),
            (
                "a secret in the reasoning summary",
                reasoning("rs_1", &format!("dummy-summary {DUMMY_SECRET}"), ENCRYPTED),
            ),
            (
                "a secret in an unknown item",
                unknown("unk_1", &format!("dummy-unknown {DUMMY_SECRET}")),
            ),
        ];
        for (label, item) in secret_items {
            let env = Env::with_config(config_root());
            env.install_dummy_secret();
            let items = vec![
                item,
                function_call("fc_1", "call_1", CONFIG_TOOL, VALID_CONFIG),
            ];
            let run = session(&env, Api::Responses, vec![responses_reply(items)]);
            assert_eq!(run.result.as_deref(), Ok("done"), "{label}");
            assert_eq!(run.executed, vec![parsed(VALID_CONFIG)], "{label}");
            assert_eq!(
                replayed(&run),
                rebuilt("call_1", CONFIG_TOOL, &parsed(VALID_CONFIG).to_string()),
                "{label}"
            );
            for request in &run.requests {
                assert!(!holds_anywhere(request, DUMMY_SECRET), "{label}");
            }
            assert!(
                !holds_anywhere(&run.requests[1], "dummy-summary"),
                "{label}"
            );
            assert!(
                !holds_anywhere(&run.requests[1], "dummy-unknown"),
                "{label}"
            );
            set_current(None);
        }
        set_env(REGISTRY_ENV, None);
    }

    /// Two turns with the same text and the same call under different call IDs each go back
    /// once, in order, with their own reasoning; the text alone never picks the turn.
    #[test]
    fn the_same_text_and_call_under_two_call_ids_go_back_as_two_turns() {
        let _guard = lock();
        let env = Env::with_config(config_root());
        let turn = |n: usize| {
            vec![
                reasoning(&format!("rs_{n}"), CLEAN_SUMMARY, ENCRYPTED),
                message(&format!("msg_{n}"), CLEAN_PREAMBLE),
                function_call(
                    &format!("fc_{n}"),
                    &format!("call_{n}"),
                    CONFIG_TOOL,
                    VALID_CONFIG,
                ),
            ]
        };
        let run = session(
            &env,
            Api::Responses,
            vec![responses_reply(turn(1)), responses_reply(turn(2))],
        );
        assert_eq!(run.executed, vec![parsed(VALID_CONFIG); 2]);
        let sent = input(&run, 2)
            .into_iter()
            .skip_while(|item| item.get("role").is_some())
            .take_while(|item| item.get("role").is_none() || item["type"] == "message")
            .collect::<Vec<_>>();
        let outputs = run.outputs(2);
        let mut expected = turn(1);
        expected.push(
            json!({"type": "function_call_output", "call_id": "call_1", "output": outputs[0].1}),
        );
        expected.extend(turn(2));
        expected.push(
            json!({"type": "function_call_output", "call_id": "call_2", "output": outputs[1].1}),
        );
        assert_eq!(sent, expected);
        set_env(REGISTRY_ENV, None);
    }

    // -----------------------------------------------------------------------------------------
    // Acceptance 9: the corpus.
    // -----------------------------------------------------------------------------------------

    /// The Responses rows name the API and the tool protocol on two axes, and take their
    /// expectations from the Chat Completions rows; the existing `wires` stay as they are.
    #[test]
    fn the_corpus_names_the_responses_rows_this_binary_runs() {
        let cases: Value = serde_json::from_str(RESPONSES_CASES).unwrap();
        assert_eq!(cases["issue"], "#1005");
        assert_eq!(cases["kind"], "synthetic");
        let chat: Value = serde_json::from_str(CASES).unwrap();
        for row in cases["rows"].as_array().unwrap() {
            assert_eq!(row["api"], "responses", "{row}");
            assert_eq!(row["tool_protocol"], "native", "{row}");
            assert!(row.get("wires").is_none(), "{row}");
            if let Some(control) = row["chat_control"].as_str() {
                assert!(
                    chat["accepted"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|accepted| accepted["id"] == control),
                    "{row}"
                );
            }
        }
        for row in chat["accepted"].as_array().unwrap() {
            assert_eq!(row["wires"], json!(["native"]), "{row}");
        }
        let ids = cases["id_errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            id_cases()
                .iter()
                .map(|case| case.label.to_string())
                .collect::<Vec<_>>()
        );
        let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(
            manifest["fixture_sha256"]["responses-cases.json"],
            sha256(RESPONSES_CASES.as_bytes())
        );
    }
}
