//! Issue #1005: G2 schema tools on the Responses API.
//!
//! Everything here applies only in a session with a schema tool
//! ([`invocation::blocks_xml_fallback`]). A session without one keeps the parent module's
//! request, parser, and replay exactly as they were.
//!
//! - Sending: an admitted schema tool's definition says `strict: false`, so the service does
//!   not tighten the operator's schema; every other definition is unchanged.
//! - Receiving: every call ID of the reply is checked before any call is received, so a bad
//!   reply leaves no receipt. A schema tool's raw `arguments` go to G2 as text.
//! - Replay: a stored turn's raw items go back only when G2 refused none of its calls, its
//!   calls are exactly the host's, and the secret scrub leaves its items unchanged. Otherwise
//!   the whole turn is rebuilt from the host's values, and its reasoning is dropped.

use std::collections::BTreeSet;

use serde_json::Value;

use super::super::parsing::sanitized_tool_schema;
use super::{
    ConversationState, StoredTurn, normalize_function_arguments, push_assistant_fallback,
    push_other,
};
use crate::state::{ConversationMessage, ToolCall};
use crate::tools::args_recovery::recover_tool_arguments;
use crate::tools::profile_tools::schema::invocation;
use crate::tools::registry::ToolSpec;

const CALL_ID_MISSING: &str = "OpenAI function_call call_id missing";
const CALL_ID_NOT_A_STRING: &str = "OpenAI function_call call_id is not a string";
const CALL_ID_EMPTY: &str = "OpenAI function_call call_id empty";
const CALL_ID_DUPLICATE: &str = "OpenAI function_call call_id duplicate";
const CALL_ID_CORRELATION: &str = "OpenAI function_call call_id correlation invalid";

pub(super) fn in_session() -> bool {
    invocation::blocks_xml_fallback()
}

/// The flat Responses definition. An admitted schema tool says `strict: false`; with no schema
/// tool selected nothing is a schema tool, and every definition is the parent's.
pub(super) fn tool_definition(tool: &ToolSpec) -> Value {
    let mut definition = sanitized_tool_schema(tool);
    if invocation::is_schema_tool(&tool.function.name) {
        definition["strict"] = Value::Bool(false);
    }
    definition
}

fn is_function_call(item: &Value) -> bool {
    item.get("type").and_then(Value::as_str) == Some("function_call")
}

/// The call IDs the conversation already holds: the client's stored turns and the history
/// this request carries, as one set, so a turn in both counts once.
fn known_call_ids(messages: &[ConversationMessage], state: &ConversationState) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    for turn in &state.turns {
        for item in turn.output.iter().filter(|item| is_function_call(item)) {
            if let Some(id) = item.get("call_id").and_then(Value::as_str) {
                known.insert(id.to_string());
            }
        }
    }
    for message in messages {
        known.extend(message.tool_calls.iter().map(|call| call.id.clone()));
        if let Some(id) = &message.tool_call_id {
            known.insert(id.clone());
        }
    }
    known
}

/// Every call of the reply, before any of them is received: a call ID that is there, is a
/// string, is not empty, and is new to the reply and to the conversation. The diagnostics are
/// fixed text and carry no ID, argument, or secret; each names `function_call`, which the loop
/// retries as a malformed native call.
pub(super) fn check_call_ids(
    output: &[Value],
    messages: &[ConversationMessage],
    state: &ConversationState,
) -> anyhow::Result<()> {
    let known = known_call_ids(messages, state);
    let mut seen = BTreeSet::new();
    for item in output.iter().filter(|item| is_function_call(item)) {
        let id = match item.get("call_id") {
            None => anyhow::bail!(CALL_ID_MISSING),
            Some(Value::String(id)) => id,
            Some(_) => anyhow::bail!(CALL_ID_NOT_A_STRING),
        };
        if id.is_empty() {
            anyhow::bail!(CALL_ID_EMPTY);
        }
        if known.contains(id) || !seen.insert(id.as_str()) {
            anyhow::bail!(CALL_ID_DUPLICATE);
        }
    }
    Ok(())
}

/// The calls of one reply whose IDs were checked, in the reply's order.
pub(super) struct Receiver {
    envelope_clean: bool,
    refused: bool,
}

