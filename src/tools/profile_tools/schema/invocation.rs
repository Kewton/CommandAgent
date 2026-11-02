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
pub(crate) fn has_schema_call(calls: &[ToolCall]) -> bool {
    calls.iter().any(|call| is_schema_tool(&call.name))
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

/// Everything before an XML body is read can change a string inside it: the removal
/// of a `<think>` block, the extraction of an inner tag, and the requirement plan's
/// cut. A reply that produced a schema call is refused as a whole if any of these
/// touched it, apart from complete `<think>` blocks ahead of everything else.
pub(crate) fn check_xml_reply(text: &str, calls: &[ToolCall]) -> anyhow::Result<()> {
    if !has_schema_call(calls) {
        return Ok(());
    }
    let cut =
        with_current(|state| state.shared.reply_cut.load(Ordering::SeqCst).then_some(())).is_some();
    if cut || reply_text_can_change(text) {
        return Err(ProfileToolRefusal::new(
            RefusalKind::SchemaViolation,
            validate::render(STAGE_XML, &[violation("tag_inside_arguments")]),
        )
        .into());
    }
    Ok(())
}

fn reply_text_can_change(text: &str) -> bool {
    let rest = after_leading_think_blocks(text);
    rest.contains("<think>") || open_tag_depth_exceeds_one(&rest.to_ascii_lowercase())
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

fn open_tag_depth_exceeds_one(lower: &str) -> bool {
    const OPEN: [&str; 4] = [
        "<anvil_tool_call",
        "<tool_call",
        "<function_call",
        "<function",
    ];
    const CLOSE: [&str; 4] = [
        "</anvil_tool_call",
        "</tool_call",
        "</function_call",
        "</function",
    ];
    let mut depth = 0usize;
    for (index, _) in lower.match_indices('<') {
        let rest = &lower[index..];
        if OPEN.iter().any(|tag| rest.starts_with(tag)) {
            depth += 1;
            if depth > 1 {
                return true;
            }
        } else if CLOSE.iter().any(|tag| rest.starts_with(tag)) {
            depth = depth.saturating_sub(1);
        }
    }
    false
}

/// What the XML prompt adds for a schema tool. Empty for every other tool.
pub(crate) fn xml_prompt_note(name: &str) -> String {
    with_current(|state| state.guards.get(name).map(|guard| guard.xml_note())).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::reply_text_can_change;

    #[test]
    fn only_complete_think_blocks_ahead_of_everything_leave_a_reply_unchanged() {
        for text in [
            "<tool_call>{}</tool_call>",
            "  <think>a</think>\n<think>b</think> <tool_call>{}</tool_call>",
            "<function=T>{}</function><tool_call>{}</tool_call>",
        ] {
            assert!(!reply_text_can_change(text), "{text}");
        }
        for text in [
            "<tool_call>{\"s\":\"<think>a</think>\"}</tool_call>",
            "text <think>a</think> <tool_call>{}</tool_call>",
            "<think>never closed <tool_call>{}</tool_call>",
            "<think>a</think><think>never closed",
            "<anvil_tool_call name=\"T\">{\"s\":\"<tool_call>\"}</anvil_tool_call>",
            "<TOOL_CALL><tool_call></tool_call></TOOL_CALL>",
        ] {
            assert!(reply_text_can_change(text), "{text}");
        }
    }
}
