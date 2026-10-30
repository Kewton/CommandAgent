//! Issue #928: the cached input token count an OpenAI-compatible Chat
//! Completions response reports reaches the existing `provider_turn_duration`
//! event as `provider_cached_input_tokens`. A loopback stand-in without
//! credentials replays the usage objects of the corpus
//! `tests/corpus/apps/compatible-cached-input-usage/`; every turn goes through
//! `provider_call::chat`, so the worker clone and `take_response_metadata` are
//! on the path, and no case reaches a real provider.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use commandagent::config::Config;
use commandagent::provider_call::{self, ProviderCallScope};
use commandagent::provider_cli;
use commandagent::providers::ChatClient;
use commandagent::state::ConversationMessage;
use commandagent::tools::registry::ToolRegistry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const CORPUS: &str = "tests/corpus/apps/compatible-cached-input-usage";
const MODEL: &str = "compatible-model-928";
const EVENT_FIELD: &str = "provider_cached_input_tokens";

fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS)
}

fn corpus() -> Value {
    let path = corpus_root().join("fixtures/cases.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn case(corpus: &Value, id: &str) -> Value {
    corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("missing case {id}"))
        .clone()
}

/// A synthetic reply envelope around a corpus usage object: a native tool
/// call when `tool_call` is set, a plain text answer otherwise.
fn response_body(usage: &Value, tool_call: bool) -> String {
    let message = if tool_call {
        json!({
            "content": "",
            "tool_calls": [{
                "id": "call-928",
                "type": "function",
                "function": {"name": "Read", "arguments": "{\"path\":\"notes.md\"}"}
            }]
        })
    } else {
        json!({"content": "done"})
    };
    let mut body = json!({
        "id": "chatcmpl-928",
        "object": "chat.completion",
        "model": MODEL,
        "created": 1_790_000_000,
        "choices": [{"index": 0, "message": message, "finish_reason": "stop"}]
    });
    if !usage.is_null() {
        body["usage"] = usage.clone();
    }
    body.to_string()
}

struct Request {
    head: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> Request {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
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
            return Request {
                head,
                body: data[end + 4..end + 4 + length].to_vec(),
            };
        }
    }
}

struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Vec<Request>>,
}

impl Server {
    /// Serves `bodies` in order, one per connection, and records every
    /// request it receives, including any beyond the scripted ones.
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
                        std::thread::sleep(Duration::from_millis(10));
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

    fn finish(self) -> Vec<Request> {
        self.stop.store(true, Ordering::SeqCst);
        self.handle.join().expect("server thread")
    }
}

fn config_for(address: SocketAddr, root: &Path) -> Config {
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
    ])
    .expect("arguments");
    let mut config = Config::from_cli_with_provider_options(parsed.cli, parsed.provider_options)
        .expect("config");
    config.eval_events_path = Some(root.join("events.jsonl"));
    config
}

fn turn_events(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["event"] == "provider_turn_duration")
        .collect()
}

/// The event value expected for a corpus case: absent when unreported.
fn expected_field(case: &Value) -> Option<u64> {
    let expected = &case["expected_cached_input_tokens"];
    if expected.is_null() {
        None
    } else {
        Some(expected.as_u64().expect("expected count is a u64"))
    }
}

fn assert_event_field(event: &Value, expected: Option<u64>, label: &str) {
    match expected {
        Some(count) => assert_eq!(event[EVENT_FIELD], json!(count), "{label}: {event}"),
        None => assert!(
            event.get(EVENT_FIELD).is_none(),
            "{label}: unreported count must leave the field absent: {event}"
        ),
    }
}

/// Run one provider turn per body with the same client and the same request,
/// then return the replies, the turn events, and the recorded requests.
fn run_turns(
    bodies: Vec<String>,
) -> (
    Vec<commandagent::providers::AssistantReply>,
    Vec<Value>,
    Vec<Request>,
) {
    let dir = tempfile::tempdir().expect("tempdir");
    let turns = bodies.len();
    let server = Server::spawn(bodies);
    let config = config_for(server.address, dir.path());
    let events = config.eval_events_path.clone().expect("events path");
    let mut client: Box<dyn ChatClient> =
        commandagent::providers::client_from_config(&config, false).expect("client");
    let registry = ToolRegistry::default();
    let messages = [ConversationMessage::user("Summarize notes.md.")];
    let mut replies = Vec::new();
    for turn in 0..turns {
        let outcome = provider_call::chat(
            client.as_mut(),
            &config,
            ProviderCallScope::Executor,
            MODEL,
            &messages,
            registry.specs(),
            true,
        );
        let reply = outcome
            .result
            .unwrap_or_else(|error| panic!("turn {turn} failed: {error:#}"));
        replies.push(reply);
    }
    let requests = server.finish();
    let events = turn_events(&events);
    (replies, events, requests)
}

fn assert_requests_unchanged(requests: &[Request], turns: usize) {
    assert_eq!(
        requests.len(),
        turns,
        "one HTTP request per provider turn and none for usage"
    );
    let digests = requests
        .iter()
        .map(|request| format!("{:x}", Sha256::digest(&request.body)))
        .collect::<Vec<_>>();
    for request in requests {
        assert!(
            request
                .head
                .starts_with("post /v1/chat/completions http/1.1"),
            "{}",
            request.head
        );
        assert!(
            !request.head.contains("authorization:"),
            "no credential is configured or sent"
        );
        let body: Value = serde_json::from_slice(&request.body).expect("request json");
        assert_eq!(body["model"], MODEL);
        for absent in ["stream", "stream_options", "usage", "prompt_tokens_details"] {
            assert!(
                body.get(absent).is_none(),
                "request gained {absent}: {body}"
            );
        }
        assert!(
            !String::from_utf8_lossy(&request.body).contains("cached"),
            "request mentions a cache field"
        );
    }
    assert!(
        digests.iter().all(|digest| digest == &digests[0]),
        "the same input must send byte-identical requests whatever usage came back: {digests:?}"
    );
}

