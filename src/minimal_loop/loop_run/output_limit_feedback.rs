//! One fixed recovery sentence after a reply that hit the output limit
//! (Issue #958).
//!
//! Two observations arm the sentence:
//!
//! - a reply without tool calls whose `completion_tokens` reached
//!   `num_predict` (`num_predict > 0`), the same rule as the
//!   `output_limit_reached` field of `provider_turn_duration`;
//! - a native tool-call parse error that carries the typed
//!   [`ToolArgumentsParseError`] context with the end-of-input
//!   classification and a reported completion count at or above
//!   `num_predict` (`num_predict > 0`), seen while the loop already retries
//!   the malformed call natively or switches to XML fallback and sends the
//!   next request. The error text is never read.
//!
//! Neither observation changes what the loop decides. The armed sentence is
//! kept here, apart from the loop's pending feedback, so a later rewrite or
//! clearing of that feedback (the empty-response stages, the fresh-session
//! retry) does not drop it. Right before the next request is built, the
//! sentence is appended once to whatever feedback that request carries, and
//! the `output_limit_feedback` event is written only after the provider call
//! shows that request reached the client. When no request follows (the last
//! iteration, a max-length guard stop, an exhausted malformed retry that
//! ends the run without XML fallback, an accepted final), nothing is added
//! and nothing is written.
//!
//! The request that carries the sentence has one dispatch state per provider
//! call, which moves once: `Pending → Dispatched` (the watch, right before it
//! asks the client) or `Pending → Closed` (the run, once the provider call
//! returned). The event is written exactly when the request was dispatched,
//! including a call interrupted after that; a call that returned first
//! writes nothing and arms the sentence again for the next request.
//!
//! Cancel and timeout behavior change, limited to the request that carries
//! the sentence: when the provider call has already returned (cancelled or
//! timed out) before its worker reaches the client, the worker no longer
//! sends that request late; it returns the interruption error instead.
//! Every other request behaves exactly as before, a late worker included.
//!
//! Known limit: this state is separate from the Issue #957 notice's. When
//! one request of a `COMMANDAGENT_BUDGET_CLOSING=once` run carries both, and
//! the call returns first while its late worker sits between the #957 watch
//! and this one, the request is not sent but the #957 notice is recorded;
//! this sentence is armed again for the next request. Closing that needs one
//! shared state, outside this Issue.
//!
//! The event carries only counts and fixed words: `kind`, `attempt` (the
//! requests of that kind actually sent), `num_predict`, `completion_tokens`,
//! `prompt_version`, and `provider_reasoning_tokens` when the provider
//! reported it for the very turn that hit the limit. A parse-error turn has
//! no metadata, so its reasoning count stays unreported, never zero.

use std::borrow::Cow;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use crate::eval_events;
use crate::providers::streaming::CancelToken;
use crate::providers::{
    AssistantReply, ChatClient, ProviderResponseMetadata, ResponseTiming, ToolArgumentsParseError,
    ToolArgumentsParseFailure,
};
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

const PROMPT_VERSION: u64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    NoToolCall,
    ToolArgumentsTruncated,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::NoToolCall => "no_tool_call_at_output_limit",
            Self::ToolArgumentsTruncated => "tool_arguments_truncated_at_output_limit",
        }
    }

    fn index(self) -> usize {
        match self {
            Self::NoToolCall => 0,
            Self::ToolArgumentsTruncated => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Observation {
    kind: Kind,
    num_predict: usize,
    completion_tokens: u64,
    reasoning_tokens: Option<u64>,
}

/// The fixed sentence (prompt version 1) for one observation.
fn guidance(observation: &Observation) -> String {
    let what = match observation.kind {
        Kind::NoToolCall => "before any tool call arrived",
        Kind::ToolArgumentsTruncated => "and its tool call arguments were cut off",
    };
    format!(
        "Your previous response reached the output limit ({} tokens) {what}. Keep your reasoning short, start your next response with one tool call, and write a large file in smaller parts.",
        observation.num_predict
    )
}

/// Whether the request that carries the sentence reached the client. It
/// moves once, either `Pending → Dispatched` (the watch, right before it
/// asks the client) or `Pending → Closed` ([`OutputLimitFeedback::settle`],
/// once the provider call returned).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Dispatch {
    #[default]
    Pending,
    Dispatched,
    Closed,
}

/// What the client saw during one provider call.
#[derive(Debug, Default)]
struct CallState {
    dispatch: Dispatch,
    reasoning_tokens: Option<u64>,
}

/// The error a watch returns instead of asking the client once the run has
/// closed the request that carries the sentence; it is the provider call's
/// own interruption text, the same one `budget_closing` returns.
const CLOSED_ERROR: &str = "aborted_by_user: interrupted by user";

type SharedCall = Arc<Mutex<CallState>>;

fn lock(call: &SharedCall) -> std::sync::MutexGuard<'_, CallState> {
    call.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) struct OutputLimitFeedback {
    events_path: Option<PathBuf>,
    pending: Option<Observation>,
    staged: Option<Observation>,
    call: SharedCall,
    attempts: [u64; 2],
    #[cfg(test)]
    dispatch_gate: Option<Arc<tests::DispatchGate>>,
}

