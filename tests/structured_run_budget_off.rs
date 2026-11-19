//! Issue #976 (G5), acceptance 6: with no wallet selected, a v1 (Chat Completions) run of the
//! binary is what the code before #976 recorded, byte for byte after the normalization of #1006
//! and nothing wider. The paths, the excluded paths and how the golden was recorded are in
//! `tests/corpus/apps/structured-run-budget/`; every test its `fixtures/cases.json` names is
//! checked here to exist.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

const CORPUS: &str = "tests/corpus/apps/structured-run-budget";
const OFF_GOLDEN: &str = include_str!("corpus/apps/structured-run-budget/fixtures/off-golden.json");
const FIXED_ROOT_LENGTH: usize = 128;
/// The items of `responses-run-budget/fixtures/off-golden.json` (#1006), word for word.
const NORMALIZED: [&str; 8] = [
    "the run's temporary root, wherever it appears in a string -> <ROOT>",
    "the fake's loopback address http://127.0.0.1:<port> and 127.0.0.1:<port> -> <LOOPBACK>",
    "a non-negative integer, at any depth of an event, under one of TIMING_KEYS -> 0; any other type is kept",
    "a string under build_commit or build_timestamp, a boolean under build_dirty -> \"<BUILD>\"; any other type is kept",
    "in the workspace lock file, the values of pid and run_id -> \"<PID>\", \"<RUN>\"",
    "in a bash_sandbox_status event, a backend that is one of SANDBOX_BACKENDS -> \"<SANDBOX_BACKEND>\"; any other value or type is kept",
    "not normalized: the run's root is exactly FIXED_ROOT_LENGTH bytes on every host, so the prompt-size estimates are compared as they are",
    "nothing else: every other byte of a request, an event line, a file, and the outcome is compared",
];
const TIMING_KEYS: &str = "duration_ms occurred_at_epoch_ms executor_ms provider_ms total_ms \
                           time_profile_provider_ms time_profile_total_ms";
const SANDBOX_BACKENDS: &str = "seatbelt landlock none";

fn named(list: &str, name: &str) -> bool {
    list.split_whitespace().any(|item| item == name)
}

/// A fake provider on 127.0.0.1: each request gets the next reply, the last one repeats. It
/// blocks on `accept`; the thread ends on a connection that sends no request line, which is the
/// one `finish` makes after the child has exited.
struct Script {
    url: String,
    thread: std::thread::JoinHandle<Vec<(String, String)>>,
}

impl Script {
    fn spawn(mut replies: Replies) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let mut seen = Vec::new();
            for mut stream in listener.incoming().map(Result::unwrap) {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let (mut line, mut length) = (String::new(), 0);
                reader.read_line(&mut line).unwrap();
                if line.is_empty() {
                    return seen;
                }
                let mut header = String::new();
                while reader.read_line(&mut header).unwrap() > 2 {
                    if let Some((name, value)) = header.split_once(':')
                        && name.eq_ignore_ascii_case("content-length")
                    {
                        length = value.trim().parse().unwrap();
                    }
                    header.clear();
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                let line = line.trim_end().to_string();
                seen.push((line, String::from_utf8_lossy(&body).to_string()));
                let (status, body) = match replies.len() {
                    1 => replies[0].clone(),
                    _ => replies.remove(0),
                };
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
            seen
        });
        Self { url, thread }
    }

    fn finish(self) -> Vec<(String, String)> {
        drop(TcpStream::connect(self.url.trim_start_matches("http://")));
        self.thread.join().unwrap()
    }
}

/// A root of exactly FIXED_ROOT_LENGTH bytes after canonicalization (`World::fixed_length` of
/// #1006), so the prompt-size estimates that count the raw path are equal on every host.
fn fixed_root(temporary: &tempfile::TempDir) -> std::path::PathBuf {
    let canonical = std::fs::canonicalize(temporary.path()).unwrap();
    let length = canonical.as_os_str().len();
    assert!(length + 2 <= FIXED_ROOT_LENGTH, "TMPDIR too long");
    let base = canonical.join("p".repeat(FIXED_ROOT_LENGTH - length - 1));
    for directory in "workspace state home operator/run-budgets ledgers".split(' ') {
        std::fs::create_dir_all(base.join(directory)).unwrap();
    }
    let base = std::fs::canonicalize(base).unwrap();
    assert_eq!(base.as_os_str().len(), FIXED_ROOT_LENGTH);
    base
}

fn chat_reply(id: &str, message: Value, finish: &str) -> (u16, String) {
    let usage = json!({"prompt_tokens": 50, "completion_tokens": 5, "total_tokens": 55,
                       "prompt_tokens_details": {"cached_tokens": 10}});
    let choice = json!({"index": 0, "message": message, "finish_reason": finish});
    let reply = json!({"id": id, "object": "chat.completion", "model": "m-1-snapshot",
                       "choices": [choice], "usage": usage});
    (200, reply.to_string())
}

type Replies = Vec<(u16, String)>;

