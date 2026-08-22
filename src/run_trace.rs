use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::{Value, json};

use crate::providers::AssistantReply;
use crate::state::ConversationMessage;
use crate::tools::registry::ToolSpec;

thread_local! {
    static TRACE_ENABLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) struct TraceGuard {
    previous: bool,
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        TRACE_ENABLED.set(self.previous);
    }
}

pub(crate) fn install(enabled: bool) -> TraceGuard {
    let previous = TRACE_ENABLED.replace(enabled);
    TraceGuard { previous }
}

pub(crate) fn enabled() -> bool {
    TRACE_ENABLED.get()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn record_provider_exchange(
    events_path: Option<&Path>,
    scope: &str,
    provider: &str,
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    result: &anyhow::Result<AssistantReply>,
) -> anyhow::Result<Option<PathBuf>> {
    if !enabled() {
        return Ok(None);
    }
    let Some(run_dir) = events_path.and_then(Path::parent) else {
        return Ok(None);
    };
    record_provider_exchange_in(
        run_dir,
        scope,
        provider,
        model,
        messages,
        tools,
        native_tools_enabled,
        result,
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn record_provider_exchange_in(
    run_dir: &Path,
    scope: &str,
    provider: &str,
    model: &str,
    messages: &[ConversationMessage],
    tools: &[ToolSpec],
    native_tools_enabled: bool,
    result: &anyhow::Result<AssistantReply>,
) -> anyhow::Result<PathBuf> {
    let trace_dir = run_dir.join("trace");
    std::fs::create_dir_all(&trace_dir)
        .with_context(|| format!("failed to create trace directory {}", trace_dir.display()))?;
    let path = trace_dir.join(format!("provider-{}.json", uuid::Uuid::now_v7()));
    let response = match result {
        Ok(reply) => json!({ "ok": true, "reply": reply }),
        Err(error) => json!({ "ok": false, "error": format!("{error:#}") }),
    };
    let mut document = json!({
        "schema_version": "commandagent.run-trace/v1",
        "scope": scope,
        "provider": provider,
        "model": model,
        "request": {
            "messages": messages,
            "tools": tools,
            "native_tools_enabled": native_tools_enabled,
        },
        "response": response,
    });
    scrub_value(&mut document);
    let bytes =
        serde_json::to_vec_pretty(&document).context("failed to serialize provider trace")?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .with_context(|| format!("failed to create provider trace {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write provider trace {}", path.display()))?;
    file.write_all(b"\n")
        .with_context(|| format!("failed to finish provider trace {}", path.display()))?;
    Ok(path)
}

fn scrub_value(value: &mut Value) {
    match value {
        Value::String(text) => *text = crate::eval_events::scrub_sensitive_text(text),
        Value::Array(values) => values.iter_mut().for_each(scrub_value),
        Value::Object(values) => values.values_mut().for_each(scrub_value),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_is_opt_in_and_scrubs_prompt_reply_and_home_paths() {
        let root = tempfile::tempdir().unwrap();
        let events_path = root.path().join("run/events.jsonl");
        std::fs::create_dir_all(events_path.parent().unwrap()).unwrap();
        let messages = vec![ConversationMessage::user(
            "use sk-test-secret at /Users/alice/project",
        )];
        let result = Ok(AssistantReply::text(
            "Authorization api_key=secret /home/bob/reply",
        ));

        assert!(
            record_provider_exchange(
                Some(&events_path),
                "executor",
                "ollama",
                "model",
                &messages,
                &[],
                false,
                &result,
            )
            .unwrap()
            .is_none()
        );
        assert!(!root.path().join("run/trace").exists());

        let _guard = install(true);
        let path = record_provider_exchange(
            Some(&events_path),
            "executor",
            "ollama",
            "model",
            &messages,
            &[],
            false,
            &result,
        )
        .unwrap();
        let path = path.expect("enabled trace returns its written path");
        let text = std::fs::read_to_string(path).unwrap();

        assert!(text.contains("commandagent.run-trace/v1"), "{text}");
        assert!(text.contains("<redacted>"), "{text}");
        assert!(text.contains("/Users/<user>/project"), "{text}");
        assert!(text.contains("/home/<user>/reply"), "{text}");
        assert!(!text.contains("sk-test-secret"), "{text}");
        assert!(!text.contains("api_key=secret"), "{text}");
    }
}