impl OutputLimitFeedback {
    pub(super) fn new(events_path: Option<&Path>) -> Self {
        Self {
            events_path: events_path.map(Path::to_path_buf),
            pending: None,
            staged: None,
            call: SharedCall::default(),
            attempts: [0; 2],
            #[cfg(test)]
            dispatch_gate: None,
        }
    }

    /// A parsed reply. Arms the sentence for a reply without tool calls whose
    /// completion count reached `num_predict`.
    pub(super) fn observe_reply(&mut self, reply: &AssistantReply, num_predict: usize) {
        if !reply.tool_calls.is_empty() {
            return;
        }
        let Some(completion_tokens) = reached(reply.completion_tokens, num_predict) else {
            return;
        };
        self.pending = Some(Observation {
            kind: Kind::NoToolCall,
            num_predict,
            completion_tokens,
            reasoning_tokens: lock(&self.call).reasoning_tokens,
        });
    }

    /// A provider error the loop is about to retry as a malformed native
    /// tool call. Arms the sentence only for the typed end-of-input context
    /// at the limit; any other error, typed or not, leaves it unarmed.
    pub(super) fn observe_malformed(&mut self, error: &anyhow::Error, num_predict: usize) {
        let Some(context) = error
            .chain()
            .find_map(|cause| cause.downcast_ref::<ToolArgumentsParseError>())
        else {
            return;
        };
        if context.failure() != ToolArgumentsParseFailure::EndOfInput {
            return;
        }
        let Some(completion_tokens) = reached(context.completion_tokens(), num_predict) else {
            return;
        };
        self.pending = Some(Observation {
            kind: Kind::ToolArgumentsTruncated,
            num_predict,
            completion_tokens,
            reasoning_tokens: None,
        });
    }

    /// The feedback of the request about to be built: unchanged when nothing
    /// is armed, otherwise that feedback with the sentence appended once
    /// (or the sentence alone). The sentence moves from armed to staged.
    pub(super) fn compose<'a>(&mut self, feedback: Option<Cow<'a, str>>) -> Option<Cow<'a, str>> {
        let Some(observation) = self.pending.take() else {
            return feedback;
        };
        let sentence = guidance(&observation);
        self.staged = Some(observation);
        Some(Cow::Owned(match feedback {
            Some(feedback) => format!("{feedback}\n\n{sentence}"),
            None => sentence,
        }))
    }

    /// The client for the next provider call. Each call gets a fresh record,
    /// so a late worker of an earlier call can never write into it. Only the
    /// request that carries the sentence is guarded by the dispatch state.
    pub(super) fn watch<C>(&mut self, inner: C) -> OutputLimitWatch<C> {
        self.call = SharedCall::default();
        OutputLimitWatch {
            inner,
            call: Arc::clone(&self.call),
            guarded: self.staged.is_some(),
            #[cfg(test)]
            gate: self.dispatch_gate.clone(),
        }
    }

    /// After the provider call returned. For the request that carries the
    /// sentence, the call's dispatch state moves once: when the watch has
    /// already taken it to dispatched, the sentence is recorded (also for a
    /// call interrupted after that); otherwise this closes it, the sentence
    /// is armed again for the next request, and a worker that reaches the
    /// watch later returns the interruption error without asking the client.
    pub(super) fn settle(&mut self) {
        let Some(observation) = self.staged.take() else {
            return;
        };
        {
            let mut call = lock(&self.call);
            if call.dispatch == Dispatch::Pending {
                call.dispatch = Dispatch::Closed;
                drop(call);
                self.pending = Some(observation);
                return;
            }
        }
        let attempt = &mut self.attempts[observation.kind.index()];
        *attempt += 1;
        let mut event = json!({
            "event": "output_limit_feedback",
            "kind": observation.kind.as_str(),
            "attempt": *attempt,
            "num_predict": observation.num_predict,
            "completion_tokens": observation.completion_tokens,
            "prompt_version": PROMPT_VERSION,
        });
        if let Some(reasoning) = observation.reasoning_tokens {
            event["provider_reasoning_tokens"] = json!(reasoning);
        }
        eval_events::emit(self.events_path.as_deref(), event);
    }
}

