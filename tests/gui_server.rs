use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use commandagent::tui::boundary_shell::route::admitted_profiles;

const TEST_TRIAL_TOKEN: &str = "commandagent-gui-test-token-000000000001";

#[test]
fn gui_server_help_exposes_only_serving_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for option in [
        "--port",
        "--base-path",
        "--static-dir",
        "--repository-root",
        "--execution-root",
        "--commandagent-bin",
    ] {
        assert!(help.contains(option), "missing {option}: {help}");
    }
    for forbidden in ["--provider", "--model", "mutation"] {
        assert!(!help.to_lowercase().contains(forbidden), "{help}");
    }
}

#[test]
fn gui_server_rejects_noncanonical_base_paths() {
    for value in ["proxy/gui", "/proxy/gui/", "/proxy//gui", "/../gui"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
            .args(["--base-path", value])
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {value:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--base-path"),
            "stderr for {value:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn gui_server_disables_trial_without_an_execution_root() {
    let mut server = Server::start_dashboard_only();
    let dashboard = server.request_without_access("GET", "/", None);
    assert_eq!(dashboard.status, 200, "{}", dashboard.body);
    let runtime = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(runtime.status, 200, "{}", runtime.body);
    let runtime: serde_json::Value = serde_json::from_str(&runtime.body).unwrap();
    assert_eq!(runtime["trial_available"], false);
    assert!(runtime["session"].is_null());
    let response =
        server.request_without_access("POST", "/api/session-proposals", Some(&session_spec()));
    assert_eq!(response.status, 503, "{}", response.body);
    assert!(response.body.contains("trial execution is disabled"));
    server.stop();
}

#[cfg(unix)]
#[test]
fn gui_server_caches_hashed_next_assets_but_not_html() {
    let temp = tempfile::tempdir().unwrap();
    let static_root = temp.path().join("out");
    let chunk = static_root.join("_next/static/chunks/app-deadbeef.js");
    let nested = static_root.join("other/_next/static/not-content-addressed.js");
    std::fs::create_dir_all(chunk.parent().unwrap()).unwrap();
    std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
    std::fs::write(
        static_root.join("index.html"),
        "<!doctype html><title>GUI</title>",
    )
    .unwrap();
    std::fs::write(&chunk, "console.log('immutable');").unwrap();
    std::fs::write(&nested, "console.log('not immutable');").unwrap();
    let mut server = Server::start_dashboard_only_with_static_root(&static_root);

    let index = server.request_without_access("GET", "/", None);
    assert_eq!(index.status, 200, "{}", index.body);
    assert_eq!(index.header("cache-control"), Some("no-store"));

    let asset = server.request_without_access("GET", "/_next/static/chunks/app-deadbeef.js", None);
    assert_eq!(asset.status, 200, "{}", asset.body);
    assert_eq!(
        asset.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );

    let nested =
        server.request_without_access("GET", "/other/_next/static/not-content-addressed.js", None);
    assert_eq!(nested.status, 200, "{}", nested.body);
    assert_eq!(nested.header("cache-control"), Some("no-store"));
    server.stop();
}

#[cfg(unix)]
#[test]
fn trial_options_match_admitted_profiles_without_trial_access() {
    let mut server = Server::start_dashboard_only();
    let response = server.request_without_access("GET", "/api/trial-options", None);
    assert_eq!(response.status, 200, "{}", response.body);
    let body: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    let actual_profiles = body["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let expected_profiles = admitted_profiles()
        .into_iter()
        .map(|profile| profile.to_string())
        .collect::<Vec<_>>();
    assert_eq!(actual_profiles, expected_profiles);
    assert!(body["profiles"].as_array().unwrap().iter().all(|option| {
        option["label"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && option["description"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
    }));

    let providers = body["providers"].as_array().unwrap();
    assert_eq!(
        providers
            .iter()
            .map(|option| option["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ollama", "lm-studio", "openai", "gemini"]
    );
    assert!(providers.iter().all(|option| {
        option["label"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
            && option["model_hint"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
    }));
    server.stop();
}

#[test]
fn gui_server_requires_a_runtime_token_for_trial_execution() {
    let workspace = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .args(["--port", "0", "--base-path", "/"])
        .arg("--repository-root")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("--execution-root")
        .arg(workspace.path())
        .env_remove("GUI_TRIAL_TOKEN")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("GUI_TRIAL_TOKEN is required"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn gui_server_rejects_repository_workspace_overlap_and_symlink_aliases() {
    use std::os::unix::fs::symlink;

    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let alias = temp.path().join("repository-alias");
    let repository_child = repository.join("gui");
    symlink(repository, &alias).unwrap();
    for workspace in [
        repository,
        repository.parent().unwrap(),
        repository_child.as_path(),
        &alias,
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
            .args(["--port", "0", "--base-path", "/"])
            .arg("--repository-root")
            .arg(repository)
            .arg("--execution-root")
            .arg(workspace)
            .env("GUI_TRIAL_TOKEN", TEST_TRIAL_TOKEN)
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {}", workspace.display());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("must be disjoint"),
            "stderr for {}: {}",
            workspace.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn gui_server_revalidates_the_workspace_before_dispatch() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(&second).unwrap();
    let alias = temp.path().join("trial-workspace");
    symlink(&first, &alias).unwrap();
    let mut server = Server::start(
        &alias,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );
    let spec = session_spec();
    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal: serde_json::Value = serde_json::from_str(&proposal.body).unwrap();

    std::fs::remove_file(&alias).unwrap();
    symlink(&second, &alias).unwrap();
    let mut confirmed = spec;
    confirmed["confirmation_hash"] = proposal["card_hash"].clone();
    let response = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(response.status, 409, "{}", response.body);
    assert!(response.body.contains("changed after startup"));
    server.stop();
}

#[cfg(unix)]
#[test]
fn spawn_failure_reports_the_binary_and_releases_the_workspace() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    let cli = temp.path().join("missing-commandagent");
    let mut server = Server::start(&workspace, &cli);
    let spec = session_spec();
    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal: serde_json::Value = serde_json::from_str(&proposal.body).unwrap();
    let mut confirmed = spec;
    confirmed["confirmation_hash"] = proposal["card_hash"].clone();

    let failed = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(failed.status, 500, "{}", failed.body);
    assert!(
        failed.body.contains(cli.to_string_lossy().as_ref()),
        "{}",
        failed.body
    );
    assert!(
        failed.body.contains("No such file or directory") && failed.body.contains("os error"),
        "{}",
        failed.body
    );
    let lease = server.request("GET", "/api/trial-workspace", None);
    assert_eq!(lease.status, 200, "{}", lease.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lease.body).unwrap(),
        serde_json::json!({ "status": "idle" })
    );
    server.stop();

    write_terminal_cli(&cli);
    let mut restarted = Server::start(&workspace, &cli);
    let lease = restarted.request("GET", "/api/trial-workspace", None);
    assert_eq!(lease.status, 200, "{}", lease.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lease.body).unwrap(),
        serde_json::json!({ "status": "idle" })
    );
    let created = restarted.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(created.status, 202, "{}", created.body);
    restarted.stop();
}

#[cfg(unix)]
#[test]
fn recovery_required_lease_is_exposed_by_an_authenticated_get() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    let session_id = "0198b9c8-fab8-7000-8000-000000000064";
    std::fs::create_dir_all(
        workspace
            .join(".anvil/runs")
            .join(session_id)
            .join("state/boundary-confirmations"),
    )
    .unwrap();
    let mut server = Server::start(
        &workspace,
        std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
    );

    let unauthorized = server.request_without_access("GET", "/api/trial-workspace", None);
    assert_eq!(unauthorized.status, 401, "{}", unauthorized.body);
    let lease = server.request("GET", "/api/trial-workspace", None);
    assert_eq!(lease.status, 200, "{}", lease.body);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&lease.body).unwrap(),
        serde_json::json!({
            "status": "recovery_required",
            "session_id": session_id,
        })
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn confirmed_session_delegates_with_cli_event_bytes_unchanged() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("workspace");
    std::fs::create_dir_all(workspace.join("cli")).unwrap();
    std::fs::write(workspace.join("cli/main.py"), "print('fixture')\n").unwrap();
    let cli = temp.path().join("fake-commandagent");
    let fixture = include_str!("fixtures/gui_cli_events.jsonl");
    let script = format!(
        "#!/bin/sh\ncase \" $* \" in\n  *\" --run-ultra-plan \"*) sleep 1; printf '%s' '{}' >> \"$COMMANDAGENT_EVAL_EVENTS\" ;;\n  *\" --ultra-plan-run \"*) sleep 1; printf '%s' '{}' > \"$COMMANDAGENT_EVAL_EVENTS\" ;;\n  *) printf '%s' '{}' > \"$COMMANDAGENT_EVAL_EVENTS\" ;;\nesac\n",
        fixture.replace('\'', "'\\''"),
        fixture.replace('\'', "'\\''"),
        fixture.replace('\'', "'\\''")
    );
    std::fs::write(&cli, script).unwrap();
    let mut permissions = std::fs::metadata(&cli).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&cli, permissions).unwrap();

    let direct_events = temp.path().join("direct-events.jsonl");
    assert!(
        Command::new(&cli)
            .env("COMMANDAGENT_EVAL_EVENTS", &direct_events)
            .status()
            .unwrap()
            .success()
    );
    let direct_bytes = std::fs::read(&direct_events).unwrap();
    let mut server = Server::start(&workspace, &cli);
    let idle = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(idle.status, 200, "{}", idle.body);
    let idle: serde_json::Value = serde_json::from_str(&idle.body).unwrap();
    assert_eq!(idle["trial_available"], true);
    assert!(idle["session"].is_null());
    let spec = serde_json::json!({
        "goal": "Create a CLI --pattern filter command",
        "profile": "python-cli",
        "provider": "ollama",
        "model": "fixture-executor",
        "planner_provider": "ollama",
        "planner_model": "fixture-planner"
    });

    let proposal = server.request("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(proposal.status, 200, "{}", proposal.body);
    let proposal_json: serde_json::Value = serde_json::from_str(&proposal.body).unwrap();
    let card_hash = proposal_json["card_hash"].as_str().unwrap();
    assert_eq!(proposal_json["price"]["duration_n"], 3);
    assert_eq!(proposal_json["price"]["cost_n"], 0);
    assert_eq!(
        proposal_json["identity"]["workspace"],
        workspace.canonicalize().unwrap().to_string_lossy().as_ref()
    );

    let unauthorized = server.request_without_access("POST", "/api/session-proposals", Some(&spec));
    assert_eq!(unauthorized.status, 401, "{}", unauthorized.body);
    let forbidden_origin = server.request_with_access(
        "POST",
        "/api/session-proposals",
        Some(&spec),
        Some(TEST_TRIAL_TOKEN),
        Some("https://attacker.invalid"),
    );
    assert_eq!(forbidden_origin.status, 403, "{}", forbidden_origin.body);

    let denied = server.request("POST", "/api/sessions", Some(&spec));
    assert_eq!(denied.status, 428, "{}", denied.body);

    let mut stale_confirmation = spec.clone();
    stale_confirmation["confirmation_hash"] =
        serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
    let stale = server.request("POST", "/api/sessions", Some(&stale_confirmation));
    assert_eq!(stale.status, 412, "{}", stale.body);

    let mut confirmed = spec;
    confirmed["confirmation_hash"] = serde_json::Value::String(card_hash.to_string());
    let created = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(created.status, 202, "{}", created.body);
    let created_json: serde_json::Value = serde_json::from_str(&created.body).unwrap();
    let id = created_json["id"].as_str().unwrap();
    let delegated_events = workspace.join(".anvil/runs").join(id).join("events.jsonl");

    let running = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(running.status, 200, "{}", running.body);
    let running: serde_json::Value = serde_json::from_str(&running.body).unwrap();
    assert_eq!(running["trial_available"], true);
    assert_eq!(running["session"]["id"], id);
    assert_eq!(running["session"]["state"], "running");

    let competing = server.request("POST", "/api/sessions", Some(&confirmed));
    assert_eq!(competing.status, 409, "{}", competing.body);

    let deadline = Instant::now() + Duration::from_secs(5);
    while !delegated_events.is_file() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(std::fs::read(&delegated_events).unwrap(), direct_bytes);

    let status = server.request("GET", &format!("/api/sessions/{id}"), None);
    assert_eq!(status.status, 200, "{}", status.body);
    assert_eq!(status.header("cache-control"), Some("private, no-cache"));
    let etag = status.header("etag").unwrap().to_string();
    assert!(etag.starts_with("W/\""), "{etag}");
    let status_json: serde_json::Value = serde_json::from_str(&status.body).unwrap();
    let status_keys = status_json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        status_keys,
        std::collections::BTreeSet::from([
            "acceptance_sheet",
            "assurance",
            "event_count",
            "events_path",
            "gate",
            "id",
            "phases",
            "section5",
            "status",
            "verdict",
        ])
    );
    assert_eq!(status_json["gate"], "gate_3");
    assert_eq!(status_json["verdict"], "full");
    assert_eq!(status_json["phases"][0]["status"], "completed");
    assert!(
        status_json["acceptance_sheet"]
            .as_str()
            .unwrap()
            .contains("# D-3c acceptance sheet")
    );
    let unchanged = server.request_if_none_match(&format!("/api/sessions/{id}"), &etag);
    assert_eq!(unchanged.status, 304, "{}", unchanged.body);
    assert!(unchanged.body.is_empty(), "304 response had a body");
    assert_eq!(unchanged.header("etag"), Some(etag.as_str()));
    let completed = server.request_without_access("GET", "/api/runtime-status", None);
    assert_eq!(completed.status, 200, "{}", completed.body);
    let completed: serde_json::Value = serde_json::from_str(&completed.body).unwrap();
    assert_eq!(completed["trial_available"], true);
    assert!(completed["session"].is_null());

    let credential = serde_json::json!({
        "directive": format!("use token={}", "a".repeat(24))
    });
    let rejected = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&credential),
    );
    assert_eq!(rejected.status, 422, "{}", rejected.body);

    let directive_request = serde_json::json!({ "directive": "Keep the output sorted" });
    let proposed = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&directive_request),
    );
    assert_eq!(proposed.status, 200, "{}", proposed.body);
    let proposed_json: serde_json::Value = serde_json::from_str(&proposed.body).unwrap();
    assert_eq!(
        proposed_json["scrubbed_directive"],
        "Keep the output sorted"
    );
    assert!(
        proposed_json["directive_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    let duplicate = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&directive_request),
    );
    assert_eq!(duplicate.status, 409, "{}", duplicate.body);
    let wrong_hash = format!("sha256:{}", "f".repeat(64));
    let unconfirmed = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives/{wrong_hash}"),
        None,
    );
    assert_eq!(unconfirmed.status, 400, "{}", unconfirmed.body);

    let directive_hash = proposed_json["directive_hash"].as_str().unwrap();
    let continued = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives/{directive_hash}"),
        None,
    );
    assert_eq!(continued.status, 202, "{}", continued.body);
    let intervention = server.request(
        "POST",
        &format!("/api/sessions/{id}/directives"),
        Some(&directive_request),
    );
    assert_eq!(intervention.status, 409, "{}", intervention.body);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let completed = server.request("GET", &format!("/api/sessions/{id}"), None);
        assert_eq!(completed.status, 200, "{}", completed.body);
        let completed_json: serde_json::Value = serde_json::from_str(&completed.body).unwrap();
        if completed_json["gate"] != "gate_2" {
            assert_eq!(completed_json["gate"], "gate_3");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "directive continuation timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    server.stop();
}

#[cfg(unix)]
struct Server {
    child: Child,
    port: u16,
}

#[cfg(unix)]
impl Server {
    fn start(workspace: &std::path::Path, cli: &std::path::Path) -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_with_workspace(Some(workspace), cli, true, &static_root)
    }

    fn start_dashboard_only() -> Self {
        let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("gui/out");
        Self::start_dashboard_only_with_static_root(&static_root)
    }

    fn start_dashboard_only_with_static_root(static_root: &std::path::Path) -> Self {
        Self::start_with_workspace(
            None,
            std::path::Path::new(env!("CARGO_BIN_EXE_commandagent")),
            false,
            static_root,
        )
    }

    fn start_with_workspace(
        workspace: Option<&std::path::Path>,
        cli: &std::path::Path,
        authenticated: bool,
        static_root: &std::path::Path,
    ) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gui_server"));
        command
            .args(["--port", "0", "--base-path", "/"])
            .arg("--repository-root")
            .arg(env!("CARGO_MANIFEST_DIR"))
            .arg("--static-dir")
            .arg(static_root)
            .arg("--commandagent-bin")
            .arg(cli)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(workspace) = workspace {
            command.arg("--execution-root").arg(workspace);
        }
        if authenticated {
            command.env("GUI_TRIAL_TOKEN", TEST_TRIAL_TOKEN);
        } else {
            command.env_remove("GUI_TRIAL_TOKEN");
        }
        let mut child = command.spawn().unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let port = line
            .split("127.0.0.1:")
            .nth(1)
            .and_then(|tail| tail.split('/').next())
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("unable to parse server address: {line}"));
        Self { child, port }
    }

    fn request(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> HttpResponse {
        let origin = format!("http://127.0.0.1:{}", self.port);
        self.request_with_access_and_headers(
            method,
            path,
            body,
            Some(TEST_TRIAL_TOKEN),
            Some(&origin),
            &[],
        )
    }

    fn request_if_none_match(&self, path: &str, etag: &str) -> HttpResponse {
        let origin = format!("http://127.0.0.1:{}", self.port);
        self.request_with_access_and_headers(
            "GET",
            path,
            None,
            Some(TEST_TRIAL_TOKEN),
            Some(&origin),
            &[("If-None-Match", etag)],
        )
    }

    fn request_without_access(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> HttpResponse {
        self.request_with_access(method, path, body, None, None)
    }

    fn request_with_access(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        token: Option<&str>,
        origin: Option<&str>,
    ) -> HttpResponse {
        self.request_with_access_and_headers(method, path, body, token, origin, &[])
    }

    fn request_with_access_and_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<&serde_json::Value>,
        token: Option<&str>,
        origin: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> HttpResponse {
        let body = body.map(ToString::to_string).unwrap_or_default();
        let authorization = token
            .map(|token| format!("Authorization: Bearer {token}\r\n"))
            .unwrap_or_default();
        let origin = origin
            .map(|origin| format!("Origin: {origin}\r\n"))
            .unwrap_or_default();
        let extra_headers = extra_headers
            .iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect::<String>();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{authorization}{origin}{extra_headers}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.port,
            body.len()
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        let (head, body) = response.split_once("\r\n\r\n").unwrap();
        let status = head
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u16>()
            .unwrap();
        let headers = head
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_string()))
            .collect();
        HttpResponse {
            status,
            headers,
            body: body.to_string(),
        }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(unix)]
struct HttpResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    body: String,
}

#[cfg(unix)]
impl HttpResponse {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }
}

fn session_spec() -> serde_json::Value {
    serde_json::json!({
        "goal": "Create a CLI --pattern filter command",
        "profile": "python-cli",
        "provider": "ollama",
        "model": "fixture-executor",
        "planner_provider": "ollama",
        "planner_model": "fixture-planner"
    })
}

#[cfg(unix)]
fn write_terminal_cli(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::write(
        path,
        "#!/bin/sh\nprintf '%s\\n' '{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"failed\",\"assurance_level\":\"none\"}' > \"$COMMANDAGENT_EVAL_EVENTS\"\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}
