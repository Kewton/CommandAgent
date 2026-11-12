//! Issue #1005: the same calls on Chat Completions and on the Responses API, for tests that
//! compare the host's contract on both wires against a disposable fake on 127.0.0.1.
//!
//! A Responses request is read back as the Chat Completions messages it stands for, so one
//! assertion reads both wires. Only the wire's envelope and item shapes are converted: no
//! value of a message, a call, or an output is changed. Items the Chat Completions wire has
//! no form for (reasoning, unknown items) are left out of that view and stay in the body.

#![allow(
    dead_code,
    reason = "each test binary that includes this file uses a different part of it"
)]

use std::io::{Read as _, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// Answers request `n` (from 1) with its body in hand.
pub type Responder = Arc<dyn Fn(usize, &str) -> String + Send + Sync>;

/// A disposable provider on 127.0.0.1 that keeps every request body and no header.
pub struct FakeHttp {
    pub url: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FakeHttp {
    pub fn start(responder: Responder) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests: Arc<Mutex<Vec<String>>> = Arc::default();
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

    pub fn finish(mut self) -> Vec<String> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().unwrap().join().unwrap();
        self.requests.lock().unwrap().clone()
    }
}

fn serve(mut stream: std::net::TcpStream, requests: &Mutex<Vec<String>>, responder: &Responder) {
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
    let reply = responder(index, &body);
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

/// Request `n` gets `replies[n - 1]`, in the API's shape already; past them, a final "done".
pub fn scripted(api: Api, replies: Vec<String>) -> Responder {
    Arc::new(move |index, _| {
        replies
            .get(index - 1)
            .cloned()
            .unwrap_or_else(|| api.text("done"))
    })
}

/// Which OpenAI API a client is configured with. The tool protocol is native on both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    Chat,
    Responses,
}

pub const APIS: [Api; 2] = [Api::Chat, Api::Responses];

impl Api {
    pub fn openai_api(self) -> commandagent::config::OpenAiApi {
        match self {
            Self::Chat => commandagent::config::OpenAiApi::ChatCompletions,
            Self::Responses => commandagent::config::OpenAiApi::Responses,
        }
    }

    /// One reply with these calls, each `(call_id, name, raw arguments)`, in this order.
    pub fn calls(self, calls: &[(&str, &str, &str)]) -> String {
        match self {
            Self::Chat => {
                let calls = calls
                    .iter()
                    .map(|(id, name, raw)| {
                        json!({
                            "id": id,
                            "type": "function",
                            "function": {"name": name, "arguments": raw},
                        })
                    })
                    .collect::<Vec<_>>();
                json!({"choices": [{"message": {
                    "role": "assistant",
                    "content": "",
                    "tool_calls": calls,
                }}]})
                .to_string()
            }
            Self::Responses => responses_reply(
                calls
                    .iter()
                    .map(|(id, name, raw)| function_call(&format!("fc_{id}"), id, name, raw))
                    .collect(),
            ),
        }
    }

    pub fn text(self, content: &str) -> String {
        match self {
            Self::Chat => {
                json!({"choices": [{"message": {"role": "assistant", "content": content}}]})
                    .to_string()
            }
            Self::Responses => responses_reply(vec![message("msg_text", content)]),
        }
    }
}

/// A Responses body with these output items. Like the Chat Completions replies here it reports
/// no usage, so neither wire's turn reads token counts the other's does not.
pub fn responses_reply(output: Vec<Value>) -> String {
    json!({
        "id": "resp_fake",
        "object": "response",
        "model": "m",
        "status": "completed",
        "output": output,
    })
    .to_string()
}

pub fn function_call(item_id: &str, call_id: &str, name: &str, raw: &str) -> Value {
    json!({
        "type": "function_call",
        "id": item_id,
        "call_id": call_id,
        "name": name,
        "arguments": raw,
        "status": "completed",
    })
}

pub fn message(id: &str, text: &str) -> Value {
    json!({
        "type": "message",
        "id": id,
        "role": "assistant",
        "status": "completed",
        "content": [{"type": "output_text", "text": text, "annotations": []}],
    })
}

pub fn reasoning(id: &str, summary: &str, encrypted: &str) -> Value {
    json!({
        "type": "reasoning",
        "id": id,
        "summary": [{"type": "summary_text", "text": summary}],
        "encrypted_content": encrypted,
    })
}

pub fn unknown(id: &str, value: &str) -> Value {
    json!({"type": "dummy_unknown_item", "id": id, "dummy_field": value})
}

/// A Chat Completions reply in the Responses shape: its text as one message item and its calls
/// as function_call items, in order. A call's `call_id` is `call_<reply>_<n>`, new for every
/// reply, and its item ID differs from it.
pub fn chat_reply_as_responses(reply: &str, reply_number: usize) -> String {
    let reply: Value = serde_json::from_str(reply).unwrap();
    let message_value = &reply["choices"][0]["message"];
    let mut output = Vec::new();
    if let Some(text) = message_value["content"].as_str()
        && !text.is_empty()
    {
        output.push(message(&format!("msg_{reply_number}"), text));
    }
    for (index, call) in message_value["tool_calls"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        let call_id = format!("call_{reply_number}_{index}");
        output.push(function_call(
            &format!("fc_{reply_number}_{index}"),
            &call_id,
            call["function"]["name"].as_str().unwrap(),
            call["function"]["arguments"].as_str().unwrap(),
        ));
    }
    responses_reply(output)
}

fn text_of(item: &Value) -> String {
    match &item["content"] {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<String>(),
        _ => String::new(),
    }
}

/// The request as Chat Completions messages: `role`, `content`, and for an assistant's calls
/// `tool_calls` (`id`, `function.name`, `function.arguments`), and `tool_call_id` for a tool
/// output. A Chat Completions body is read the same way, so both views compare as values.
pub fn view(body: &Value) -> Vec<Value> {
    if let Some(messages) = body["messages"].as_array() {
        return messages
            .iter()
            .map(|message| {
                let mut seen = json!({
                    "role": message["role"],
                    "content": message["content"].as_str().unwrap_or_default(),
                });
                if let Some(calls) = message["tool_calls"].as_array() {
                    seen["tool_calls"] = Value::Array(
                        calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "id": call["id"],
                                    "function": {
                                        "name": call["function"]["name"],
                                        "arguments": call["function"]["arguments"],
                                    },
                                })
                            })
                            .collect(),
                    );
                }
                if let Some(id) = message["tool_call_id"].as_str() {
                    seen["tool_call_id"] = json!(id);
                }
                seen
            })
            .collect();
    }
    let mut messages: Vec<Value> = Vec::new();
    for item in body["input"].as_array().into_iter().flatten() {
        match item["type"].as_str() {
            Some("function_call") => {
                let call = json!({
                    "id": item["call_id"],
                    "function": {"name": item["name"], "arguments": item["arguments"]},
                });
                match messages.last_mut() {
                    Some(last) if last["role"] == "assistant" && last["tool_calls"].is_array() => {
                        last["tool_calls"].as_array_mut().unwrap().push(call);
                    }
                    _ => messages.push(json!({
                        "role": "assistant",
                        "content": "",
                        "tool_calls": [call],
                    })),
                }
            }
            Some("function_call_output") => messages.push(json!({
                "role": "tool",
                "content": item["output"].as_str().unwrap_or_default(),
                "tool_call_id": item["call_id"],
            })),
            Some("message") | None if item.get("role").is_some() => {
                messages.push(json!({"role": item["role"], "content": text_of(item)}));
            }
            _ => {}
        }
    }
    messages
}

