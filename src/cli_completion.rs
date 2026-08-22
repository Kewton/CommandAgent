use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::time::Duration;

use clap_complete::CompletionCandidate;
use reqwest::blocking::Client;
use serde_json::Value;

const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";
const DEFAULT_LM_STUDIO_HOST: &str = "http://localhost:1234";
const COMPLETION_TIMEOUT: Duration = Duration::from_millis(300);

pub(crate) fn complete_model_ids(current: &OsStr) -> Vec<CompletionCandidate> {
    complete_model_ids_from_hosts(current, DEFAULT_OLLAMA_HOST, DEFAULT_LM_STUDIO_HOST)
}

pub(crate) fn complete_preset_names(current: &OsStr) -> Vec<CompletionCandidate> {
    let Ok(root) = std::env::current_dir() else {
        return Vec::new();
    };
    complete_preset_names_from_root(current, &root)
}

fn complete_preset_names_from_root(
    current: &OsStr,
    root: &std::path::Path,
) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return Vec::new();
    };
    crate::config::preset_names(root)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(CompletionCandidate::new)
        .collect()
}

fn complete_model_ids_from_hosts(
    current: &OsStr,
    ollama_host: &str,
    lm_studio_host: &str,
) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return Vec::new();
    };
    let Ok(client) = Client::builder()
        .connect_timeout(COMPLETION_TIMEOUT)
        .timeout(COMPLETION_TIMEOUT)
        .build()
    else {
        return Vec::new();
    };

    let mut models = BTreeSet::new();
    extend_ollama_models(&client, ollama_host, &mut models);
    extend_lm_studio_models(&client, lm_studio_host, &mut models);
    models
        .into_iter()
        .filter(|model| model.starts_with(prefix))
        .map(CompletionCandidate::new)
        .collect()
}

fn extend_ollama_models(client: &Client, host: &str, models: &mut BTreeSet<String>) {
    let Ok(response) = client.get(format!("{host}/api/tags")).send() else {
        return;
    };
    let Ok(response) = response.error_for_status() else {
        return;
    };
    let Ok(body) = response.json::<Value>() else {
        return;
    };
    let Some(entries) = body.get("models").and_then(Value::as_array) else {
        return;
    };
    models.extend(entries.iter().filter_map(|entry| {
        entry
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
    }));
}

fn extend_lm_studio_models(client: &Client, host: &str, models: &mut BTreeSet<String>) {
    let Ok(response) = client.get(format!("{host}/v1/models")).send() else {
        return;
    };
    let Ok(response) = response.error_for_status() else {
        return;
    };
    let Ok(body) = response.json::<Value>() else {
        return;
    };
    let Some(entries) = body.get("data").and_then(Value::as_array) else {
        return;
    };
    models.extend(
        entries
            .iter()
            .filter_map(|entry| entry.get("id").and_then(Value::as_str).map(str::to_string)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn local_model_completion_merges_filters_sorts_and_deduplicates() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 1024];
                let read = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..read]);
                let body = if request.starts_with("GET /api/tags ") {
                    r#"{"models":[{"name":"qwen-local"},{"name":"shared-model"}]}"#
                } else {
                    r#"{"data":[{"id":"lm-local"},{"id":"shared-model"}]}"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        let candidates = complete_model_ids_from_hosts(OsStr::new("s"), &host, &host)
            .into_iter()
            .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        server.join().unwrap();

        assert_eq!(candidates, vec!["shared-model"]);
    }

    #[test]
    fn unreachable_local_providers_fall_back_without_candidates() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);

        assert!(complete_model_ids_from_hosts(OsStr::new(""), &host, &host).is_empty());
    }

    #[test]
    fn preset_completion_merges_sorts_deduplicates_and_filters_search_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join(".commandagent")).unwrap();
        std::fs::create_dir_all(root.path().join(".anvil")).unwrap();
        std::fs::write(
            root.path().join(".commandagent/config.toml"),
            "[preset.zeta_issue255]\n[preset.shared_issue255]\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join(".anvil/config.toml"),
            "[preset.alpha_issue255]\n[preset.shared_issue255]\n",
        )
        .unwrap();

        let candidates = complete_preset_names_from_root(OsStr::new("s"), root.path())
            .into_iter()
            .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(candidates, vec!["shared_issue255"]);
    }
}
