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
//!   the malformed call. The error text is never read.
//!
//! Neither observation changes what the loop decides. The armed sentence is
//! kept here, apart from the loop's pending feedback, so a later rewrite or
//! clearing of that feedback (the empty-response stages, the fresh-session
//! retry) does not drop it. Right before the next request is built, the
//! sentence is appended once to whatever feedback that request carries, and
//! the `output_limit_feedback` event is written only after the provider call
//! shows that request reached the client. When no request follows (the last
//! iteration, a max-length guard stop, an exhausted malformed retry, an
//! accepted final), nothing is added and nothing is written.
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

/// What the client saw during one provider call.
#[derive(Debug, Default)]
struct CallState {
    dispatched: bool,
    reasoning_tokens: Option<u64>,
}

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
}

impl OutputLimitFeedback {
    pub(super) fn new(events_path: Option<&Path>) -> Self {
        Self {
            events_path: events_path.map(Path::to_path_buf),
            pending: None,
            staged: None,
            call: SharedCall::default(),
            attempts: [0; 2],
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
    /// so a late worker of an earlier call can never write into it.
    pub(super) fn watch<C>(&mut self, inner: C) -> OutputLimitWatch<C> {
        self.call = SharedCall::default();
        OutputLimitWatch {
            inner,
            call: Arc::clone(&self.call),
        }
    }

    /// After the provider call returned: a staged sentence whose request
    /// reached the client is recorded; one whose request never did is armed
    /// again for the next request.
    pub(super) fn settle(&mut self) {
        let Some(observation) = self.staged.take() else {
            return;
        };
        if !lock(&self.call).dispatched {
            self.pending = Some(observation);
            return;
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
pub(super) struct OutputLimitWatch<C> {
    inner: C,
    call: SharedCall,
}

impl<C> OutputLimitWatch<C> {
    fn dispatched(&self) {
        let mut call = lock(&self.call);
        call.dispatched = true;
        call.reasoning_tokens = None;
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
        self.dispatched();
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
        self.dispatched();
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
