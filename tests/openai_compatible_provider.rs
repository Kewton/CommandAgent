use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;

#[test]
fn binary_accepts_generic_provider_and_doctor_probes_mock_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 4096];
        let read = stream.read(&mut request).unwrap();
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("GET /v1/models "), "{request}");
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        let body = r#"{"object":"list","data":[{"id":"served-model"}]}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let workspace = tempfile::tempdir().unwrap();
    let home = workspace.path().join("home");
    let state = workspace.path().join("state");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&state).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--doctor",
            "--json",
            "--provider",
            "openai-compatible",
            "--model",
            "served-model",
            "--base-url",
            &format!("http://{address}/v1"),
            "--cwd",
            workspace.path().to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
        ])
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
    let checks = report["checks"].as_array().unwrap();
    let provider = checks
        .iter()
        .find(|check| check["id"] == "config.provider")
        .unwrap();
    assert_eq!(provider["details"]["value"], "openai-compatible");
    let reachable = checks
        .iter()
        .find(|check| check["id"] == "provider.openai_compatible.reachable")
        .unwrap();
    assert_eq!(reachable["status"], "pass");
    assert_eq!(reachable["details"]["reachable"], true);
    assert!(
        checks
            .iter()
            .all(|check| check["id"] != "provider.lm_studio.reachable"),
        "generic provider must retain its own diagnostic identity"
    );
    for role in ["executor", "planner", "classifier"] {
        let id = format!("provider.openai_compatible.{role}_model");
        let model = checks.iter().find(|check| check["id"] == id).unwrap();
        assert_eq!(model["status"], "pass");
    }
}

#[test]
fn binary_help_advertises_generic_provider_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for expected in ["openai-compatible", "--base-url", "--api-key-env"] {
        assert!(help.contains(expected), "missing {expected} in {help}");
    }
}
