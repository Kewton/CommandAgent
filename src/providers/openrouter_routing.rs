//! OpenRouter provider routing on the generic OpenAI-compatible transport
//! (Issue #939).
//!
//! A declared `openrouter_provider` preset value is fixed when the client is
//! built and added as one top-level `provider` object to every Chat
//! Completions and Responses body, on every clone, worker, retry, and native or
//! XML tool mode. Nothing else in the body changes, and an existing `provider`
//! key is a conflict, never merged. The response's top-level `provider` field
//! is recorded as reported by the upstream, without filling it in from the
//! requested routing or the transport name.

use serde_json::{Value, json};

use crate::config::{OpenAiCompatibleConfig, ProviderRole};

use super::compatible_effort::role_name;

/// The longest reported provider name kept, in characters. A longer value is
/// `invalid`, never truncated into a plausible name.
pub(crate) const MAX_RESPONSE_PROVIDER_CHARS: usize = 128;

/// The routing one generic client sends, fixed at construction.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClientRouting {
    requested: Value,
    source: String,
    role: ProviderRole,
}

impl ClientRouting {
    /// The routing `role` sends: only a declared, non-empty preset value on a
    /// role that resolved to the generic OpenAI-compatible transport.
    pub(crate) fn for_role(
        compatible: &OpenAiCompatibleConfig,
        role: ProviderRole,
    ) -> Option<Self> {
        if !compatible.applies_to(role) {
            return None;
        }
        compatible
            .openrouter_provider()
            .filter(|resolved| resolved.is_declared())
            .map(|resolved| Self {
                requested: resolved.routing.provider_object(),
                source: resolved.source.clone(),
                role,
            })
    }

    /// Adds the `provider` object to a request body. A body that is not an
    /// object, or already has a `provider` key, is refused instead of merged.
    pub(crate) fn apply(&self, mut body: Value) -> anyhow::Result<Value> {
        let Some(object) = body.as_object_mut() else {
            anyhow::bail!("openrouter_provider can only be added to a JSON object request body");
        };
        if object.contains_key("provider") {
            anyhow::bail!(
                "openrouter_provider conflicts with a provider key already in the request body; nothing was sent"
            );
        }
        object.insert("provider".to_string(), self.requested.clone());
        Ok(body)
    }

    /// The additive `provider_request` fields; the transport `provider` stays.
    pub(crate) fn annotate_request(&self, event: &mut Value) {
        event["requested_provider"] = self.requested.clone();
        event["provider_routing_source"] = json!(self.source);
        event["provider_role"] = json!(role_name(self.role));
    }

    /// The `openrouter_provider_observed` event for one 2xx response body.
    /// `transport` is the existing transport label, never a provider name;
    /// `redact` is the client's credential redaction.
    pub(crate) fn observed_event(
        &self,
        transport: &str,
        api: &str,
        model: &str,
        attempt: usize,
        body: &str,
        redact: impl Fn(&str) -> String,
    ) -> Value {
        let (response_provider, status) = match observe(body) {
            Observation::Reported(name) => {
                let scrubbed = crate::eval_events::scrub_sensitive_text(&redact(&name));
                if scrubbed == name {
                    (Some(name), "reported")
                } else {
                    // A field carrying a credential is not a provider name.
                    (None, "invalid")
                }
            }
            Observation::Missing => (None, "missing"),
            Observation::Invalid => (None, "invalid"),
            Observation::Unparseable => (None, "unparseable"),
        };
        json!({
            "event": "openrouter_provider_observed",
            "schema_version": "1",
            "provider": transport,
            "provider_role": role_name(self.role),
            "api": api,
            "model": model,
            "attempt": attempt,
            "requested_provider": self.requested,
            "response_provider": response_provider,
            "response_provider_status": status,
        })
    }
}

/// What a response body says about its top-level `provider` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Observation {
    Reported(String),
    Missing,
    Invalid,
    Unparseable,
}

