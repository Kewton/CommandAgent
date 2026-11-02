//! What a session admitted, on the path from a provider's raw reply to an execution.
//!
//! The parsers that see raw arguments sit inside a `ChatClient`, behind wrappers
//! this stage does not change, so nothing here travels through the client. A session
//! installs its [`ReceiveState`] in a thread-local, as the run's secret scope does,
//! and `provider_call` hands it to its worker the same way. With no schema tool
//! selected nothing is installed, and every function here returns at once.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Map, Value};

use super::super::descriptor::{RegistryBinding, verify_descriptor_pin};
use super::super::{ProfileToolRefusal, RefusalKind};
use super::binding::Adoption;
use super::raw_json::{self, DecodeError, DecodeLimits};
use super::validate::{self, Violation};
use crate::config::Config;
use crate::providers::ChatClient;
use crate::providers::profile_tool_schema::{self, ProfileToolSchemaWire};
use crate::providers::xml_fallback::LocatedToolCall;
use crate::state::ToolCall;
use crate::tools::args_recovery::{ToolArgsRecovery, recover_tool_arguments};
use crate::tools::registry::ToolSpec;

const STAGE_NATIVE: &str = "received_native";
const STAGE_XML: &str = "received_xml";
const STAGE_RECEIVED: &str = "received_call";
const STAGE_EXECUTION: &str = "before_execution";
/// How much a `{"name":…,"arguments":…}` wrapper adds around the arguments.
const WRAPPER_BYTES: usize = 1024;
const ENVELOPE_DEPTH: usize = 128;

/// One tool call as received: the value exactly as decoded, or the refusal.
#[derive(Debug)]
struct Receipt {
    name: String,
    outcome: Result<Value, String>,
}

#[derive(Debug, Default)]
pub(crate) struct Shared {
    in_session: AtomicBool,
    reply_cut: AtomicBool,
    receipts: Mutex<Vec<Receipt>>,
}

impl Shared {
    fn push(&self, name: &str, outcome: Result<Value, String>) {
        self.receipts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Receipt {
                name: name.to_string(),
                outcome,
            });
    }

    fn take(&self, name: &str) -> Option<Result<Value, String>> {
        let mut receipts = self.receipts.lock().unwrap_or_else(PoisonError::into_inner);
        let position = receipts.iter().position(|receipt| receipt.name == name)?;
        Some(receipts.remove(position).outcome)
    }

    fn clear(&self) {
        self.receipts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }
}

/// The check one schema tool carries to its execution boundary.
#[derive(Debug)]
pub(crate) struct SchemaGuard {
    name: String,
    description: String,
    adoption: Arc<Adoption>,
    registry: RegistryBinding,
    descriptor_sha256: String,
    shared: Arc<Shared>,
}

impl SchemaGuard {
    pub(crate) fn published_schema(&self) -> &Value {
        self.adoption.schema().source()
    }

    fn limits(&self) -> DecodeLimits {
        let limits = self.adoption.schema().limits();
        DecodeLimits {
            bytes: limits.argument_bytes,
            depth: limits.depth,
            elements: limits.elements,
            lossless_numbers: true,
        }
    }

    fn verify_binding(&self, stage: &str) -> Result<(), ProfileToolRefusal> {
        verify_descriptor_pin(&self.registry, &self.descriptor_sha256).map_err(|refusal| {
            ProfileToolRefusal::new(refusal.kind, format!("stage={stage} {}", refusal.reason))
        })?;
        self.adoption.verify(stage)
    }

    /// Only a session has receipts. A caller of the public registry gave a value,
    /// not raw text, so it meets the check right before the execution alone.
    pub(crate) fn check_received(&self, arguments: &Value) -> anyhow::Result<()> {
        if !self.shared.in_session.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.verify_binding(STAGE_RECEIVED)?;
        let refused = |data: String| {
            anyhow::Error::new(ProfileToolRefusal::new(RefusalKind::SchemaViolation, data))
        };
        match self.shared.take(&self.name) {
            Some(Ok(value)) if value == *arguments => Ok(()),
            Some(Err(diagnostics)) => Err(refused(diagnostics)),
            Some(Ok(_)) | None => Err(refused(validate::render(
                STAGE_RECEIVED,
                &[violation("arguments_without_receipt")],
            ))),
        }
    }