fn tool_call(prefix: &str) -> Replies {
    let arguments = "{\"path\":\"notes.txt\",\"content\":\"synthetic notes\\n\"}";
    let call = json!({"id": format!("call_{prefix}1"), "type": "function",
                      "function": {"name": "Write", "arguments": arguments}});
    let message = json!({"role": "assistant", "content": null, "tool_calls": [call]});
    let done = json!({"role": "assistant", "content": "done"});
    let first = chat_reply(&format!("{prefix}1"), message, "tool_calls");
    vec![first, chat_reply(&format!("{prefix}2"), done, "stop")]
}

/// (id, provider, replies, chat retries). ordinary-reply and client-error are excluded: see the
/// corpus README, "Excluded paths".
#[rustfmt::skip]
fn paths() -> [(&'static str, &'static str, Replies, &'static str); 3] {
    let done = json!({"role": "assistant", "content": "done"});
    let retry = vec![(500, "{}".to_string()), chat_reply("c-x1", done, "stop")];
    [
        ("compatible-chat-tool-call", "openai-compatible", tool_call("c-t"), "0"),
        ("compatible-chat-retry", "openai-compatible", retry, "1"),
        ("lm-studio-chat-tool-call", "lm-studio", tool_call("l-t"), "0"),
    ]
}

fn files_under(root: &Path) -> BTreeMap<String, String> {
    let walk = ignore::WalkBuilder::new(root)
        .standard_filters(false)
        .build();
    let files = walk
        .map(Result::unwrap)
        .filter(|entry| entry.path().is_file());
    let file = |path: &Path| String::from_utf8_lossy(&std::fs::read(path).unwrap()).to_string();
    let name = |path: &Path| path.strip_prefix(root).unwrap().display().to_string();
    files
        .map(|entry| (name(entry.path()), file(entry.path())))
        .collect()
}

/// The normalization; `on` false keeps every byte (the raw evidence of each item).
struct Normalize<'a> {
    on: bool,
    root: &'a str,
    loopback: &'a str,
}

impl Normalize<'_> {
    fn text(&self, text: &str) -> String {
        if !self.on {
            return text.to_string();
        }
        let host = self.loopback.trim_start_matches("http://");
        let text = text.replace(self.root, "<ROOT>");
        text.replace(self.loopback, "<LOOPBACK>")
            .replace(host, "<LOOPBACK>")
    }

    fn value(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for (key, item) in map.iter_mut() {
                    let build = match key.as_str() {
                        "build_dirty" => item.is_boolean(),
                        "build_commit" | "build_timestamp" => item.is_string(),
                        _ => false,
                    };
                    if named(TIMING_KEYS, key) && item.is_u64() {
                        *item = json!(0);
                    } else if build {
                        *item = json!("<BUILD>");
                    } else {
                        Self::value(item);
                    }
                }
            }
            Value::Array(items) => items.iter_mut().for_each(Self::value),
            _ => {}
        }
    }

    fn event(&self, line: &str) -> String {
        let Ok(mut value) = serde_json::from_str::<Value>(line) else {
            return format!("<NOT JSON> {}", self.text(line));
        };
        if self.on {
            Self::value(&mut value);
            let sandbox = value["event"] == "bash_sandbox_status";
            if let Some(backend) = value.get_mut("backend").filter(|_| sandbox)
                && backend
                    .as_str()
                    .is_some_and(|name| named(SANDBOX_BACKENDS, name))
            {
                *backend = json!("<SANDBOX_BACKEND>");
            }
        }
        self.text(&value.to_string())
    }

    fn file(&self, name: &str, content: String) -> String {
        let lock = self.on && name == ".commandagent/lock";
        let mut value = serde_json::from_str::<Value>(&content).ok();
        let value = value
            .as_mut()
            .filter(|value| lock && value.get("pid").is_some());
        let Some(value) = value else {
            return self.text(&content);
        };
        value["pid"] = json!("<PID>");
        value["run_id"] = json!("<RUN>");
        self.text(&value.to_string())
    }
}