/// Reads only the top-level `provider` field; nothing else in the body is
/// inspected or kept.
pub(crate) fn observe(body: &str) -> Observation {
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(body) else {
        return Observation::Unparseable;
    };
    match object.get("provider") {
        None | Some(Value::Null) => Observation::Missing,
        Some(Value::String(name))
            if !name.trim().is_empty()
                && !name.chars().any(char::is_control)
                && name.chars().count() <= MAX_RESPONSE_PROVIDER_CHARS =>
        {
            Observation::Reported(name.clone())
        }
        Some(_) => Observation::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{ClientRouting, MAX_RESPONSE_PROVIDER_CHARS, Observation, observe};
    use crate::config::ProviderRole;

    fn routing(requested: Value) -> ClientRouting {
        ClientRouting {
            requested,
            source: "preset:glm".to_string(),
            role: ProviderRole::Planner,
        }
    }

    fn legacy_body() -> Value {
        json!({
            "model": "m",
            "messages": [{"role": "user", "content": "hello"}],
            "tools": [{"type": "function", "function": {"name": "Read"}}],
            "tool_choice": "auto",
            "reasoning_effort": "low",
            "max_tokens": 64,
        })
    }

    #[test]
    fn apply_adds_only_the_provider_object() {
        let requested = json!({"only": ["open-inference"], "allow_fallbacks": false});
        let routed = routing(requested.clone()).apply(legacy_body()).unwrap();
        assert_eq!(routed["provider"], requested);
        let mut rest = routed.clone();
        rest.as_object_mut().unwrap().remove("provider");
        assert_eq!(rest, legacy_body());
        assert_eq!(
            serde_json::to_string(&rest).unwrap(),
            serde_json::to_string(&legacy_body()).unwrap()
        );
    }

    #[test]
    fn an_existing_provider_key_or_non_object_body_is_refused() {
        let mut body = legacy_body();
        body["provider"] = json!({"only": ["other"]});
        let error = routing(json!({"order": ["a"]})).apply(body).unwrap_err();
        assert!(error.to_string().contains("conflicts"), "{error}");
        assert!(routing(json!({"order": ["a"]})).apply(json!([1])).is_err());
    }

    #[test]
    fn request_annotation_keeps_the_transport_provider() {
        let mut event = json!({"event": "provider_request", "provider": "openai-compatible"});
        routing(json!({"order": ["a"]})).annotate_request(&mut event);
        assert_eq!(event["provider"], "openai-compatible");
        assert_eq!(event["requested_provider"], json!({"order": ["a"]}));
        assert_eq!(event["provider_routing_source"], "preset:glm");
        assert_eq!(event["provider_role"], "planner");
    }

    #[test]
    fn observation_distinguishes_reported_missing_invalid_and_unparseable() {
        assert_eq!(
            observe(r#"{"provider":"OpenInference","choices":[]}"#),
            Observation::Reported("OpenInference".to_string())
        );
        assert_eq!(observe(r#"{"choices":[]}"#), Observation::Missing);
        assert_eq!(observe(r#"{"provider":null}"#), Observation::Missing);
        for invalid in [
            r#"{"provider":7}"#,
            r#"{"provider":["OpenInference"]}"#,
            r#"{"provider":{"name":"OpenInference"}}"#,
            r#"{"provider":""}"#,
            r#"{"provider":"  "}"#,
            r#"{"provider":"Open\nInference"}"#,
            r#"{"provider":"Open\u0000Inference"}"#,
        ] {
            assert_eq!(observe(invalid), Observation::Invalid, "{invalid}");
        }
        let long = format!(
            r#"{{"provider":"{}"}}"#,
            "p".repeat(MAX_RESPONSE_PROVIDER_CHARS + 1)
        );
        assert_eq!(observe(&long), Observation::Invalid);
        let exact = format!(
            r#"{{"provider":"{}"}}"#,
            "p".repeat(MAX_RESPONSE_PROVIDER_CHARS)
        );
        assert!(matches!(observe(&exact), Observation::Reported(_)));
        for unparseable in [
            "",
            "not json",
            "[1,2]",
            "\"OpenInference\"",
            "{\"provider\":",
        ] {
            assert_eq!(
                observe(unparseable),
                Observation::Unparseable,
                "{unparseable}"
            );
        }
    }

    #[test]
    fn observed_event_never_fills_in_the_requested_routing_or_transport() {
        let requested = json!({"only": ["open-inference/fp4"], "allow_fallbacks": false});
        let routing = routing(requested.clone());
        let event = |body: &str| {
            routing.observed_event(
                "openai-compatible",
                "chat_completions",
                "m",
                2,
                body,
                |value| value.to_string(),
            )
        };
        let reported = event(r#"{"provider":"OpenInference"}"#);
        assert_eq!(reported["event"], "openrouter_provider_observed");
        assert_eq!(reported["schema_version"], "1");
        assert_eq!(reported["provider"], "openai-compatible");
        assert_eq!(reported["provider_role"], "planner");
        assert_eq!(reported["api"], "chat_completions");
        assert_eq!(reported["model"], "m");
        assert_eq!(reported["attempt"], 2);
        assert_eq!(reported["requested_provider"], requested);
        assert_eq!(reported["response_provider"], "OpenInference");
        assert_eq!(reported["response_provider_status"], "reported");
        for (body, status) in [
            (r#"{"choices":[]}"#, "missing"),
            (r#"{"provider":5}"#, "invalid"),
            ("<html>", "unparseable"),
        ] {
            let observed = event(body);
            assert!(observed["response_provider"].is_null(), "{observed}");
            assert_eq!(observed["response_provider_status"], status);
        }
        let keys = reported
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            [
                "api",
                "attempt",
                "event",
                "model",
                "provider",
                "provider_role",
                "requested_provider",
                "response_provider",
                "response_provider_status",
                "schema_version",
            ]
        );
    }

    #[test]
    fn a_reflected_credential_is_not_recorded_as_the_provider() {
        let token = "fake-token-939-reflected";
        let routing = routing(json!({"order": ["a"]}));
        let body = format!(r#"{{"provider":"{token}","choices":[]}}"#);
        let event =
            routing.observed_event("openai-compatible", "responses", "m", 1, &body, |value| {
                value.replace(token, "<redacted>")
            });
        assert!(!event.to_string().contains(token), "{event}");
        assert!(event["response_provider"].is_null());
        assert_eq!(event["response_provider_status"], "invalid");
        let heuristic = routing.observed_event(
            "openai-compatible",
            "responses",
            "m",
            1,
            r#"{"provider":"sk-939-looks-like-a-key"}"#,
            |value| value.to_string(),
        );
        assert!(!heuristic.to_string().contains("sk-939"), "{heuristic}");
        assert_eq!(heuristic["response_provider_status"], "invalid");
    }
}
