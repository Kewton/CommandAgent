use crate::config::Provider;

pub(crate) fn connection_error(
    provider: Provider,
    endpoint: &str,
    error: impl std::fmt::Display,
) -> anyhow::Error {
    let error = single_line(&error.to_string());
    anyhow::anyhow!(
        "{} request failed: {error}\nHint: {}",
        provider_name(provider),
        connection_hint(provider, endpoint)
    )
}

pub(crate) fn http_status_error(
    provider: Provider,
    model: &str,
    status: reqwest::StatusCode,
) -> anyhow::Error {
    anyhow::anyhow!(
        "{} request failed: HTTP {status}\nHint: {}",
        provider_name(provider),
        status_hint(provider, model, status.as_u16())
    )
}

fn connection_hint(provider: Provider, endpoint: &str) -> String {
    let endpoint = single_line(endpoint);
    match provider {
        Provider::Ollama => format!(
            "Start Ollama with `ollama serve`, verify `--ollama-host {endpoint}`, then run `commandagent --doctor`."
        ),
        Provider::Openai | Provider::Gemini => {
            format!("Check connectivity to {endpoint}, then run `commandagent --doctor`.")
        }
    }
}

fn status_hint(provider: Provider, model: &str, status: u16) -> String {
    let model = single_line(model);
    match status {
        401 | 403 => match api_key_name(provider) {
            Some(key) => format!(
                "Set `{key}` in the environment or workspace `.env`, then run `commandagent --doctor`."
            ),
            None => "Check provider authentication and run `commandagent --doctor`.".to_string(),
        },
        404 if provider == Provider::Ollama => format!(
            "Model `{model}` was not found. Run `ollama pull {model}`, then run `commandagent --doctor`."
        ),
        404 => format!(
            "Verify model `{model}` exists and your account can access it, then run `commandagent --doctor`."
        ),
        _ => "Run `commandagent --doctor` and check the provider configuration.".to_string(),
    }
}

fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Ollama => "Ollama",
        Provider::Openai => "OpenAI",
        Provider::Gemini => "Gemini",
    }
}

fn api_key_name(provider: Provider) -> Option<&'static str> {
    match provider {
        Provider::Ollama => None,
        Provider::Openai => Some("OPENAI_API_KEY"),
        Provider::Gemini => Some("GEMINI_API_KEY"),
    }
}

fn single_line(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '?'
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_failure_has_fixed_ollama_remediation_line() {
        let error = connection_error(
            Provider::Ollama,
            "http://127.0.0.1:11434",
            "connection refused",
        )
        .to_string();

        assert_eq!(
            error,
            "Ollama request failed: connection refused\nHint: Start Ollama with `ollama serve`, verify `--ollama-host http://127.0.0.1:11434`, then run `commandagent --doctor`."
        );
    }

    #[test]
    fn not_found_failure_has_fixed_model_remediation_line() {
        let error = http_status_error(
            Provider::Ollama,
            "qwen3:latest",
            reqwest::StatusCode::NOT_FOUND,
        )
        .to_string();

        assert_eq!(
            error,
            "Ollama request failed: HTTP 404 Not Found\nHint: Model `qwen3:latest` was not found. Run `ollama pull qwen3:latest`, then run `commandagent --doctor`."
        );
    }

    #[test]
    fn authentication_failures_have_fixed_key_remediation_lines() {
        for (provider, status, expected) in [
            (
                Provider::Openai,
                reqwest::StatusCode::UNAUTHORIZED,
                "OpenAI request failed: HTTP 401 Unauthorized\nHint: Set `OPENAI_API_KEY` in the environment or workspace `.env`, then run `commandagent --doctor`.",
            ),
            (
                Provider::Gemini,
                reqwest::StatusCode::FORBIDDEN,
                "Gemini request failed: HTTP 403 Forbidden\nHint: Set `GEMINI_API_KEY` in the environment or workspace `.env`, then run `commandagent --doctor`.",
            ),
        ] {
            assert_eq!(
                http_status_error(provider, "model", status).to_string(),
                expected
            );
        }
    }
}