    /// The pins and the validator, on the value about to run. A provider that
    /// ignored the schema and a direct caller of the registry both stop here.
    pub(crate) fn check_before_execution(&self, arguments: &Value) -> anyhow::Result<()> {
        self.verify_binding(STAGE_EXECUTION)?;
        validate::validate(self.adoption.schema(), arguments).map_err(|violations| {
            anyhow::Error::new(ProfileToolRefusal::new(
                RefusalKind::SchemaViolation,
                validate::render(STAGE_EXECUTION, &violations),
            ))
        })
    }

    /// The prompt line of a turn without a wire schema.
    fn xml_note(&self) -> String {
        format!("\n  Arguments schema: {}", self.adoption.schema().source())
    }

    fn receive(&self, raw: Result<&str, &'static str>, stage: &str) -> ReceivedCall {
        let decoded = raw.map_err(|code| vec![violation(code)]).and_then(|raw| {
            raw_json::decode(raw, self.limits())
                .map_err(|error: DecodeError| vec![violation(error.code())])
        });
        self.finish(decoded, stage)
    }

    fn finish(&self, decoded: Result<Value, Vec<Violation>>, stage: &str) -> ReceivedCall {
        let outcome = decoded
            .and_then(|value| validate::validate(self.adoption.schema(), &value).map(|()| value))
            .map_err(|violations| validate::render(stage, &violations));
        // A refused call stays visible to the loop as an empty object: the raw text
        // is not valid data, and the receipt is what its execution attempt will meet.
        let arguments = outcome
            .as_ref()
            .map_or_else(|_| Value::Object(Map::new()), Value::clone);
        self.shared.push(&self.name, outcome);
        ReceivedCall { arguments }
    }
}

struct ReceivedCall {
    arguments: Value,
}

fn violation(code: &'static str) -> Violation {
    Violation {
        path: "/".to_string(),
        code,
    }
}

/// The schema tools of one admitted set.
#[derive(Debug, Default)]
pub(crate) struct ReceiveState {
    guards: BTreeMap<String, Arc<SchemaGuard>>,
    shared: Arc<Shared>,
}

impl ReceiveState {
    pub(in super::super) fn add(
        &mut self,
        name: &str,
        description: &str,
        adoption: Arc<Adoption>,
        registry: RegistryBinding,
        descriptor_sha256: &str,
    ) -> Arc<SchemaGuard> {
        let guard = Arc::new(SchemaGuard {
            name: name.to_string(),
            description: description.to_string(),
            adoption,
            registry,
            descriptor_sha256: descriptor_sha256.to_string(),
            shared: self.shared.clone(),
        });
        self.guards.insert(name.to_string(), guard.clone());
        guard
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.guards.is_empty()
    }

    /// What must hold before the first provider call, for the client that will
    /// carry these tools. Nothing here is weakened to pass: a schema a shared step
    /// on the way out would change is refused, and that step stays as it is.
    pub(crate) fn preflight(
        &self,
        config: &Config,
        wire: ProfileToolSchemaWire,
    ) -> Result<(), ProfileToolRefusal> {
        if self.guards.is_empty() {
            return Ok(());
        }
        for name in self.guards.keys() {
            if !wire.carries_schema_tools() {
                return Err(ProfileToolRefusal::new(
                    RefusalKind::SchemaUnsupported,
                    format!(
                        "tool `{name}` needs a Chat Completions wire or explicit text on one; this client {}",
                        wire.describe()
                    ),
                ));
            }
        }
        let scrubbed = |name: &str| {
            ProfileToolRefusal::new(
                RefusalKind::SchemaScrubbed,
                format!("tool `{name}` schema would change under the secret scrub"),
            )
        };
        let context = crate::config::install_run_secret_scope(config)
            .map_err(|_| scrubbed("(the secret scope cannot be installed)"))?;
        for (name, guard) in &self.guards {
            let source = guard.adoption.schema().source();
            if !profile_tool_schema::survives_sanitizer(source) {
                return Err(ProfileToolRefusal::new(
                    RefusalKind::SchemaUnsupported,
                    format!("tool `{name}` schema would change under the provider sanitizer"),
                ));
            }
            let mut copy = source.clone();
            context.scrub_value_free(&mut copy);
            let note = guard.xml_note();
            if copy != *source
                || context.scrub_text(&note) != note
                || context.scrub_text(&guard.description) != guard.description
            {
                return Err(scrubbed(name));
            }
        }
        Ok(())
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Arc<ReceiveState>>> = const { RefCell::new(None) };
}

fn current() -> Option<Arc<ReceiveState>> {
    CURRENT.with(|current| current.borrow().clone())
}

fn with_current<T>(read: impl FnOnce(&ReceiveState) -> Option<T>) -> Option<T> {
    CURRENT.with(|current| current.borrow().as_deref().and_then(read))
}

/// Holds a session's state on its thread, and puts the previous one back.
#[derive(Debug)]
pub(crate) struct SessionScope {
    previous: Option<Arc<ReceiveState>>,
}

impl Drop for SessionScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|current| *current.borrow_mut() = previous);
    }
}