/// The request in the Chat Completions shape the existing tests read (`messages`).
pub fn as_chat_body(body: &Value) -> Value {
    json!({"messages": view(body)})
}

/// The tool outputs of a request, each `(call ID, content)`, in order.
pub fn tool_outputs(body: &Value) -> Vec<(String, String)> {
    view(body)
        .iter()
        .filter(|message| message["role"] == "tool")
        .map(|message| {
            (
                message["tool_call_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                message["content"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// Every string in a request: keys and values, and, where a string is itself JSON, every string
/// inside it too, at any depth.
pub fn every_string(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            out.push(text.clone());
            if let Ok(inner) = serde_json::from_str::<Value>(text)
                && (inner.is_object() || inner.is_array() || inner.is_string())
            {
                every_string(&inner, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| every_string(item, out)),
        Value::Object(fields) => {
            for (key, item) in fields {
                out.push(key.clone());
                every_string(item, out);
            }
        }
        _ => {}
    }
}

/// Whether `needle` is anywhere in a request: in its bytes, or in any string of it once decoded.
pub fn holds_anywhere(body: &str, needle: &str) -> bool {
    if body.contains(needle) {
        return true;
    }
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    let mut strings = Vec::new();
    every_string(&value, &mut strings);
    strings.iter().any(|text| text.contains(needle))
}