#[test]
fn hit_missing_zero_hit_reach_each_turn_event_without_carry_over() {
    let corpus = corpus();
    let sequence = corpus["sequence"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| case(&corpus, id.as_str().unwrap()))
        .collect::<Vec<_>>();
    let expected = sequence.iter().map(expected_field).collect::<Vec<_>>();
    assert_eq!(expected, vec![Some(5344), None, Some(0), Some(2688)]);
    let bodies = sequence
        .iter()
        .enumerate()
        .map(|(index, case)| response_body(&case["usage"], index % 2 == 0))
        .collect::<Vec<_>>();

    let (replies, events, requests) = run_turns(bodies);

    assert_eq!(events.len(), sequence.len(), "{events:?}");
    for (index, ((case, event), reply)) in sequence.iter().zip(&events).zip(&replies).enumerate() {
        let label = format!("turn {index} {}", case["id"]);
        assert_eq!(event["ok"], true, "{label}");
        assert_eq!(event["provider"], "openai-compatible", "{label}");
        assert_event_field(event, expected[index], &label);
        assert_eq!(
            event["prompt_eval_count"], case["usage"]["prompt_tokens"],
            "{label}: the prompt total is the reported one, not cached added to it"
        );
        assert_eq!(
            event["eval_count"], case["usage"]["completion_tokens"],
            "{label}"
        );
        assert_eq!(reply.prompt_tokens, case["usage"]["prompt_tokens"].as_u64());
        assert_eq!(
            reply.completion_tokens,
            case["usage"]["completion_tokens"].as_u64()
        );
        if index % 2 == 0 {
            assert_eq!(event["tool_calls_in_response"], 1, "{label}");
            assert_eq!(reply.tool_calls.len(), 1, "{label}");
            assert_eq!(reply.tool_calls[0].name, "Read", "{label}");
            assert_eq!(reply.tool_calls[0].arguments["path"], "notes.md", "{label}");
        } else {
            assert_eq!(event["tool_calls_in_response"], 0, "{label}");
            assert_eq!(reply.content, "done", "{label}");
        }
        assert_eq!(event["provider_response_id"], "chatcmpl-928", "{label}");
        assert!(event.get("provider_total_tokens").is_none(), "{label}");
    }
    assert_requests_unchanged(&requests, sequence.len());
}

#[test]
fn every_corpus_case_reaches_the_event_as_reported_or_absent() {
    let corpus = corpus();
    let cases = corpus["cases"].as_array().unwrap().clone();
    let bodies = cases
        .iter()
        .map(|case| response_body(&case["usage"], false))
        .collect::<Vec<_>>();

    let (replies, events, requests) = run_turns(bodies);

    assert_eq!(events.len(), cases.len(), "{events:?}");
    for ((case, event), reply) in cases.iter().zip(&events).zip(&replies) {
        let label = case["id"].as_str().unwrap();
        assert_eq!(
            event["ok"], true,
            "{label}: a bad count must not fail the turn"
        );
        assert_eq!(reply.content, "done", "{label}");
        assert_event_field(event, expected_field(case), label);
        let prompt_total = case["usage"]["prompt_tokens"].as_u64();
        assert_eq!(reply.prompt_tokens, prompt_total, "{label}");
        assert_eq!(
            event["prompt_eval_count"],
            json!(prompt_total),
            "{label}: no prompt total is invented or increased"
        );
    }
    assert_requests_unchanged(&requests, cases.len());
}

fn sorted_keys(value: &Value) -> Vec<&str> {
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

#[test]
fn corpus_separates_recorded_usage_excerpts_from_synthetic_cases() {
    let root = corpus_root();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("source-manifest.json")).unwrap())
            .unwrap();
    let bytes = std::fs::read(root.join("fixtures/cases.json")).unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    assert_eq!(manifest["fixture_sha256"]["cases.json"], digest);

    let text = String::from_utf8(bytes).unwrap();
    for forbidden in ["/Users/", "/Volumes/", "/home/", "Bearer", "sk-", "api_key"] {
        assert!(!text.contains(forbidden), "fixture contains {forbidden}");
    }

    let corpus = corpus();
    let sources = manifest["recorded_source"]["responses"]
        .as_object()
        .unwrap();
    let mut recorded = 0;
    for case in corpus["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let keys = sorted_keys(case);
        match case["kind"].as_str().unwrap() {
            "recorded" => {
                recorded += 1;
                assert!(id.starts_with("recorded-"), "{id}");
                let source = case["source"].as_str().unwrap();
                assert!(sources.contains_key(source), "{id}: {source} has no hash");
                assert_eq!(
                    keys,
                    [
                        "expected_cached_input_tokens",
                        "id",
                        "kind",
                        "source",
                        "usage"
                    ],
                    "{id}: a recorded case keeps only the usage excerpt"
                );
                assert_eq!(
                    sorted_keys(&case["usage"]),
                    [
                        "completion_tokens",
                        "prompt_tokens",
                        "prompt_tokens_details",
                        "total_tokens"
                    ],
                    "{id}"
                );
                assert_eq!(
                    case["usage"]["prompt_tokens_details"]["cached_tokens"],
                    case["expected_cached_input_tokens"],
                    "{id}"
                );
            }
            "synthetic" => {
                assert!(id.starts_with("synthetic-"), "{id}");
                assert!(case.get("source").is_none(), "{id}");
            }
            other => panic!("{id}: unknown kind {other}"),
        }
    }
    assert_eq!(recorded, sources.len());
}