pub(crate) fn enter_session(state: Arc<ReceiveState>) -> SessionScope {
    state.shared.in_session.store(true, Ordering::SeqCst);
    SessionScope {
        previous: CURRENT.with(|current| current.borrow_mut().replace(state)),
    }
}

/// A new provider call starts a new turn. Its worker thread gets the same state.
pub(crate) fn for_provider_worker() -> Option<Arc<ReceiveState>> {
    let state = current()?;
    state.shared.clear();
    state.shared.reply_cut.store(false, Ordering::SeqCst);
    Some(state)
}

pub(crate) fn install_on_provider_worker(state: Option<Arc<ReceiveState>>) {
    CURRENT.with(|current| *current.borrow_mut() = state);
}

/// Right before a request leaves: the copy after the scrub must still be the adopted
/// schema, and a client built for this one call must carry the wire.
pub(crate) fn check_send(
    tools: &[ToolSpec],
    overriding_client: Option<&dyn ChatClient>,
) -> anyhow::Result<()> {
    let Some(state) = current() else {
        return Ok(());
    };
    for tool in tools {
        let Some(guard) = state.guards.get(&tool.function.name) else {
            continue;
        };
        if tool.function.parameters != *guard.adoption.schema().source() {
            return Err(ProfileToolRefusal::new(
                RefusalKind::SchemaScrubbed,
                format!(
                    "tool `{}` schema changed on its way out of the host",
                    guard.name
                ),
            )
            .into());
        }
        if let Some(client) = overriding_client
            && !client.profile_tool_schema_wire().carries_schema_tools()
        {
            return Err(ProfileToolRefusal::new(
                RefusalKind::SchemaUnsupported,
                format!(
                    "tool `{}` cannot be sent on an overriding client",
                    guard.name
                ),
            )
            .into());
        }
    }
    Ok(())
}

fn is_schema_tool(name: &str) -> bool {
    with_current(|state| state.guards.contains_key(name).then_some(())).is_some()
}

/// The shared recovery reshapes arguments, so a schema tool does not take it.
pub(crate) fn recover_unless_schema_tool(name: &str, arguments: Value) -> ToolArgsRecovery {
    if is_schema_tool(name) {
        return ToolArgsRecovery {
            arguments,
            changed: false,
            changes: Vec::new(),
        };
    }
    recover_tool_arguments(name, arguments)
}

/// A selected schema tool turns the native-to-XML fallback off: a refused call must
/// not change how the rest of the session talks.
pub(crate) fn blocks_xml_fallback() -> bool {
    with_current(|state| (!state.is_empty()).then_some(())).is_some()
}

/// A `serde_json::Value` of the whole body has already lost a repeated key around
/// the arguments. Checked only when a schema tool is selected, on a body that has
/// already parsed, so `serde_json`'s own recursion limit bounds the depth.
pub(crate) fn envelope_is_clean(body: &str) -> bool {
    if current().is_none() {
        return true;
    }
    raw_json::decode(
        body,
        DecodeLimits {
            bytes: usize::MAX,
            depth: ENVELOPE_DEPTH,
            elements: usize::MAX,
            lossless_numbers: false,
        },
    )
    .err()
        != Some(DecodeError::DuplicateKey)
}