fn reached(completion_tokens: Option<u64>, num_predict: usize) -> Option<u64> {
    let limit = u64::try_from(num_predict).ok().filter(|limit| *limit > 0)?;
    completion_tokens.filter(|count| *count >= limit)
}

/// The client handed to one provider call. It forwards every method and
/// notes, in the call's own record, that the request reached the client and
/// the reasoning count of that call's metadata.
///
/// For the request that carries the sentence (`guarded`), `chat` and
/// `chat_stream` first take the dispatch state from pending to dispatched;
/// when [`OutputLimitFeedback::settle`] already closed it, they return the
/// interruption error without asking the client. Every other request is
/// forwarded exactly as before, a late worker included.
pub(super) struct OutputLimitWatch<C> {
    inner: C,
    call: SharedCall,
    guarded: bool,
    #[cfg(test)]
    gate: Option<Arc<tests::DispatchGate>>,
}

impl<C> OutputLimitWatch<C> {
    /// False only for a guarded request the run already closed.
    fn dispatch(&self) -> bool {
        #[cfg(test)]
        if let Some(gate) = &self.gate {
            gate.hold();
        }
        let proceed = {
            let mut call = lock(&self.call);
            let proceed = !self.guarded || call.dispatch != Dispatch::Closed;
            if proceed {
                call.dispatch = Dispatch::Dispatched;
                call.reasoning_tokens = None;
            }
            proceed
        };
        #[cfg(test)]
        if let Some(gate) = &self.gate {
            gate.decided(proceed);
        }
        proceed
    }
}

impl<C> ChatClient for OutputLimitWatch<C>
where
    C: DerefMut + Send,
    C::Target: ChatClient,
{
    fn label(&self) -> &str {
        self.inner.label()
    }

    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(OutputLimitWatch {
            inner: self.inner.boxed_clone(),
            call: Arc::clone(&self.call),
            guarded: self.guarded,
            #[cfg(test)]
            gate: self.gate.clone(),
        })
    }

    fn supports_native_tools(&self, model: &str) -> bool {
        self.inner.supports_native_tools(model)
    }

    fn allows_xml_fallback(&self) -> bool {
        self.inner.allows_xml_fallback()
    }

    fn parses_xml_tool_calls_before_host(&self) -> bool {
        self.inner.parses_xml_tool_calls_before_host()
    }

    fn supports_ollama_think(&self) -> bool {
        self.inner.supports_ollama_think()
    }

    fn take_response_timing(&mut self) -> Option<ResponseTiming> {
        self.inner.take_response_timing()
    }

    fn take_response_metadata(&mut self) -> Option<ProviderResponseMetadata> {
        let metadata = self.inner.take_response_metadata();
        lock(&self.call).reasoning_tokens = metadata
            .as_ref()
            .and_then(|metadata| metadata.reasoning_tokens);
        metadata
    }

    fn set_cancel_token(&mut self, token: CancelToken) {
        self.inner.set_cancel_token(token);
    }

    fn provider_turn_carried_result(&mut self) {
        self.inner.provider_turn_carried_result();
    }

    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }

    fn supports_streaming_for_model(&self, model: &str) -> bool {
        self.inner.supports_streaming_for_model(model)
    }

    fn chat_stream(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
        on_chunk: &mut dyn FnMut(&str) -> anyhow::Result<()>,
    ) -> anyhow::Result<AssistantReply> {
        if !self.dispatch() {
            anyhow::bail!(CLOSED_ERROR);
        }
        self.inner
            .chat_stream(model, messages, tools, native_tools_enabled, on_chunk)
    }

    fn chat(
        &mut self,
        model: &str,
        messages: &[ConversationMessage],
        tools: &[ToolSpec],
        native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        if !self.dispatch() {
            anyhow::bail!(CLOSED_ERROR);
        }
        // Called only from inside the provider call, which handed this client
        // over; the forwarding form matches the other wrappers
        // (`src/minimal_loop/budget_closing.rs`).
        ChatClient::chat(
            &mut *self.inner,
            model,
            messages,
            tools,
            native_tools_enabled,
        )
    }
}

#[cfg(test)]
mod tests;
