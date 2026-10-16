use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-probe-verdicts.sh")
}

fn run_probe_verifier(arg_path: &Path) -> Output {
    Command::new("bash")
        .arg(script_path())
        .arg(arg_path)
        .env_remove("COMMANDAGENT_PROBE_VERDICT_FILE")
        .output()
        .expect("failed to execute check-probe-verdicts.sh")
}

fn combined_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    format!("{stdout}\n{stderr}")
}

#[test]
fn unverified_verdict_fails_with_error_annotation() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let file_path = temp.path().join("probe-verdicts.txt");
    std::fs::write(
        &file_path,
        "issue667/tiocsti: UNVERIFIED (could not write legacy_tiocsti)\n\
         issue502/sandbox: unverified: backend not enabled\n",
    )
    .expect("write verdicts");

    let output = run_probe_verifier(&file_path);
    assert!(
        !output.status.success(),
        "script must exit non-zero when UNVERIFIED lines are present, got: {:?}",
        output.status
    );
    let out = combined_output(&output);
    assert!(
        out.contains("::error::"),
        "expected '::error::' in output, got:\n{out}"
    );
    assert!(
        out.contains("::error::issue667/tiocsti: UNVERIFIED (could not write legacy_tiocsti)"),
        "expected line-by-line annotation for uppercase UNVERIFIED, got:\n{out}"
    );
    assert!(
        out.contains("::error::issue502/sandbox: unverified: backend not enabled"),
        "expected line-by-line annotation for lowercase unverified, got:\n{out}"
    );
    assert!(
        out.contains("dev.tty.legacy_tiocsti"),
        "expected failure message to mention dev.tty.legacy_tiocsti, got:\n{out}"
    );
}

#[test]
fn empty_verdict_file_fails_with_error_annotation() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let file_path = temp.path().join("probe-verdicts.txt");
    std::fs::write(&file_path, "").expect("write empty verdicts");

    let output = run_probe_verifier(&file_path);
    assert!(
        !output.status.success(),
        "script must exit non-zero for empty verdict file, got: {:?}",
        output.status
    );
    let out = combined_output(&output);
    assert!(
        out.contains("::error::"),
        "expected '::error::' in output for empty file, got:\n{out}"
    );
    assert!(
        out.contains("dev.tty.legacy_tiocsti"),
        "expected failure message to mention dev.tty.legacy_tiocsti, got:\n{out}"
    );
}

#[test]
fn missing_verdict_file_fails_with_error_annotation() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let file_path = temp.path().join("nonexistent-probe-verdicts.txt");

    let output = run_probe_verifier(&file_path);
    assert!(
        !output.status.success(),
        "script must exit non-zero when verdict file is missing, got: {:?}",
        output.status
    );
    let out = combined_output(&output);
    assert!(
        out.contains("::error::"),
        "expected '::error::' in output for missing file, got:\n{out}"
    );
    assert!(
        out.contains("dev.tty.legacy_tiocsti"),
        "expected failure message to mention dev.tty.legacy_tiocsti, got:\n{out}"
    );
}

#[test]
fn blocked_only_verdicts_succeed_without_error() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let file_path = temp.path().join("probe-verdicts.txt");
    std::fs::write(
        &file_path,
        "issue667/tiocsti: blocked; legacy_tiocsti=1\n\
         issue502/sandbox: blocked; confinement verified\n",
    )
    .expect("write verdicts");

    let output = run_probe_verifier(&file_path);
    assert!(
        output.status.success(),
        "script must exit 0 when only BLOCKED verdicts are recorded, got: {:?}",
        output.status
    );
    let out = combined_output(&output);
    assert!(
        !out.contains("::error::"),
        "expected no '::error::' in output, got:\n{out}"
    );
    assert!(
        out.contains("isolation probe verdicts"),
        "expected header to be printed, got:\n{out}"
    );
    assert!(
        out.contains("issue667/tiocsti: blocked; legacy_tiocsti=1"),
        "expected verdict contents to be printed, got:\n{out}"
    );
}