/// A native call to a schema tool: its raw `arguments` text, strictly. `None` for
/// any other call, which keeps the existing parser. A refusal never fails the parse,
/// because a parse error is what starts the XML fallback.
pub(crate) fn receive_native(
    name: &str,
    id: impl FnOnce() -> String,
    raw: Option<&Value>,
    envelope_clean: bool,
) -> Option<ToolCall> {
    let guard = with_current(|state| state.guards.get(name).cloned())?;
    let raw = match raw.map(Value::as_str) {
        _ if !envelope_clean => Err("duplicate_key"),
        Some(Some(raw)) => Ok(raw),
        _ => Err("malformed_json"),
    };
    let received = guard.receive(raw, STAGE_NATIVE);
    Some(ToolCall {
        id: id(),
        name: name.to_string(),
        arguments: received.arguments,
    })
}

/// An XML call to a schema tool. The body goes through neither the relaxed JSON nor
/// the wrapper rules: with a named tag the body is the arguments, and a plain tag
/// holds exactly `{"name":…,"arguments":…}`. `None` for any other tool.
pub(crate) fn receive_xml(raw: &str, default_name: Option<&str>) -> Option<ToolCall> {
    let state = current()?;
    let (guard, received) = match default_name {
        Some(name) => {
            let guard = state.guard_for_written_name(name)?;
            let raw = if name == guard.name {
                Ok(raw)
            } else {
                Err("tool_name_not_exact")
            };
            let received = guard.receive(raw, STAGE_XML);
            (guard, received)
        }
        None => state.receive_plain_tag(raw)?,
    };
    Some(ToolCall::new(guard.name.clone(), received.arguments))
}

impl ReceiveState {
    fn guard_for_written_name(&self, name: &str) -> Option<Arc<SchemaGuard>> {
        self.guards
            .iter()
            .find(|(admitted, _)| admitted.eq_ignore_ascii_case(name))
            .map(|(_, guard)| guard.clone())
    }

    fn guard_mentioned_in(&self, raw: &str) -> Option<Arc<SchemaGuard>> {
        let lower = raw.to_ascii_lowercase();
        self.guards
            .iter()
            .find(|(admitted, _)| lower.contains(&admitted.to_ascii_lowercase()))
            .map(|(_, guard)| guard.clone())
    }

    fn receive_plain_tag(&self, raw: &str) -> Option<(Arc<SchemaGuard>, ReceivedCall)> {
        let widest = self.guards.values().map(|guard| guard.limits()).fold(
            DecodeLimits {
                bytes: 0,
                depth: 0,
                elements: 0,
                lossless_numbers: true,
            },
            |widest, limits| DecodeLimits {
                bytes: widest.bytes.max(limits.bytes + WRAPPER_BYTES),
                depth: widest.depth.max(limits.depth + 1),
                elements: widest.elements.max(limits.elements + 2),
                lossless_numbers: true,
            },
        );
        match raw_json::decode(raw, widest) {
            Ok(wrapper) => {
                let written = wrapper.get("name").and_then(Value::as_str);
                let guard = match written {
                    Some(name) => self.guard_for_written_name(name)?,
                    None => self.guard_mentioned_in(raw)?,
                };
                let decoded = plain_tag_arguments(&wrapper, written, &guard.name);
                let received = guard.finish(decoded, STAGE_XML);
                Some((guard, received))
            }
            Err(error) => {
                let guard = self.guard_mentioned_in(raw)?;
                let received = guard.finish(Err(vec![violation(error.code())]), STAGE_XML);
                Some((guard, received))
            }
        }
    }
}

fn plain_tag_arguments(
    wrapper: &Value,
    written: Option<&str>,
    admitted: &str,
) -> Result<Value, Vec<Violation>> {
    let shape = wrapper.as_object().filter(|members| members.len() == 2);
    match (shape.and_then(|members| members.get("arguments")), written) {
        (Some(arguments), Some(name)) if name == admitted => Ok(arguments.clone()),
        (Some(_), Some(_)) => Err(vec![violation("tool_name_not_exact")]),
        _ => Err(vec![violation("wrapper_not_exact")]),
    }
}