impl Receiver {
    /// The whole body is checked for a repeated key once, as on Chat Completions.
    pub(super) fn new(body: &str) -> Self {
        Self {
            envelope_clean: invocation::envelope_is_clean(body),
            refused: false,
        }
    }

    /// A schema tool's call goes to G2 with its raw `arguments`; any other call keeps the
    /// parent's parse, with the checked call ID.
    pub(super) fn call(&mut self, item: &Value) -> anyhow::Result<ToolCall> {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("OpenAI function_call missing name"))?;
        let id = item
            .get("call_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!(CALL_ID_MISSING))?
            .to_string();
        if let Some((call, accepted)) = invocation::receive_native_judged(
            name,
            || id.clone(),
            item.get("arguments"),
            self.envelope_clean,
        ) {
            self.refused |= !accepted;
            return Ok(call);
        }
        let arguments = normalize_function_arguments(item.get("arguments").cloned())?;
        Ok(ToolCall {
            id,
            name: name.to_string(),
            arguments: recover_tool_arguments(name, arguments).arguments,
        })
    }

    pub(super) fn refused(&self) -> bool {
        self.refused
    }
}

/// The request's history must correlate before it leaves: every call has a non-empty ID used
/// once, and every tool output answers, once, a call before it.
pub(super) fn check_correlation(messages: &[ConversationMessage]) -> anyhow::Result<()> {
    let mut calls = BTreeSet::new();
    let mut answered = BTreeSet::new();
    for message in messages {
        for call in &message.tool_calls {
            if call.id.is_empty() || !calls.insert(call.id.as_str()) {
                anyhow::bail!(CALL_ID_CORRELATION);
            }
        }
        if message.role == "tool"
            && let Some(id) = message.tool_call_id.as_deref()
            && (!calls.contains(id) || !answered.insert(id))
        {
            anyhow::bail!(CALL_ID_CORRELATION);
        }
    }
    Ok(())
}

fn raw_calls(turn: &StoredTurn) -> impl Iterator<Item = &Value> {
    turn.output.iter().filter(|item| is_function_call(item))
}

/// The stored turn whose calls carry the host message's call IDs, in order. Text alone does not
/// identify a turn: the host clears an assistant's text next to its calls before the request.
fn matching_turn(
    message: &ConversationMessage,
    state: &ConversationState,
    used: &[bool],
) -> Option<usize> {
    if message.tool_calls.is_empty() {
        return None;
    }
    state.turns.iter().enumerate().position(|(index, turn)| {
        !used[index]
            && raw_calls(turn)
                .map(|item| item.get("call_id").and_then(Value::as_str))
                .eq(message.tool_calls.iter().map(|call| Some(call.id.as_str())))
    })
}

/// (b) Each raw call is the host's call: the same ID, name, and decoded arguments, in the same
/// number and order.
fn calls_match(turn: &StoredTurn, message: &ConversationMessage) -> bool {
    raw_calls(turn).count() == message.tool_calls.len()
        && raw_calls(turn)
            .zip(&message.tool_calls)
            .all(|(item, call)| {
                item.get("call_id").and_then(Value::as_str) == Some(call.id.as_str())
                    && item.get("name").and_then(Value::as_str) == Some(call.name.as_str())
                    && normalize_function_arguments(item.get("arguments").cloned())
                        .is_ok_and(|arguments| arguments == call.arguments)
            })
}

/// (c) The scrub of this run leaves every raw item as it is: message text, a reasoning summary,
/// and any field of an unknown item included. Opaque content is compared, never read.
fn survives_scrub(turn: &StoredTurn) -> bool {
    let Some(context) = crate::sensitive_data::current() else {
        return true;
    };
    let original = Value::Array(turn.output.clone());
    let mut scrubbed = original.clone();
    context.scrub_value_free(&mut scrubbed);
    scrubbed == original
}

/// The three conditions under which a turn's raw items may go back whole.
fn replays_raw(turn: &StoredTurn, message: &ConversationMessage) -> bool {
    !turn.refused && calls_match(turn, message) && survives_scrub(turn)
}