/// One path through `main` with no wallet variable, key or proxy: its environment is HOME, PATH,
/// the operator root and the event file. `output` waits for the child.
fn record(id: &str, provider: &str, replies: Replies, retries: &str, on: bool) -> Value {
    let temporary = tempfile::tempdir().unwrap();
    let base = fixed_root(&temporary);
    let script = Script::spawn(replies);
    let endpoint = match provider {
        "lm-studio" => "--lm-studio-host",
        _ => "--base-url",
    };
    let root = base.to_str().unwrap();
    let arguments = format!(
        "--provider {provider} {endpoint} {}/v1 --api chat-completions --model m-1 \
         --chat-retries {retries} --yes",
        script.url
    );
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .env_clear()
        .env("HOME", base.join("home"))
        .env("PATH", "/usr/bin:/bin")
        .env("COMMANDAGENT_ADAPTER_CONFIG_ROOT", base.join("operator"))
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env("COMMANDAGENT_EVAL_EVENTS", base.join("events.jsonl"))
        .args([
            "--cwd",
            &format!("{root}/workspace"),
            "--state-dir",
            &format!("{root}/state"),
        ])
        .args(arguments.split(' '))
        .args(["--prompt", "say hi"])
        .current_dir(base.join("workspace"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let error = stderr.lines().rfind(|line| line.starts_with("error"));
    let loopback = script.url.clone();
    let n = Normalize {
        on,
        root,
        loopback: &loopback,
    };
    let requests: Vec<Value> = (script.finish().into_iter())
        .map(|(line, body)| json!({"line": line, "body": n.text(&body)}))
        .collect();
    let events = std::fs::read_to_string(base.join("events.jsonl")).unwrap_or_default();
    let events: Vec<String> = events.lines().map(|line| n.event(line)).collect();
    let artifacts: BTreeMap<String, String> = (files_under(&base.join("workspace")).into_iter())
        .map(|(name, content)| (name.clone(), n.file(&name, content)))
        .collect();
    json!({
        "id": id, "requests": requests, "events": events, "artifacts": artifacts,
        "outcome": {"code": output.status.code(), "error": n.text(error.unwrap_or(""))},
        "ledger_files": files_under(&base.join("ledgers")).len()
    })
}

/// Every path once, as the bytes of `off-golden.json`.
fn record_all(on: bool) -> String {
    let paths =
        paths().map(|(id, provider, replies, retries)| record(id, provider, replies, retries, on));
    let record = json!({"issue": "#976", "kind": "structured-run-budget-off",
                        "normalization": NORMALIZED, "paths": paths});
    serde_json::to_string_pretty(&record).unwrap() + "\n"
}

/// None when the two are byte-equal; otherwise the first differing line of each.
fn difference(golden: &str, now: &str) -> Option<String> {
    if golden.as_bytes() == now.as_bytes() {
        return None;
    }
    let (mut base, mut current) = (golden.split('\n'), now.split('\n'));
    (1..).find_map(|line| match (base.next(), current.next()) {
        (Some(a), Some(b)) if a == b => None,
        (a, b) => Some(format!("line {line}: base {a:?} now {b:?}")),
    })
}

/// The recorder: `STRUCTURED_RUN_BUDGET_OFF_OUT=<file>` writes the record there, raw when
/// `STRUCTURED_RUN_BUDGET_OFF_RAW` is set. Without it the test does nothing.
#[test]
fn records_the_off_golden() {
    if let Some(out) = std::env::var_os("STRUCTURED_RUN_BUDGET_OFF_OUT") {
        let raw = std::env::var_os("STRUCTURED_RUN_BUDGET_OFF_RAW").is_some();
        std::fs::write(out, record_all(!raw)).unwrap();
    }
}

#[test]
fn ac6_with_no_wallet_a_v1_run_is_byte_equal_to_the_recording_before_976() {
    if let Some(difference) = difference(OFF_GOLDEN, &record_all(true)) {
        panic!("the off run changed: {difference}");
    }
}

#[test]
fn ac6_the_off_comparison_fails_without_the_normalization_and_on_one_changed_byte() {
    let now = record_all(true);
    assert_eq!(difference(OFF_GOLDEN, &now), None);
    let raw = difference(OFF_GOLDEN, &record_all(false));
    assert!(raw.is_some(), "a raw run passed");
    let bytes = OFF_GOLDEN.as_bytes();
    for at in (0..bytes.len()).step_by(997).chain([bytes.len() - 2]) {
        let mut changed = bytes.to_vec();
        changed[at] = if changed[at] == b'x' { b'y' } else { b'x' };
        let changed = difference(&String::from_utf8(changed).unwrap(), &now);
        assert!(changed.is_some(), "byte {at} changed and passed");
    }
}

/// Each `path::name` of cases.json whose file has no `fn <name>(`.
fn missing_tests(cases: &Value) -> Vec<&str> {
    let tests = cases["cases"].as_array().unwrap().iter();
    let tests = tests.flat_map(|case| case["tests"].as_array().unwrap().iter());
    let tests = tests.map(|test| test.as_str().unwrap());
    tests
        .filter(|test| {
            let (file, name) = test.rsplit_once("::").unwrap();
            let file = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(file));
            !file.unwrap_or_default().contains(&format!("fn {name}("))
        })
        .collect()
}

#[test]
fn every_test_cases_json_names_exists_and_a_made_up_one_is_caught() {
    let path = format!(
        "{}/{CORPUS}/fixtures/cases.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut cases: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let criteria: Vec<&str> = (cases["cases"].as_array().unwrap().iter())
        .inspect(|case| assert!(!case["tests"].as_array().unwrap().is_empty(), "{case}"))
        .map(|case| case["criterion"].as_str().unwrap())
        .collect();
    assert_eq!(criteria, ["AC1", "AC2", "AC3", "AC4", "AC5", "AC6"]);
    assert_eq!(missing_tests(&cases), Vec::<&str>::new());
    let made_up = "tests/structured_run_budget_off.rs::no_such_test";
    cases["cases"][0]["tests"][0] = json!(made_up);
    assert_eq!(missing_tests(&cases), [made_up]);
}
