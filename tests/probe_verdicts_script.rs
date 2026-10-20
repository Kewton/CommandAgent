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

const TIOCSTI_BLOCKED: &str = "issue667/tiocsti: blocked; legacy_tiocsti=1\n";
const MPTCP_REFUSED: &str = "issue672/mptcp: REFUSED; kernel=6.8 abi=4 net.mptcp.enabled=1\n";
const MPTCP_ALLOWED: &str = "issue672/mptcp: ALLOWED; kernel=6.8 abi=4 net.mptcp.enabled=1\n";

fn verify_contents(contents: &str) -> (bool, String) {
    let temp = tempfile::tempdir().expect("create tempdir");
    let file_path = temp.path().join("probe-verdicts.txt");
    std::fs::write(&file_path, contents).expect("write verdicts");
    let output = run_probe_verifier(&file_path);
    (output.status.success(), combined_output(&output))
}

#[test]
fn both_required_verdicts_succeed_without_error() {
    let contents = format!(
        "{TIOCSTI_BLOCKED}{MPTCP_REFUSED}issue502/sandbox: blocked; confinement verified\n"
    );
    let (ok, out) = verify_contents(&contents);
    assert!(ok, "script must exit 0 with both required verdicts:\n{out}");
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

#[test]
fn mptcp_allowed_is_recorded_as_a_kernel_limit_and_succeeds() {
    let (ok, out) = verify_contents(&format!("{TIOCSTI_BLOCKED}{MPTCP_ALLOWED}"));
    assert!(ok, "MPTCP ALLOWED is an accepted recorded limit:\n{out}");
    assert!(
        out.contains("issue672/mptcp: ALLOWED"),
        "ALLOWED must be printed as recorded, not rewritten:\n{out}"
    );
    assert!(
        !out.contains("REFUSED"),
        "ALLOWED must not be read as a refusal:\n{out}"
    );
}

#[test]
fn blocked_only_verdicts_without_mptcp_fail() {
    let (ok, out) = verify_contents(
        "issue667/tiocsti: blocked; legacy_tiocsti=1\n\
         issue502/sandbox: blocked; confinement verified\n",
    );
    assert!(!ok, "a missing MPTCP verdict must fail:\n{out}");
    assert!(
        out.contains("::error::required isolation probe verdict is missing: issue672/mptcp"),
        "{out}"
    );
}

#[test]
fn missing_tiocsti_verdict_fails() {
    let (ok, out) = verify_contents(MPTCP_REFUSED);
    assert!(!ok, "a missing TIOCSTI verdict must fail:\n{out}");
    assert!(
        out.contains("::error::required isolation probe verdict is missing: issue667/tiocsti"),
        "{out}"
    );
}

#[test]
fn empty_verdicts_for_required_ids_fail() {
    for line in ["issue672/mptcp:\n", "issue672/mptcp: ;kernel=6.8\n"] {
        let (ok, out) = verify_contents(&format!("{TIOCSTI_BLOCKED}{line}"));
        assert!(!ok, "an empty MPTCP verdict must fail ({line:?}):\n{out}");
        assert!(out.contains("verdict is empty: issue672/mptcp"), "{out}");
    }
    let (ok, out) = verify_contents(&format!("issue667/tiocsti:\n{MPTCP_REFUSED}"));
    assert!(!ok, "an empty TIOCSTI verdict must fail:\n{out}");
    assert!(out.contains("verdict is empty: issue667/tiocsti"), "{out}");
}

#[test]
fn unknown_verdicts_for_required_ids_fail() {
    let (ok, out) = verify_contents(&format!(
        "{TIOCSTI_BLOCKED}issue672/mptcp: maybe; kernel=6.8\n"
    ));
    assert!(!ok, "an unknown MPTCP verdict must fail:\n{out}");
    assert!(out.contains("not accepted: issue672/mptcp: maybe"), "{out}");
    let (ok, out) = verify_contents(&format!(
        "issue667/tiocsti: allowed; legacy_tiocsti=1\n{MPTCP_REFUSED}"
    ));
    assert!(
        !ok,
        "a TIOCSTI verdict other than blocked must fail:\n{out}"
    );
    assert!(
        out.contains("not accepted: issue667/tiocsti: allowed"),
        "{out}"
    );
    // The MPTCP verdict words are not valid for TIOCSTI.
    let (ok, out) = verify_contents(&format!(
        "issue667/tiocsti: REFUSED; legacy_tiocsti=1\n{MPTCP_REFUSED}"
    ));
    assert!(!ok, "REFUSED is not a TIOCSTI verdict:\n{out}");
}

#[test]
fn unverified_required_verdicts_fail_even_with_the_other_id_present() {
    let (ok, out) = verify_contents(&format!(
        "{TIOCSTI_BLOCKED}issue672/mptcp: UNVERIFIED (socket EPROTONOSUPPORT); kernel=6.8\n"
    ));
    assert!(!ok, "UNVERIFIED MPTCP must fail:\n{out}");
    assert!(out.contains("::error::issue672/mptcp: UNVERIFIED"), "{out}");
    let (ok, out) = verify_contents(&format!(
        "issue667/tiocsti: unverified: no tty; legacy_tiocsti=1\n{MPTCP_REFUSED}"
    ));
    assert!(!ok, "UNVERIFIED TIOCSTI must fail:\n{out}");
}

#[test]
fn duplicate_required_verdicts_fail() {
    let (ok, out) = verify_contents(&format!("{TIOCSTI_BLOCKED}{MPTCP_REFUSED}{MPTCP_ALLOWED}"));
    assert!(!ok, "a duplicated MPTCP verdict must fail:\n{out}");
    assert!(out.contains("recorded 2 times: issue672/mptcp"), "{out}");
}

#[test]
fn required_ids_match_whole_lines_only() {
    // A prefix-sharing id or an id mentioned mid-line is not the required line.
    let (ok, out) = verify_contents(&format!(
        "{TIOCSTI_BLOCKED}issue672/mptcp-extra: REFUSED\nnote issue672/mptcp: REFUSED\n"
    ));
    assert!(!ok, "only an exact id line counts:\n{out}");
    assert!(out.contains("missing: issue672/mptcp"), "{out}");
}
