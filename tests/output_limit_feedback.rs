//! Issue #958: the reasoning count of an OpenAI-compatible Chat Completions
//! response, and the one fixed recovery sentence after a reply that hit the
//! output limit, through the public minimal loop.
//!
//! Every scenario is a row of the synthetic corpus fixture
//! `tests/corpus/apps/output-limit-feedback/fixtures/events.jsonl`. A loopback
//! stand-in without credentials serves the row's scripted response bodies to
//! the real `openai-compatible` client, so each turn goes through the Chat
//! Completions parser, the client, `provider_call`, and the loop; no case
//! reaches a real provider.
//!
//! Each row's `base` was recorded from the base commit c84aaa82 before this
//! change: the outcome, the number of requests, a digest of every request
//! body, the event-name sequence, and a digest of every normalized event.
//! With the sentence removed from the requests that carry it, and with the
//! new event and the newly reported `provider_reasoning_tokens` removed from
//! the events, every run must reproduce its base exactly. The `expect`
//! entries come from the rules in the Issue body.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::Config;
use commandagent::minimal_loop::loop_run::run_session_with_outcome_with_ui;
use commandagent::provider_cli;
use commandagent::state::SessionSnapshot;
use commandagent::tui::NOOP_UI;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const FIXTURE: &str = include_str!("corpus/apps/output-limit-feedback/fixtures/events.jsonl");
const MODEL: &str = "compatible-model-958";
const EVENT: &str = "output_limit_feedback";
const REASONING_FIELD: &str = "provider_reasoning_tokens";

/// Event numbers the digest does not compare: wall-clock values and the
/// prompt-size estimates, which count the system prompt that names the
/// temporary root or depend on requests made earlier in the same binary.
const NORMALIZED_NUMBER_FIELDS: [&str; 2] = [
    "estimated_prompt_tokens_sent",
    "estimated_stable_prefix_chars",
];

/// The fixed sentence (prompt version 1) for a kind and an output limit.
fn sentence(kind: &str, num_predict: u64) -> String {
    let what = match kind {
        "no_tool_call_at_output_limit" => "before any tool call arrived",
        "tool_arguments_truncated_at_output_limit" => "and its tool call arguments were cut off",
        other => panic!("unknown kind {other}"),
    };
    format!(
        "Your previous response reached the output limit ({num_predict} tokens) {what}. Keep your reasoning short, start your next response with one tool call, and write a large file in smaller parts."
    )
}

fn rows() -> Vec<Value> {
    FIXTURE
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn row(id: &str) -> Value {
    rows()
        .into_iter()
        .find(|row| row["id"] == id)
        .unwrap_or_else(|| panic!("missing row {id}"))
}

/// One scripted Chat Completions body. `content` is the reply text,
/// `function` (when present) the raw function object of one tool call,
/// `completion` the reported completion count, and `reasoning` (when
/// present) the raw `completion_tokens_details.reasoning_tokens` value.
fn response_body(step: &Value) -> String {
    let mut message = json!({"content": step["content"].as_str().unwrap_or_default()});
    if let Some(function) = step.get("function") {
        message["tool_calls"] =
            json!([{"id": "call-958", "type": "function", "function": function}]);
    }
    let mut usage = json!({
        // Above any request's size estimate, so the process-wide
        // context-truncation counter never fires.
        "prompt_tokens": 20_000,
        "completion_tokens": step["completion"],
    });
    if let Some(reasoning) = step.get("reasoning") {
        usage["completion_tokens_details"] = json!({"reasoning_tokens": reasoning});
    }
    json!({
        "id": "chatcmpl-958",
        "object": "chat.completion",
        "model": MODEL,
        "created": 1_790_000_000,
        "choices": [{"index": 0, "message": message, "finish_reason": "stop"}],
        "usage": usage,
    })
    .to_string()
}

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    let mut data = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buffer).expect("request");
        assert!(read > 0, "client closed before sending the request");
        data.extend_from_slice(&buffer[..read]);
        let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&data[..end]).to_ascii_lowercase();
        let length = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if data.len() >= end + 4 + length {
            return data[end + 4..end + 4 + length].to_vec();
        }
    }
}

struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Vec<Vec<u8>>>,
}

