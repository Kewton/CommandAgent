use std::collections::BTreeSet;
use std::time::Duration;

use crate::config::{Action, Config, Provider};

const STARTUP_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

pub(crate) fn warnings(config: &Config, stdin_is_terminal: bool) -> Vec<String> {
    if !should_probe(config, stdin_is_terminal) {
        return Vec::new();
    }
    let models = configured_ollama_models(config);
    if models.is_empty() {
        return Vec::new();
    }

    let result = crate::providers::ollama::OllamaClient::new(
        config.ollama_host.clone(),
        STARTUP_PROBE_TIMEOUT.as_secs(),
        1,
        0,
    )
    .and_then(|client| client.list_models());
    match result {
        Ok(installed) => {
            let installed = installed.into_iter().collect::<BTreeSet<_>>();
            models
                .difference(&installed)
                .map(|model| {
                    let model = single_line(model);
                    format!(
                        "warning: Ollama model `{model}` is not installed at {}. Run `ollama pull {model}`, then run `commandagent --doctor`.",
                        single_line(&config.ollama_host)
                    )
                })
                .collect()
        }
        Err(error) => vec![format!(
            "warning: Ollama is unreachable at {} ({error}). Start it with `ollama serve`, verify `--ollama-host`, then run `commandagent --doctor`; continuing.",
            single_line(&config.ollama_host),
            error = single_line(&error.to_string())
        )],
    }
}

fn should_probe(config: &Config, stdin_is_terminal: bool) -> bool {
    stdin_is_terminal && !config.offline && matches!(config.action, Action::Repl)
}

fn configured_ollama_models(config: &Config) -> BTreeSet<String> {
    [
        (config.provider, config.model.as_str()),
        (config.planner_provider, config.planner_model.as_str()),
    ]
    .into_iter()
    .filter(|(provider, _)| *provider == Provider::Ollama)
    .map(|(_, model)| model.to_string())
    .collect()
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    use super::*;
    use crate::config::{ConfigFieldSources, NarrationMode, PlanPreset, PromptLayout};

    fn config(host: String) -> Config {
        Config {
            workspace_root: PathBuf::from("."),
            state_dir: PathBuf::from("state"),
            eval_events_path: None,
            completion_contract_path: None,
            yes: false,
            offline: false,
            context_budget: 1_000,
            model: "executor:latest".to_string(),
            provider: Provider::Ollama,
            prompt_layout: PromptLayout::Stable,
            plan_preset: PlanPreset::None,
            intent_override: None,
            planner_model: "planner:latest".to_string(),
            planner_provider: Provider::Ollama,
            ollama_host: host,
            num_predict: 1,
            max_iterations: 1,
            chat_timeout_secs: 600,
            chat_timeout_source: "default:local_provider".to_string(),
            field_sources: ConfigFieldSources::default(),
            chat_retries: 0,
            stream: true,
            resume: None,
            fresh_session: false,
            no_footer: true,
            narration: NarrationMode::Normal,
            profile: "generic".to_string(),
            profile_explicit: false,
            profile_inference: None,
            style: "default".to_string(),
            action: Action::Repl,
        }
    }

    #[test]
    fn offline_and_non_interactive_actions_skip_startup_probe() {
        let mut config = config("not a valid URL".to_string());
        config.offline = true;
        assert!(warnings(&config, true).is_empty());

        config.offline = false;
        assert!(warnings(&config, false).is_empty());

        config.action = Action::Prompt("hello".to_string());
        assert!(warnings(&config, true).is_empty());
    }

    #[test]
    fn reachable_ollama_uses_one_tags_request_and_warns_for_missing_model() {
        let requests = Arc::new(AtomicUsize::new(0));
        let server_requests = Arc::clone(&requests);
        let (host, server) = tags_server(r#"{"models":[{"name":"executor:latest"}]}"#, move || {
            server_requests.fetch_add(1, Ordering::SeqCst);
        });

        let warnings = warnings(&config(host.clone()), true);
        server.join().unwrap();

        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(
            warnings[0],
            format!(
                "warning: Ollama model `planner:latest` is not installed at {host}. Run `ollama pull planner:latest`, then run `commandagent --doctor`."
            )
        );
    }

    #[test]
    fn unreachable_ollama_warning_is_bounded_and_actionable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);

        let started = Instant::now();
        let warnings = warnings(&config(host.clone()), true);

        assert!(started.elapsed() <= Duration::from_secs(3));
        assert_eq!(warnings.len(), 1);
        for expected in [
            &host,
            "ollama serve",
            "--ollama-host",
            "commandagent --doctor",
        ] {
            assert!(warnings[0].contains(expected), "{}", warnings[0]);
        }
        assert!(warnings[0].ends_with("continuing."), "{}", warnings[0]);
    }

    #[test]
    fn unresponsive_ollama_respects_two_second_probe_bound() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).unwrap();
            std::thread::sleep(Duration::from_millis(2_500));
        });

        let started = Instant::now();
        let warnings = warnings(&config(host), true);
        let elapsed = started.elapsed();
        server.join().unwrap();

        assert!(
            elapsed >= Duration::from_millis(1_800),
            "elapsed={elapsed:?}"
        );
        assert!(elapsed <= Duration::from_secs(3), "elapsed={elapsed:?}");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].ends_with("continuing."), "{}", warnings[0]);
    }

    fn tags_server(
        body: &'static str,
        on_request: impl FnOnce() + Send + 'static,
    ) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let read = stream.read(&mut request).unwrap();
            assert!(String::from_utf8_lossy(&request[..read]).contains("GET /api/tags"));
            on_request();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        (host, server)
    }
}