/// Whether the calls a parser produced include a schema tool. This reads the decoded
/// names, so a name written with a JSON escape counts, and a name mentioned in text
/// does not.
pub(crate) fn has_schema_call<'a>(calls: impl IntoIterator<Item = &'a ToolCall>) -> bool {
    calls.into_iter().any(|call| is_schema_tool(&call.name))
}

/// A parse that did not end in a schema call leaves nothing to execute. What it or a
/// discarded repair candidate received must not stand in for a later call.
pub(crate) fn forget_receipts() {
    if let Some(state) = current() {
        state.shared.clear();
    }
}

/// An old-path parse named a schema tool: a body the strict decode did not take, such
/// as an unexact wrapper whose name was written with an escape.
pub(crate) fn refuse_unstrict_xml(name: &str) -> Option<ToolCall> {
    let guard = with_current(|state| state.guards.get(name).cloned())?;
    let received = guard.finish(Err(vec![violation("wrapper_not_exact")]), STAGE_XML);
    Some(ToolCall::new(guard.name.clone(), received.arguments))
}

/// The requirement plan cut a block out of this reply's text before the XML parse, so
/// what the parse read is not what was sent.
pub(crate) fn note_reply_cut_before_parse() {
    if let Some(state) = current() {
        state.shared.reply_cut.store(true, Ordering::SeqCst);
    }
}

pub(crate) fn watches_reply_text() -> bool {
    current().is_some()
}

/// In a reply that produced a schema call, every call, a builtin's too, must have been read
/// from consecutive text of the reply: the parser removed nothing inside its tag. This reads
/// the parser's own record, so it cannot differ from what the parser did. The requirement
/// plan's cut comes before the parser, and a `<think>` block outside the complete ones ahead
/// of everything else is refused as well.
pub(crate) fn check_xml_reply(text: &str, calls: &[LocatedToolCall]) -> anyhow::Result<()> {
    if !has_schema_call(calls.iter().map(|located| &located.call)) {
        return Ok(());
    }
    let cut =
        with_current(|state| state.shared.reply_cut.load(Ordering::SeqCst).then_some(())).is_some();
    if cut || think_block_outside_the_lead(text) || calls.iter().any(|located| located.spliced) {
        return Err(ProfileToolRefusal::new(
            RefusalKind::SchemaViolation,
            validate::render(STAGE_XML, &[violation("tag_inside_arguments")]),
        )
        .into());
    }
    Ok(())
}

fn think_block_outside_the_lead(text: &str) -> bool {
    after_leading_think_blocks(text).contains("<think>")
}

fn after_leading_think_blocks(text: &str) -> &str {
    let mut rest = text.trim_start();
    while let Some((_, after)) = rest
        .strip_prefix("<think>")
        .and_then(|inner| inner.split_once("</think>"))
    {
        rest = after.trim_start();
    }
    rest
}

