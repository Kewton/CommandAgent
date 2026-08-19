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

    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args(["--doctor", "--json", "--profile", "static-site"])
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
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
