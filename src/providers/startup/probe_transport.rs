use std::time::Duration;

/// Private transport boundary for the Ollama model-list startup probe.
pub(super) trait OllamaProbe {
    fn list_models(&self, host: &str, timeout: Duration) -> anyhow::Result<Vec<String>>;
}

/// Real HTTP transport used in production.
pub(super) struct HttpOllamaProbe;

impl OllamaProbe for HttpOllamaProbe {
    fn list_models(&self, host: &str, timeout: Duration) -> anyhow::Result<Vec<String>> {
        crate::providers::ollama::OllamaClient::new(host.to_string(), timeout.as_secs(), 1, 0)
            .and_then(|client| client.list_models())
    }
}