/// What the XML prompt adds for a schema tool. Empty for every other tool.
pub(crate) fn xml_prompt_note(name: &str) -> String {
    with_current(|state| state.guards.get(name).map(|guard| guard.xml_note())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::think_block_outside_the_lead;

    #[test]
    fn only_complete_think_blocks_ahead_of_everything_are_allowed() {
        for text in [
            "<tool_call>{}</tool_call>",
            "  <think>a</think>\n<think>b</think> <tool_call>{}</tool_call>",
            "<function=T>{}</function><tool_call>{}</tool_call>",
            "<TOOL_CALL><tool_call></tool_call></TOOL_CALL>",
            "<THINK>a</THINK><tool_call>{}</tool_call>",
        ] {
            assert!(!think_block_outside_the_lead(text), "{text}");
        }
        for text in [
            "<tool_call>{\"s\":\"<think>a</think>\"}</tool_call>",
            "text <think>a</think> <tool_call>{}</tool_call>",
            "<think>never closed <tool_call>{}</tool_call>",
            "<think>a</think><think>never closed",
            "<tool_call>{}</tool_call><think>later</think>",
        ] {
            assert!(think_block_outside_the_lead(text), "{text}");
        }
    }
}

/// Every order of the pieces that can confuse the XML parser, inside and around a schema
/// call. Whatever the parser does, a value that would run must be one the reply holds as
/// consecutive text. The enumeration is fixed: there is no randomness.
#[cfg(test)]
mod exhaustive_tests {
    use std::path::Path;
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::super::super::descriptor::RegistryBinding;
    use super::super::binding::{ToolPins, adopt};
    use super::super::raw_json::{self, DecodeLimits};
    use super::{ReceiveState, enter_session};
    use crate::minimal_loop::tool_protocol::normalize_text_reply;
    use crate::providers::AssistantReply;
    use crate::tools::registry::{FunctionSpec, ToolSpec};

    const TOOL: &str = "T";
    /// A complete builtin call.
    const BUILTIN: &str =
        "<anvil_tool_call>{'name':'Read','arguments':{'path':'notes.txt'}}</anvil_tool_call>";
    /// Everything the XML parser treats specially, and the spellings it does not read.
    const ALL_PIECES: [&str; 25] = [
        "<anvil_tool_call>",
        "<anvil_tool_call name=\"T\">",
        "<tool_call>",
        "<function_call>",
        "<function name=\"T\">",
        "<function=T>",
        "</anvil_tool_call>",
        "</tool_call>",
        "</function_call>",
        "</function>",
        "<TOOL_CALL>",
        "</TOOL_CALL>",
        "<FUNCTION=T>",
        "</FUNCTION>",
        "<think>",
        "</think>",
        "<to",
        "ol_call>",
        "<func",
        "tion=T>",
        "</tool",
        "_call>",
        "</func",
        "tion>",
        BUILTIN,
    ];
    /// The length 4 set: each kind of confusion once, to keep the run short.
    const CORE_PIECES: [&str; 10] = [
        "<anvil_tool_call>",
        "</anvil_tool_call>",
        "<tool_call>",
        "</tool_call>",
        "</TOOL_CALL>",
        "<think>",
        "</think>",
        "<to",
        "ol_call>",
        BUILTIN,
    ];

    /// A schema call in pieces, each gap one place a piece can go.
    struct Template {
        open: (&'static str, &'static str),
        before_value: &'static str,
        after_value: &'static str,
        close: (&'static str, &'static str),
    }

    const TEMPLATES: [Template; 2] = [
        Template {
            open: ("<to", "ol_call>"),
            before_value: "{\"name\":\"T\",\"arguments\":{\"s\":\"",
            after_value: "\"}}",
            close: ("</tool", "_call>"),
        },
        Template {
            open: ("<func", "tion=T>"),
            before_value: "{\"s\":\"",
            after_value: "\"}",
            close: ("</func", "tion>"),
        },
    ];

    const SLOTS: usize = 5;

    /// Every way to give `length` pieces, in order, to the slots.
    fn compositions(length: usize) -> Vec<[usize; SLOTS]> {
        fn fill(
            slot: usize,
            left: usize,
            current: &mut [usize; SLOTS],
            out: &mut Vec<[usize; SLOTS]>,
        ) {
            if slot == SLOTS - 1 {
                current[slot] = left;
                out.push(*current);
                return;
            }
            for here in 0..=left {
                current[slot] = here;
                fill(slot + 1, left - here, current, out);
            }
        }
        let mut out = Vec::new();
        fill(0, length, &mut [0; SLOTS], &mut out);
        out
    }

    fn render(template: &Template, pieces: &[&str], slots: &[usize; SLOTS]) -> String {
        let mut text = String::new();
        let mut next = 0;
        let mut take = |text: &mut String, slot: usize| {
            for piece in &pieces[next..next + slots[slot]] {
                text.push_str(piece);
            }
            next += slots[slot];
        };
        take(&mut text, 0);
        text.push_str(template.open.0);
        take(&mut text, 1);
        text.push_str(template.open.1);
        text.push_str(template.before_value);
        take(&mut text, 2);
        text.push_str(template.after_value);
        text.push_str(template.close.0);
        take(&mut text, 3);
        text.push_str(template.close.1);
        take(&mut text, 4);
        text
    }

    fn holds_as_consecutive_text(reply: &str, value: &Value) -> bool {
        if reply.contains(&value.to_string()) {
            return true;
        }
        let limits = DecodeLimits {
            bytes: usize::MAX,
            depth: 32,
            elements: 512,
            lossless_numbers: true,
        };
        reply.match_indices('{').any(|(start, _)| {
            reply[start..].match_indices('}').any(|(end, _)| {
                raw_json::decode(&reply[start..=start + end], limits).as_ref() == Ok(value)
            })
        })
    }

    fn tool_spec(name: &str) -> ToolSpec {
        ToolSpec {
            kind: "function".to_string(),
            function: FunctionSpec {
                name: name.to_string(),
                description: "d".to_string(),
                parameters: json!({"type": "object"}),
            },
        }
    }

    struct Sweep {
        state: Arc<ReceiveState>,
        specs: Vec<ToolSpec>,
        replies: usize,
        executed: usize,
        failures: Vec<String>,
    }

    impl Sweep {
        fn new(workspace: &Path) -> Self {
            let schema = json!({
                "protocol": "profile-tool.schema/v1",
                "root": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {"s": {"type": "string"}},
                },
            });
            let pins = ToolPins {
                tool_id: "exhaustive",
                version: "1",
                descriptor_sha256: "pin",
                max_input_bytes: 1 << 20,
            };
            let adoption = adopt(&schema, &pins, workspace, "exhaustive").unwrap();
            let mut state = ReceiveState::default();
            state.add(
                TOOL,
                "d",
                adoption,
                RegistryBinding::unreachable_for_test(),
                "pin",
            );
            Self {
                state: Arc::new(state),
                specs: vec![tool_spec(TOOL), tool_spec("Read")],
                replies: 0,
                executed: 0,
                failures: Vec::new(),
            }
        }

        fn check(&mut self, text: &str) {
            self.replies += 1;
            self.state.shared.clear();
            let mut reply = AssistantReply::text(text);
            if normalize_text_reply(&mut reply, &self.specs).is_err() {
                return;
            }
            for call in reply.tool_calls.iter().filter(|call| call.name == TOOL) {
                let Some(Ok(value)) = self.state.shared.take(TOOL) else {
                    continue;
                };
                self.executed += 1;
                if value != call.arguments || !holds_as_consecutive_text(text, &value) {
                    self.failures.push(format!("{value} from {text}"));
                }
            }
        }

        fn sweep(&mut self, pieces: &[&str], length: usize) {
            let compositions = compositions(length);
            let mut indices = vec![0usize; length];
            let mut chosen = vec![""; length];
            loop {
                for (slot, index) in indices.iter().enumerate() {
                    chosen[slot] = pieces[*index];
                }
                for template in &TEMPLATES {
                    for slots in &compositions {
                        self.check(&render(template, &chosen, slots));
                    }
                }
                let Some(position) = indices.iter().rposition(|index| index + 1 < pieces.len())
                else {
                    return;
                };
                indices[position] += 1;
                indices[position + 1..].fill(0);
            }
        }
    }

    #[test]
    fn no_order_of_the_pieces_runs_a_value_the_reply_does_not_hold() {
        let workspace = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(workspace.path()).unwrap();
        let mut sweep = Sweep::new(&root);
        let _scope = enter_session(sweep.state.clone());
        let started = std::time::Instant::now();
        sweep.sweep(&ALL_PIECES, 3);
        let at_three = sweep.replies;
        sweep.sweep(&CORE_PIECES, 4);
        println!(
            "exhaustive: length 3 over {} pieces: {at_three} replies; length 4 over {} pieces: {} replies; {} values would run; {} failures; {:?}",
            ALL_PIECES.len(),
            CORE_PIECES.len(),
            sweep.replies - at_three,
            sweep.executed,
            sweep.failures.len(),
            started.elapsed(),
        );
        // The sweep measures something: some values do run.
        assert!(sweep.executed > 0);
        assert!(
            sweep.failures.is_empty(),
            "{} replies ran a value they do not hold:\n{}",
            sweep.failures.len(),
            sweep.failures.join("\n")
        );
    }
}