impl Server {
    /// Serves `bodies` in order, one per connection, and records every
    /// request body it receives, including any beyond the scripted ones.
    fn spawn(bodies: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        listener.set_nonblocking(true).expect("nonblocking");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = std::thread::spawn(move || {
            let mut bodies = bodies.into_iter();
            let mut requests = Vec::new();
            while !thread_stop.load(Ordering::SeqCst) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("accept: {error}"),
                };
                stream.set_nonblocking(false).expect("blocking stream");
                requests.push(read_request(&mut stream));
                let (status, body) = match bodies.next() {
                    Some(body) => ("200 OK", body),
                    None => (
                        "400 Bad Request",
                        r#"{"error":"script exhausted"}"#.to_string(),
                    ),
                };
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .expect("response");
                stream.flush().expect("flush");
            }
            requests
        });
        Self {
            address,
            stop,
            handle,
        }
    }

    fn finish(self) -> Vec<Vec<u8>> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle.join().expect("server thread")
    }
}

fn config_for(address: SocketAddr, root: &Path, row: &Value) -> Config {
    let num_predict = row["num_predict"].as_u64().unwrap().to_string();
    let max_iterations = row["max_iterations"].as_u64().unwrap_or(6).to_string();
    let parsed = provider_cli::parse_from([
        "commandagent",
        "--cwd",
        &root.to_string_lossy(),
        "--state-dir",
        &root.join("state").to_string_lossy(),
        "--model",
        MODEL,
        "--provider",
        "openai-compatible",
        "--base-url",
        &format!("http://{address}/v1"),
        "--chat-timeout-secs",
        "30",
        "--yes",
        "--num-predict",
        &num_predict,
        "--max-iterations",
        &max_iterations,
    ])
    .expect("arguments");
    let mut config = Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
        .expect("config");
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

struct Trace {
    root: PathBuf,
    address: SocketAddr,
    _dir: tempfile::TempDir,
    outcome: Value,
    requests: Vec<Value>,
    events: Vec<Value>,
}

fn run(row: &Value) -> Trace {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::write(root.join("seed.md"), "seed notes\n").unwrap();
    let bodies = row["script"]
        .as_array()
        .unwrap()
        .iter()
        .map(response_body)
        .collect();
    let server = Server::spawn(bodies);
    let address = server.address;
    let config = config_for(address, &root, row);
    let mut client = commandagent::providers::client_from_config(&config, false).expect("client");
    let required: Vec<String> = row["required_paths"]
        .as_array()
        .map(|paths| {
            paths
                .iter()
                .map(|path| path.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default();
    let result = run_session_with_outcome_with_ui(
        client.as_mut(),
        &mut SessionSnapshot::new(),
        row["prompt"].as_str().unwrap(),
        &required,
        &config,
        &NOOP_UI,
    );
    let requests = server
        .finish()
        .iter()
        .map(|body| serde_json::from_slice(body).expect("request JSON"))
        .collect();
    let events = std::fs::read_to_string(root.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect();
    let mut trace = Trace {
        root,
        address,
        _dir: dir,
        outcome: Value::Null,
        requests,
        events,
    };
    trace.outcome = match result {
        Ok(outcome) => json!({
            "stop_reason": format!("{:?}", outcome.stop_reason),
            "iterations": outcome.iterations,
        }),
        Err(error) => json!({"error": trace.normalize(&format!("{error:#}"))}),
    };
    trace
}

fn digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Trace {
    fn normalize(&self, text: &str) -> String {
        let mut text = text.to_string();
        if let Ok(canonical) = self.root.canonicalize() {
            text = text.replace(&canonical.display().to_string(), "<ROOT>");
        }
        text = text.replace(&self.root.display().to_string(), "<ROOT>");
        text = text.replace(&self.address.to_string(), "<ADDR>");
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

    fn normalize_value(&self, value: &Value) -> Value {
        match value {
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| {
                        let wall_clock = key.ends_with("_ms") || key.ends_with("_epoch_ms");
                        let value = if value.is_number()
                            && (wall_clock || NORMALIZED_NUMBER_FIELDS.contains(&key.as_str()))
                        {
                            json!("<N>")
                        } else {
                            self.normalize_value(value)
                        };
                        (key.clone(), value)
                    })
                    .collect(),
            ),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.normalize_value(item))
                    .collect(),
            ),
            Value::String(text) => Value::String(self.normalize(text)),
            other => other.clone(),
        }
    }

    /// The request bodies with the sentence taken back out: from the end of
    /// the last message's text, or the whole message when it was the
    /// sentence alone.
    fn requests_without_sentence(&self, num_predict: u64) -> Vec<Value> {
        let sentences = [
            sentence("no_tool_call_at_output_limit", num_predict),
            sentence("tool_arguments_truncated_at_output_limit", num_predict),
        ];
        self.requests
            .iter()
            .map(|request| {
                let mut request = request.clone();
                let messages = request["messages"].as_array_mut().unwrap();
                let last = messages.last().unwrap()["content"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                for sentence in &sentences {
                    if last == *sentence {
                        messages.pop();
                    } else if let Some(kept) = last.strip_suffix(&format!("\n\n{sentence}")) {
                        let at = messages.len() - 1;
                        messages[at]["content"] = json!(kept);
                    }
                }
                request
            })
            .collect()
    }

    /// The events without the new event and with the newly reported
    /// reasoning count left out of `provider_turn_duration`.
    fn events_as_at_base(&self) -> Vec<Value> {
        self.events
            .iter()
            .filter(|event| event["event"] != EVENT)
            .map(|event| {
                let mut event = event.clone();
                if event["event"] == "provider_turn_duration" {
                    event.as_object_mut().unwrap().remove(REASONING_FIELD);
                }
                event
            })
            .collect()
    }

    /// The view of this run the base commit recorded.
    fn base_view(&self, num_predict: u64) -> Value {
        let requests: Vec<String> = self
            .requests_without_sentence(num_predict)
            .iter()
            .map(|request| digest(&self.normalize(&request.to_string())))
            .collect();
        let events = self.events_as_at_base();
        let names: Vec<&str> = events
            .iter()
            .map(|event| event["event"].as_str().unwrap_or_default())
            .collect();
        let lines: Vec<String> = events
            .iter()
            .map(|event| self.normalize_value(event).to_string())
            .collect();
        json!({
            "outcome": self.outcome,
            "calls": self.requests.len(),
            "request_digests": requests,
            "event_names": names,
            "event_digest": digest(&lines.join("\n")),
        })
    }

    fn feedback_events(&self) -> Vec<Value> {
        self.events
            .iter()
            .filter(|event| event["event"] == EVENT)
            .cloned()
            .collect()
    }

    fn sentence_count(&self, index: usize, sentence: &str) -> usize {
        self.requests[index]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| {
                message["content"]
                    .as_str()
                    .unwrap_or_default()
                    .matches(sentence)
                    .count()
            })
            .sum()
    }
}

/// The reasoning value a scripted step reports, under the #928 rule.
fn kept_reasoning(step: &Value) -> Option<u64> {
    let reasoning = step.get("reasoning")?.as_u64()?;
    let completion = step["completion"].as_u64();
    match completion {
        Some(total) if reasoning > total => None,
        _ => Some(reasoning),
    }
}

/// Runs one row and checks it against its recorded base and its
/// expectations.
fn check(id: &str) {
    let row = row(id);
    let num_predict = row["num_predict"].as_u64().unwrap();
    let trace = run(&row);
    let view = trace.base_view(num_predict);
    if view != row["base"] {
        println!("observed {id}: {view}");
    }
    assert_eq!(
        view, row["base"],
        "{id}: the run differs from its base apart from the sentence, the new event, and the reasoning count"
    );

    // Requests: the sentence appears exactly where expected, once.
    let expected = row["expect"]["feedback"].as_array().unwrap();
    for index in 0..trace.requests.len() {
        let wanted = expected
            .iter()
            .find(|entry| entry["request"].as_u64() == Some(index as u64));
        for kind in [
            "no_tool_call_at_output_limit",
            "tool_arguments_truncated_at_output_limit",
        ] {
            let count = trace.sentence_count(index, &sentence(kind, num_predict));
            let want = usize::from(wanted.is_some_and(|entry| entry["kind"] == kind));
            assert_eq!(count, want, "{id}: request {index} sentence {kind}");
        }
        if wanted.is_some() {
            let messages = trace.requests[index]["messages"].as_array().unwrap();
            let last = messages.last().unwrap()["content"].as_str().unwrap();
            assert!(last.ends_with("tool call, and write a large file in smaller parts."));
        }
    }
    // The request the loop sends after switching to XML fallback carries no
    // native tool schemas, and the requests before it do.
    if let Some(index) = row["expect"]["xml_fallback_request"].as_u64() {
        let index = index as usize;
        for (at, request) in trace.requests.iter().enumerate() {
            assert_eq!(
                request.get("tools").is_some(),
                at < index,
                "{id}: request {at} tools"
            );
        }
        let system = trace.requests[index]["messages"][0]["content"]
            .as_str()
            .unwrap();
        assert!(system.contains("XML fallback"), "{id}: {system}");
    }
    for entry in row["expect"]["contains"].as_array().into_iter().flatten() {
        let index = entry["request"].as_u64().unwrap() as usize;
        let text = entry["text"].as_str().unwrap();
        let last = trace.requests[index]["messages"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["content"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(last.contains(text), "{id}: request {index}: {last}");
    }

    // Events: one per sent sentence, with counts only.
    let written = trace.feedback_events();
    assert_eq!(written.len(), expected.len(), "{id}: {written:?}");
    for (event, entry) in written.iter().zip(expected) {
        let mut want = json!({
            "event": EVENT,
            "kind": entry["kind"],
            "attempt": entry["attempt"],
            "num_predict": num_predict,
            "completion_tokens": entry["completion_tokens"],
            "prompt_version": 1,
            "schema_version": "1",
        });
        if let Some(reasoning) = entry.get(REASONING_FIELD) {
            want[REASONING_FIELD] = reasoning.clone();
        }
        assert_eq!(*event, want, "{id}");
    }
    // Each new event follows the provider turn of the request it rode on.
    let names: Vec<&str> = trace
        .events
        .iter()
        .map(|event| event["event"].as_str().unwrap_or_default())
        .collect();
    for (at, name) in names.iter().enumerate() {
        if *name == EVENT {
            assert!(
                names[..at].contains(&"provider_turn_duration"),
                "{id}: {names:?}"
            );
        }
    }

    // The reasoning count reaches `provider_turn_duration` only when the
    // turn's response reported a value the #928 rule keeps.
    let turns: Vec<&Value> = trace
        .events
        .iter()
        .filter(|event| event["event"] == "provider_turn_duration")
        .collect();
    let script = row["script"].as_array().unwrap();
    for (index, turn) in turns.iter().enumerate() {
        // A body that fails to parse (`fails`) and a request beyond the
        // script have no metadata.
        let want = script
            .get(index)
            .filter(|step| step["fails"] != true)
            .and_then(kept_reasoning);
        assert_eq!(
            turn.get(REASONING_FIELD)
                .map(|value| value.as_u64().unwrap()),
            want,
            "{id}: turn {index}"
        );
    }
}

#[test]
fn no_tool_reply_at_the_limit_keeps_the_missing_artifact_feedback() {
    check("no-tool-missing-artifact");
}

#[test]
fn the_sentence_survives_the_empty_response_stage_rewriting_the_feedback() {
    check("empty-stage-rewrite");
}

#[test]
fn the_fresh_session_retry_request_carries_the_sentence() {
    check("fresh-session-retry");
}

#[test]
fn the_default_guard_stop_sends_nothing_more() {
    check("guard-default-stop");
}

#[test]
fn an_accepted_final_at_the_limit_sends_nothing_more() {
    check("accepted-final-at-limit");
}

#[test]
fn a_long_reply_with_a_tool_call_gets_no_sentence() {
    check("long-tool-call");
}

#[test]
fn arguments_cut_off_at_the_limit_add_the_sentence_to_the_malformed_feedback() {
    check("truncated-at-limit");
}

#[test]
fn arguments_cut_off_below_the_limit_keep_only_the_malformed_feedback() {
    check("truncated-below-limit");
}

#[test]
fn another_syntax_error_at_the_limit_keeps_only_the_malformed_feedback() {
    check("syntax-error-at-limit");
}

#[test]
fn a_missing_name_at_the_limit_keeps_only_the_malformed_feedback() {
    check("missing-name-at-limit");
}

#[test]
fn missing_arguments_at_the_limit_keep_only_the_malformed_feedback() {
    check("missing-arguments-at-limit");
}

#[test]
fn non_object_arguments_at_the_limit_keep_only_the_malformed_feedback() {
    check("non-object-at-limit");
}

/// Two native retries, then the third cut-off error switches the loop to XML
/// fallback; the XML fallback request carries the malformed feedback and the
/// sentence once each, and its event is the third attempt.
#[test]
fn the_xml_fallback_request_after_an_exhausted_retry_carries_the_sentence() {
    check("truncated-retry-exhausted");
}

/// The same sequence below the limit: the XML fallback request keeps only
/// the malformed feedback.
#[test]
fn the_xml_fallback_request_below_the_limit_gets_no_sentence() {
    check("truncated-below-limit-xml-fallback");
}

#[test]
fn a_zero_output_limit_never_adds_the_sentence() {
    check("num-predict-zero");
}

#[test]
fn malformed_reasoning_counts_stay_unreported() {
    check("reasoning-malformed-values");
}

#[test]
fn the_event_never_borrows_another_turns_reasoning_count() {
    check("reasoning-other-turn");
}

#[test]
fn the_fixture_rows_are_synthetic_and_each_has_a_test() {
    let source = include_str!("output_limit_feedback.rs");
    for row in rows() {
        assert_eq!(row["kind"], "synthetic");
        assert_eq!(row["source"], "c84aaa82");
        let id = row["id"].as_str().unwrap();
        assert!(
            source.contains(&format!("check(\"{id}\")")),
            "{id} has no test"
        );
        assert!(row["base"].is_object(), "{id} has no recorded base");
    }
}
