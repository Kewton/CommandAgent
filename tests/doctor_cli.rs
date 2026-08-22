use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Output};

fn run_doctor(workspace: &std::path::Path, key: Option<&str>) -> Output {
    let home = workspace.join("home");
    let state = workspace.join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_commandagent"));
    command.args([
        "--doctor",
        "--json",
        "--provider",
        "openai",
        "--planner-provider",
        "openai",
        "--model",
        "executor-test-model",
        "--planner-model",
        "planner-test-model",
        "--cwd",
        workspace.to_str().unwrap(),
        "--state-dir",
        state.to_str().unwrap(),
    ]);
    command.env("HOME", home);
    command.env("PATH", "");
    command.env_remove("OPENAI_API_KEY");
    if let Some(key) = key {
        command.env("OPENAI_API_KEY", key);
    }
    command.output().unwrap()
}

fn run_preset_doctor(workspace: &std::path::Path, config: &str) -> Output {
    let home = workspace.join("home");
    let state = workspace.join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::create_dir_all(workspace.join(".commandagent")).unwrap();
    std::fs::write(workspace.join(".commandagent/config.toml"), config).unwrap();
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--doctor",
            "--json",
            "--preset",
            "selected",
            "--cwd",
            workspace.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
        ])
        .env("HOME", home)
        .env("PATH", "")
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap()
}

const COMPLETE_SELECTED_PRESET: &str = concat!(
    "[preset.selected]\n",
    "model = \"executor-test-model\"\n",
    "provider = \"openai\"\n",
    "planner_model = \"planner-test-model\"\n",
    "planner_provider = \"openai\"\n",
    "context_budget = 32768\n",
    "chat_timeout_secs = 180\n",
    "plan_preset = \"none\"\n",
    "profile = \"generic\"\n",
    "narration = \"quiet\"\n",
    "footer = \"off\"\n",
    "stream = \"off\"\n",
);

#[test]
fn doctor_json_warn_only_exits_zero_and_redacts_credentials() {
    let workspace = tempfile::tempdir().unwrap();
    let output = run_doctor(workspace.path(), Some("integration-secret"));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(output.status.success(), "stderr={:?}", output.stderr);
    assert_eq!(report["schema_version"], "1");
    assert_eq!(report["status"], "warn");
    assert!(
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| { check["id"] == "config.model" && check["details"]["source"] == "cli" })
    );
    assert!(stdout.contains("<redacted>"));
    assert!(!stdout.contains("integration-secret"));
}

#[test]
fn doctor_json_failure_exits_nonzero_and_still_emits_report() {
    let workspace = tempfile::tempdir().unwrap();
    let output = run_doctor(workspace.path(), None);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(!output.status.success());
    assert_eq!(report["status"], "fail");
    let key_check = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "provider.openai.api_key")
        .unwrap();
    assert_eq!(key_check["status"], "fail");
    assert_eq!(key_check["details"]["present"], false);
}

#[test]
fn doctor_reports_missing_keys_for_an_unresolvable_incomplete_preset() {
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::create_dir_all(workspace.path().join(".commandagent")).unwrap();
    std::fs::write(
        workspace.path().join(".commandagent/config.toml"),
        "[preset.partial]\nprovider = \"ollama\"\nplanner_provider = \"gemini\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--doctor",
            "--json",
            "--preset",
            "partial",
            "--cwd",
            workspace.path().to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
        ])
        .env("HOME", home)
        .env("PATH", "")
        .output()
        .unwrap();
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    assert!(!output.status.success());
    let preset = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "config.preset")
        .unwrap();
    assert_eq!(preset["status"], "fail");
    assert!(
        preset["details"]["missing_keys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|key| key == "planner_model")
    );
    assert!(preset["message"].as_str().unwrap().contains("missing keys"));
}

#[test]
fn doctor_preserves_other_preset_validation_error_without_reporting_selected_not_found() {
    let workspace = tempfile::tempdir().unwrap();
    let config = format!("{COMPLETE_SELECTED_PRESET}[preset.other]\nprovder = \"openai\"\n");
    let output = run_preset_doctor(workspace.path(), &config);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let preset = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "config.preset")
        .unwrap();
    let message = preset["message"].as_str().unwrap();

    assert!(!output.status.success());
    assert!(message.contains("preset.other.provder"), "{message}");
    assert!(message.contains("could not be inspected"), "{message}");
    assert!(!message.contains("was not found"), "{message}");
}

#[test]
fn doctor_reports_malformed_toml_as_syntax_error_instead_of_unknown_key() {
    let workspace = tempfile::tempdir().unwrap();
    let config = format!("{COMPLETE_SELECTED_PRESET}{{ malformed = \"toml\" }}\n");
    let output = run_preset_doctor(workspace.path(), &config);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let config_file = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "config.file.workspace_commandagent")
        .unwrap();
    let message = config_file["message"].as_str().unwrap();

    assert!(!output.status.success());
    assert!(message.contains("invalid TOML syntax"), "{message}");
    assert!(!message.contains("unknown config key"), "{message}");
}

#[test]
fn doctor_lists_external_draft_profile_hash_and_cli_requires_its_extension_root() {
    let workspace = tempfile::tempdir().unwrap();
    let extension = tempfile::tempdir().unwrap();
    let state = workspace.path().join("state");
    let home = workspace.path().join("home");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let profile_dir = extension.path().join("profiles/static-site");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(
        profile_dir.join("manifest.toml"),
        include_str!(
            "corpus/apps/issue117-draft-profile/extension-root/profiles/static-site/manifest.toml"
        ),
    )
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let read = stream.read(&mut request).unwrap();
        assert!(String::from_utf8_lossy(&request[..read]).contains("GET /api/tags"));
        let body = r#"{"models":[{"name":"doctor-fixture"}]}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });

    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--doctor",
            "--json",
            "--profile",
            "static-site",
            "--model",
            "doctor-fixture",
            "--planner-model",
            "doctor-fixture",
            "--context-budget",
            "32768",
            "--ollama-host",
        ])
        .arg(format!("http://{address}"))
        .arg("--cwd")
        .arg(workspace.path())
        .arg("--state-dir")
        .arg(&state)
        .arg("--extension-root")
        .arg(extension.path())
        .env("HOME", &home)
        .env("PATH", "")
        .output()
        .unwrap();
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let context_budget = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "config.context_budget")
        .unwrap();
    assert_eq!(context_budget["details"]["value"], 32_768);
    assert_eq!(context_budget["details"]["ollama_num_ctx"], 32_768);
    assert_eq!(
        context_budget["details"]["ollama_roles"],
        serde_json::json!(["executor", "planner"])
    );
    let profiles = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["id"] == "profile.extensions")
        .unwrap();
    assert_eq!(profiles["status"], "warn");
    assert_eq!(profiles["details"]["profiles"][0]["id"], "static-site");
    assert_eq!(
        profiles["details"]["profiles"][0]["assurance_ceiling"],
        "static"
    );
    assert!(
        profiles["details"]["profiles"][0]["manifest_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );

    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--profile", "static-site", "--cwd"])
        .arg(workspace.path())
        .arg("Create the requested site")
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requires an extension root"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
