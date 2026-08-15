use std::process::Command;

#[test]
fn gui_server_help_exposes_only_serving_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_gui_server"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for option in ["--port", "--base-path", "--static-dir", "--repository-root"] {
        assert!(help.contains(option), "missing {option}: {help}");
    }
    for forbidden in ["provider", "execute", "mutation"] {
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