/// The input of a request in a session with a schema tool. A turn that may go back raw goes
/// once, with every item in its order; any other assistant turn is rebuilt whole from the
/// host's values, as the parent's fallback builds it.
pub(super) fn input(messages: &[ConversationMessage], state: &ConversationState) -> Vec<Value> {
    let mut input = Vec::new();
    let mut used = vec![false; state.turns.len()];
    for message in messages {
        if message.role != "assistant" {
            push_other(&mut input, message);
            continue;
        }
        let raw = matching_turn(message, state, &used).and_then(|index| {
            used[index] = true;
            let turn = &state.turns[index];
            replays_raw(turn, message).then_some(turn)
        });
        match raw {
            Some(turn) => input.extend(turn.output.iter().cloned()),
            None => push_assistant_fallback(&mut input, message),
        }
    }
    input
}

/// The Responses wire of a session with a schema tool, with a schema tool `T` adopted on this
/// thread. The direct OpenAI client goes to a loopback on 127.0.0.1 only.
#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use serde_json::{Value, json};

    use super::super::{
        ConversationState, build_lm_studio_request, build_request, check_request, parse_response,
        parse_session_response,
    };
    use super::{
        CALL_ID_CORRELATION, CALL_ID_DUPLICATE, CALL_ID_EMPTY, CALL_ID_MISSING,
        CALL_ID_NOT_A_STRING,
    };
    use crate::providers::ChatClient;
    use crate::providers::openai::OpenAiClient;
    use crate::sensitive_data::{RedactionContext, SecretCatalog, set_current};
    use crate::state::{ConversationMessage, ToolCall};
    use crate::tools::profile_tools::schema::invocation::provider_test_support::{
        adopted_schema, receipts, schema_state,
    };
    use crate::tools::profile_tools::schema::invocation::{enter_session, for_provider_worker};
    use crate::tools::registry::{FunctionSpec, ToolSpec};

    const DUMMY_SECRET: &str = "fake-dummy-credential-1005-not-a-real-key";

    fn spec(name: &str, parameters: Value) -> ToolSpec {
        ToolSpec {
            kind: "function".to_string(),
            function: FunctionSpec {
                name: name.to_string(),
                description: "d".to_string(),
                parameters,
            },
        }
    }

    fn specs(schema: Value) -> Vec<ToolSpec> {
        vec![
            spec(
                "Read",
                json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            ),
            spec("T", schema),
        ]
    }

    fn reply(output: Vec<Value>) -> String {
        json!({"id": "resp", "object": "response", "output": output}).to_string()
    }

    fn call(call_id: Value, name: &str, raw: &str) -> Value {
        json!({
            "type": "function_call",
            "id": "fc_item",
            "call_id": call_id,
            "name": name,
            "arguments": raw,
        })
    }

    fn without_call_id(name: &str, raw: &str) -> Value {
        json!({"type": "function_call", "id": "fc_item", "name": name, "arguments": raw})
    }

    fn reasoning(summary: &str) -> Value {
        json!({
            "type": "reasoning",
            "id": "rs_1",
            "summary": [{"type": "summary_text", "text": summary}],
            "encrypted_content": "opaque-dummy-1005",
        })
    }

    fn preamble(text: &str) -> Value {
        json!({
            "type": "message",
            "id": "msg_1",
            "role": "assistant",
            "content": [{"type": "output_text", "text": text}],
        })
    }

    fn read_call(id: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: "Read".to_string(),
            arguments: json!({"path": "a"}),
        }
    }

    #[test]
    fn only_the_schema_tool_says_strict_false_and_a_request_without_one_is_unchanged() {
        let state = schema_state();
        let tools = specs(adopted_schema(&state));
        let messages = [ConversationMessage::user("x")];
        let none = ConversationState::default();
        let off = [
            build_request("m", &messages, &tools, true, 64, Some("high"), &none),
            build_lm_studio_request("m", &messages, &tools, true, 64, &none),
        ];
        for body in &off {
            for tool in body["tools"].as_array().unwrap() {
                assert!(tool.get("strict").is_none(), "{tool}");
            }
        }
        let _scope = enter_session(state.clone());
        let on = [
            build_request("m", &messages, &tools, true, 64, Some("high"), &none),
            build_lm_studio_request("m", &messages, &tools, true, 64, &none),
        ];
        for (off, on) in off.iter().zip(&on) {
            assert_eq!(on["tools"][1]["strict"], json!(false));
            assert_eq!(on["tools"][1]["parameters"], adopted_schema(&state));
            assert!(on["tools"][0].get("strict").is_none());
            let mut without = on.clone();
            without["tools"][1]
                .as_object_mut()
                .unwrap()
                .remove("strict");
            assert_eq!(&without, off);
        }
        // Direct OpenAI keeps its store, include, and effort; the compatible request adds none.
        assert_eq!(on[0]["store"], json!(false));
        assert_eq!(on[0]["include"], json!(["reasoning.encrypted_content"]));
        assert_eq!(on[0]["reasoning"], json!({"effort": "high"}));
        assert_eq!(on[0]["max_output_tokens"], json!(64));
        for key in ["store", "include", "reasoning"] {
            assert!(on[1].get(key).is_none(), "{key}");
        }
    }

    #[test]
    fn a_bad_call_id_fails_the_whole_reply_before_any_receipt() {
        let state = schema_state();
        let history = vec![
            ConversationMessage::user("x"),
            ConversationMessage::assistant("", vec![read_call("held_call")]),
            ConversationMessage::tool_result("Read", Some("held_call"), "text"),
        ];
        let mut stored = ConversationState::default();
        stored.record(
            &parse_response(&reply(vec![call(json!("stored_call"), "Read", "{}")])).unwrap(),
        );
        let good = |id: &str| call(json!(id), "T", r#"{"s":"v"}"#);

        // Off: no schema tool selected, and the parser fills an ID in as it did.
        let off = parse_session_response(
            &reply(vec![without_call_id("T", r#"{"s":"v"}"#)]),
            &history,
            &stored,
        )
        .unwrap();
        assert_eq!(off.reply.tool_calls[0].id, "fc_item");

        let _scope = enter_session(state.clone());
        let cases = [
            (
                CALL_ID_MISSING,
                vec![good("first"), without_call_id("T", "{}")],
            ),
            (
                CALL_ID_NOT_A_STRING,
                vec![good("first"), call(json!(7), "T", "{}")],
            ),
            (CALL_ID_NOT_A_STRING, vec![call(Value::Null, "Read", "{}")]),
            (
                CALL_ID_EMPTY,
                vec![good("first"), call(json!(""), "T", "{}")],
            ),
            (CALL_ID_DUPLICATE, vec![good("same"), good("same")]),
            (
                CALL_ID_DUPLICATE,
                vec![call(json!("same"), "Read", "{}"), good("same")],
            ),
            (
                CALL_ID_DUPLICATE,
                vec![
                    call(json!("same"), "Read", "{}"),
                    call(json!("same"), "Read", "{}"),
                ],
            ),
            (CALL_ID_DUPLICATE, vec![good("held_call")]),
            (CALL_ID_DUPLICATE, vec![good("stored_call")]),
        ];
        for (expected, output) in cases {
            for_provider_worker();
            let error = parse_session_response(&reply(output.clone()), &history, &stored)
                .expect_err(expected);
            assert_eq!(error.to_string(), expected, "{output:?}");
            assert_eq!(receipts(&state), 0, "{output:?}");
        }
        for_provider_worker();
        let parsed = parse_session_response(
            &reply(vec![
                good("new_1"),
                call(json!("new_2"), "Read", r#"{"path":"a"}"#),
                good("new_3"),
            ]),
            &history,
            &stored,
        )
        .unwrap();
        let ids = parsed
            .reply
            .tool_calls
            .iter()
            .map(|call| call.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, ["new_1", "new_2", "new_3"]);
        assert_eq!(receipts(&state), 2, "one receipt per schema call");
        assert!(!parsed.refused);
    }

    #[test]
    fn a_history_that_does_not_correlate_is_refused_before_the_request_leaves() {
        let user = || ConversationMessage::user("x");
        let tool = |id: &str| ConversationMessage::tool_result("Read", Some(id), "text");
        let asked = |id: &str| ConversationMessage::assistant("", vec![read_call(id)]);
        let bad = [
            vec![user(), tool("nobody")],
            vec![user(), asked("a"), tool("a"), tool("a")],
            vec![user(), asked("a"), tool("a"), asked("a")],
            vec![user(), asked("")],
            vec![user(), tool("a"), asked("a")],
        ];
        let good = vec![user(), asked("a"), tool("a"), asked("b"), tool("b")];
        for messages in &bad {
            check_request(messages).unwrap();
        }
        let _scope = enter_session(schema_state());
        for messages in &bad {
            let error = check_request(messages).unwrap_err();
            assert_eq!(error.to_string(), CALL_ID_CORRELATION);
        }
        check_request(&good).unwrap();

        // The client refuses it with no connection.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let mut client = OpenAiClient::for_test_responses("sk-test-not-real", url, None);
        let error = ChatClient::chat(&mut client, "m", &bad[0], &[], true).unwrap_err();
        assert_eq!(error.to_string(), CALL_ID_CORRELATION);
        listener.set_nonblocking(true).unwrap();
        assert!(listener.accept().is_err(), "the request left");

        // The loop retries a provider error naming `function_call` and none of the words of a
        // transport failure, and ends the run at its existing limit.
        let lower = CALL_ID_CORRELATION.to_ascii_lowercase();
        assert!(lower.contains("function_call"));
        for word in [" api failed:", "http", "status", "network", "timeout"] {
            assert!(!lower.contains(word), "{word}");
        }
    }

    /// Serves `replies` in order on 127.0.0.1 and keeps each request body.
    fn loopback(
        replies: Vec<String>,
    ) -> (String, Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let bodies: Arc<Mutex<Vec<Value>>> = Arc::default();
        let kept = bodies.clone();
        let server = std::thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                let mut chunk = [0_u8; 8192];
                let start = loop {
                    let read = stream.read(&mut chunk).unwrap();
                    bytes.extend_from_slice(&chunk[..read]);
                    if let Some(at) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                        break at + 4;
                    }
                };
                let head = String::from_utf8_lossy(&bytes[..start]).to_ascii_lowercase();
                let length = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while bytes.len() < start + length {
                    let read = stream.read(&mut chunk).unwrap();
                    bytes.extend_from_slice(&chunk[..read]);
                }
                kept.lock()
                    .unwrap()
                    .push(serde_json::from_slice(&bytes[start..]).unwrap());
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                )
                .unwrap();
            }
        });
        (url, bodies, server)
    }

    /// The direct OpenAI client's Responses wire, on its test-only loopback client: the schema
    /// tool goes out with `strict: false`, its call comes back through G2 under the call ID, and
    /// the turn goes back raw with its reasoning, through a clone of the client.
    #[test]
    fn the_direct_client_carries_a_schema_call_there_and_back() {
        let state = schema_state();
        let tools = specs(adopted_schema(&state));
        let _scope = enter_session(state.clone());
        let output = vec![
            reasoning("dummy-summary-clean"),
            call(json!("call_1"), "T", r#"{"s":"v"}"#),
        ];
        let (url, bodies, server) =
            loopback(vec![reply(output.clone()), reply(vec![preamble("done")])]);
        let mut client = OpenAiClient::for_test_responses("sk-test-not-real", url, None);
        for_provider_worker();
        let first = ChatClient::chat(
            &mut client,
            "m",
            &[ConversationMessage::user("x")],
            &tools,
            true,
        )
        .unwrap();
        assert_eq!(first.tool_calls[0].id, "call_1");
        assert_eq!(first.tool_calls[0].arguments, json!({"s": "v"}));
        assert_eq!(receipts(&state), 1);
        let messages = [
            ConversationMessage::user("x"),
            ConversationMessage::assistant("", first.tool_calls.clone()),
            ConversationMessage::tool_result("T", Some("call_1"), "the output"),
        ];
        for_provider_worker();
        let mut clone = client.clone();
        let second = ChatClient::chat(&mut clone, "m", &messages, &tools, true).unwrap();
        server.join().unwrap();
        assert_eq!(second.content, "done");

        let bodies = bodies.lock().unwrap();
        assert_eq!(bodies[0]["store"], json!(false));
        assert_eq!(bodies[0]["tools"][1]["strict"], json!(false));
        assert_eq!(bodies[0]["tools"][1]["parameters"], adopted_schema(&state));
        let mut expected = vec![bodies[1]["input"][0].clone()];
        expected.extend(output);
        expected.push(json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "the output",
        }));
        assert_eq!(bodies[1]["input"], Value::Array(expected));
    }

    /// The second request's items after the user's, for a first reply `output` whose call the
    /// host holds with `host_arguments` (the parsed value when `None`).
    fn second_input(output: Vec<Value>, host_arguments: Option<Value>, answer: &str) -> Vec<Value> {
        let user = ConversationMessage::user("x");
        let mut stored = ConversationState::default();
        for_provider_worker();
        let parsed =
            parse_session_response(&reply(output), std::slice::from_ref(&user), &stored).unwrap();
        stored.record(&parsed);
        let mut calls = parsed.reply.tool_calls.clone();
        if let Some(arguments) = host_arguments {
            calls[0].arguments = arguments;
        }
        let id = calls[0].id.clone();
        let messages = [
            user,
            ConversationMessage::assistant("", calls),
            ConversationMessage::tool_result("T", Some(&id), answer),
        ];
        let body = build_lm_studio_request("m", &messages, &[], true, 64, &stored);
        body["input"].as_array().unwrap()[1..].to_vec()
    }

    fn rebuilt(arguments: &str, answer: &str) -> Vec<Value> {
        vec![
            json!({"type": "function_call", "call_id": "call_1", "name": "T", "arguments": arguments}),
            json!({"type": "function_call_output", "call_id": "call_1", "output": answer}),
        ]
    }

    #[test]
    fn a_turn_goes_back_raw_only_when_all_three_conditions_hold() {
        let _scope = enter_session(schema_state());
        let turn = |raw: &str, text: &str| {
            vec![
                reasoning("dummy-summary-clean"),
                preamble(text),
                call(json!("call_1"), "T", raw),
            ]
        };
        let output = |answer: &str| {
            let mut item = json!({"type": "function_call_output", "call_id": "call_1"});
            item["output"] = json!(answer);
            item
        };

        // All three hold: the raw turn, once, in its order.
        let raw = turn(r#"{"s":"v"}"#, "Submitting.");
        let mut expected = raw.clone();
        expected.push(output("out"));
        assert_eq!(second_input(raw, None, "out"), expected);

        // An empty object the schema accepts is no refusal: the flag is the parse's.
        let empty = turn("{}", "Submitting.");
        let mut expected = empty.clone();
        expected.push(output("out"));
        assert_eq!(second_input(empty, None, "out"), expected);

        // (a) G2 refused the call: the host's call with "{}", and nothing raw.
        assert_eq!(
            second_input(turn(r#"{"s":7}"#, "Submitting."), None, "out"),
            rebuilt("{}", "out")
        );
        // (b) The host holds other arguments than the raw call decodes to.
        assert_eq!(
            second_input(
                turn(r#"{"s":"v"}"#, "Submitting."),
                Some(json!({"s": "w"})),
                "out"
            ),
            rebuilt(r#"{"s":"w"}"#, "out")
        );
        // (c) The scrub would change an item: here the preamble.
        let mut catalog = SecretCatalog::new();
        catalog
            .register_credential("DUMMY_API_KEY", DUMMY_SECRET)
            .unwrap();
        set_current(Some(RedactionContext::from_catalog(catalog)));
        let sent = second_input(
            turn(r#"{"s":"v"}"#, &format!("Submitting {DUMMY_SECRET}")),
            None,
            "out",
        );
        set_current(None);
        assert_eq!(sent, rebuilt(r#"{"s":"v"}"#, "out"));

        // A tool message's text goes out as it is: the history fitted it once, when the message
        // was made, and the wire does not fit it again.
        let long = "x ".repeat(2_500);
        let kept = ConversationMessage::tool_result("T", Some("call_1"), long.as_str()).content;
        let sent = second_input(turn(r#"{"s":"v"}"#, "Submitting."), None, &long);
        assert_eq!(sent.last().unwrap()["output"], json!(kept));
    }
}
