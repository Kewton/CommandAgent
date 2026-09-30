#![cfg(unix)]

use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;
use std::sync::Mutex;

use commandagent::mode::ExecutionMode;
use commandagent::planner::repair::{
    RecoveryHandoff, RepairContext, save_recovery_ultra_plan, save_repair_report_with_context,
    save_ultra_recovery_prompt,
};
use commandagent::planner::verify::VerificationReport;
use commandagent::sensitive_data::{SecretCatalog, current, install_scope, reset_scopes_for_tests};
use commandagent::tools::bash::{self, BashOutcomeKind};
use commandagent::tools::registry::{ToolContext, ToolRegistry, tool_error_kind};
use commandagent::tools::workspace_policy::WorkspacePolicy;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const EVENTS: &str =
    include_str!("corpus/apps/issue441-placeholder/fixtures/i2-events-196-213.jsonl");

/// Mirrors `tools::placeholder_path::PREFIX_ENCODING`, which is crate-private.
const PREFIX_ENCODING: &str = "omitted; the raw command prefix is never persisted in a run";

/// The fake canary. Scope-less rejections must never persist it.
const C1: &str = "H01_546_FAKE_canary_zz";

/// The fake credential value. It is registered in a scope, so another thread's
/// scope-less rejection must not persist it either.
const C2: &str = "H01_546_FAKE_registered_key_77";

/// Issue #557 fake values. `C` is used without a scope; `R` is registered as a
/// credential before a run scope is installed.
const C: &str = "H01_557_FAKE_canary_qq";
const R: &str = "H01_557_FAKE_registered_key_88";

/// The scope registry is process-global, so tests that install or reset a scope
/// share one lock.
static SCOPE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn scope_guard() -> std::sync::MutexGuard<'static, ()> {
    SCOPE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn context(root: &Path, events: &Path) -> ToolContext {
    ToolContext {
        root: root.to_path_buf(),
        mode: ExecutionMode::Act,
        auto_approve: true,
        interactive_approval: false,
        offline: true,
        workspace_policy: WorkspacePolicy::NormalTask,
        eval_events_path: Some(events.to_path_buf()),
        expected_paths: Vec::new(),
        protected_paths: Vec::new(),
    }
}

fn event_values(text: &str) -> Vec<Value> {
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn private_records(root: &Path) -> Vec<Value> {
    let dir = root.join(".commandagent/evidence/bash-placeholders");
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
        })
        .collect()
}

fn private_record_texts(root: &Path) -> Vec<String> {
    let dir = root.join(".commandagent/evidence/bash-placeholders");
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect()
}

/// The raw text must not contain the fake value as a string, nor its 10-decimal
/// byte array.
fn assert_no_fake_value(text: &str, value: &str) {
    assert!(
        !text.contains(value),
        "raw evidence kept the fake value string: {text}"
    );
    let decimal = value
        .bytes()
        .map(|byte| byte.to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert!(
        !text.contains(&decimal),
        "raw evidence kept the fake value's decimal byte array: {text}"
    );
}

/// The four fields retained per the #546 contract: an empty prefix, the one
/// constant encoding, the original byte length, and the full-command hash.
fn assert_omitted_prefix(record: &Value, command: &str) {
    assert_eq!(record["command_prefix_bytes"], json!([]));
    assert_eq!(record["prefix_encoding"], PREFIX_ENCODING);
    assert_eq!(record["command_bytes"], command.len());
    assert_eq!(
        record["command_sha256"],
        format!("{:x}", Sha256::digest(command.as_bytes()))
    );
}

/// Issue #557: the error's Display, Debug, and alternate (`{:#}`) forms must
/// none of them contain the fake value that was embedded in the command.
fn assert_error_omits(error: &anyhow::Error, value: &str) {
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        format!("{error:#}"),
    ] {
        assert!(
            !rendered.contains(value),
            "the error kept the fake value `{value}`: {rendered}"
        );
    }
}

#[test]
fn literal_placeholders_are_rejected_before_normalization_or_shell_execution() {
    let root = tempfile::tempdir().unwrap();
    let events = root.path().join("events.jsonl");
    let commands = [
        "cd /Users/<user>/x && cat a",
        "cd '/Users/<redacted>/x' && cat a",
        "printf forbidden > /Users/<user>/x",
        "printf '<redacted>' > written.txt",
        "echo <user> && touch written.txt",
    ];
    for command in commands {
        let rejection = bash::path_confinement_rejection(command, root.path()).unwrap();
        assert_eq!(rejection.operation, "placeholder path");
        let error = ToolRegistry::default()
            .execute(
                "Bash",
                &json!({"command":command}),
                &context(root.path(), &events),
            )
            .unwrap_err();
        assert_eq!(tool_error_kind(&error), "bash_path_confinement_error");
        assert!(
            error.to_string().contains("placeholder path detected"),
            "{error}"
        );
        assert!(error.to_string().contains("workspace-relative"), "{error}");
        assert!(!error.to_string().contains("redirection target"), "{error}");
        assert!(!root.path().join("written.txt").exists());
    }
    let records = private_records(root.path());
    let public = event_values(&std::fs::read_to_string(events).unwrap());
    assert_eq!(records.len(), commands.len());
    assert_eq!(public.len(), commands.len());
    for (event, command) in public.iter().zip(commands) {
        assert_eq!(event["event"], "bash_path_confinement_rejected");
        assert_eq!(event["schema_version"], "1");
        assert_eq!(event["placeholder_detected"], true);
        assert_eq!(
            event["command_sha256"],
            format!("{:x}", Sha256::digest(command))
        );
        let record = records
            .iter()
            .find(|record| record["command_sha256"] == event["command_sha256"])
            .unwrap();
        assert_omitted_prefix(record, command);
        assert!(event.get("command_prefix_bytes").is_none());
    }
}

#[test]
fn evidence_hashes_original_bytes_even_when_cd_could_be_stripped() {
    let root = tempfile::tempdir().unwrap();
    let real_cd = format!("cd '{}' && printf real-workspace", root.path().display());
    assert_eq!(
        bash::strip_workspace_root_cd_prefix(&real_cd, root.path())
            .unwrap()
            .normalized_command,
        "printf real-workspace"
    );
    assert!(
        bash::run(&real_cd, root.path(), true)
            .unwrap()
            .contains("outcome: Success")
    );
    // A command that is not rejected never creates the evidence directory.
    assert!(
        !root
            .path()
            .join(".commandagent/evidence/bash-placeholders")
            .exists()
    );
    let command = format!("cd '{}' && printf '<user> 日本語'", root.path().display());
    assert!(bash::strip_workspace_root_cd_prefix(&command, root.path()).is_none());
    let result = bash::run_structured(
        &command,
        root.path(),
        true,
        std::time::Duration::from_secs(1),
        || false,
    )
    .unwrap();
    assert_eq!(result.kind, BashOutcomeKind::Blocked);
    let records = private_records(root.path());
    assert_omitted_prefix(&records[0], &command);
    assert_eq!(records[0]["schema_version"], "1");
    assert_eq!(records[0].as_object().unwrap().len(), 5);

    let command = format!("echo {}<redacted>", "日".repeat(24));
    bash::run(&command, root.path(), true).unwrap_err();
    let records = private_records(root.path());
    let record = records
        .iter()
        .find(|record| record["command_sha256"] == format!("{:x}", Sha256::digest(&command)))
        .unwrap();
    assert_omitted_prefix(record, &command);
    assert_eq!(record["command_bytes"], 87);
    assert!(std::str::from_utf8(&command.as_bytes()[..64]).is_err());
}

#[test]
fn evidence_prefix_boundaries_preserve_full_command_hashes() {
    let exact = format!("echo '<user>{}'", "x".repeat(51));
    let commands = [
        ("echo '<user>'".to_string(), 13),
        (exact.clone(), 64),
        (format!("{exact} A"), 66),
        (format!("{exact} B"), 66),
    ];
    let mut records = Vec::new();
    for (command, expected_bytes) in commands {
        assert_eq!(command.len(), expected_bytes);
        let root = tempfile::tempdir().unwrap();
        let error = bash::run(&command, root.path(), true).unwrap_err();
        assert!(error.to_string().contains("placeholder path detected"));
        let evidence = private_records(root.path());
        assert_eq!(evidence.len(), 1);
        let record = &evidence[0];
        assert_eq!(record["schema_version"], "1");
        assert_eq!(record["command_bytes"], expected_bytes);
        assert_eq!(
            record["command_sha256"],
            format!("{:x}", Sha256::digest(command.as_bytes()))
        );
        assert_eq!(record["command_prefix_bytes"], json!([]));
        assert_eq!(record["prefix_encoding"], PREFIX_ENCODING);
        assert_eq!(record["command_prefix_bytes"].as_array().unwrap().len(), 0);
        records.push(record.clone());
    }
    for record in &records[2..] {
        assert_eq!(record["command_prefix_bytes"], json!([]));
        assert_eq!(
            record["command_prefix_bytes"],
            records[1]["command_prefix_bytes"]
        );
        assert_ne!(record["command_sha256"], records[1]["command_sha256"]);
    }
    assert_ne!(records[2]["command_sha256"], records[3]["command_sha256"]);
    // 66A and 66B share the empty prefix and differ only by their full hash.
    assert_eq!(records[2]["command_prefix_bytes"], json!([]));
    assert_eq!(records[3]["command_prefix_bytes"], json!([]));
    assert_eq!(records[2]["command_bytes"], 66);
    assert_eq!(records[3]["command_bytes"], 66);
}

#[test]
fn unscoped_bash_api_omits_the_raw_prefix_and_the_fake_value() {
    let _guard = scope_guard();
    reset_scopes_for_tests();
    assert!(current().is_none());

    let commands = [
        format!("echo {C1} /Users/<user>/x"),
        format!("echo 秘密の偽値です_{C1} <user>"),
        format!("echo <user> {} {C1}", "x".repeat(70)),
    ];
    for command in commands {
        let root = tempfile::tempdir().unwrap();
        let error = bash::run(&command, root.path(), true).unwrap_err();
        assert!(
            error.to_string().contains("placeholder path detected"),
            "{error}"
        );
        // The rejection message never echoes the fake value.
        assert!(!error.to_string().contains(C1), "{error}");
        let records = private_records(root.path());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["schema_version"], "1");
        assert_eq!(records[0].as_object().unwrap().len(), 5);
        assert_omitted_prefix(&records[0], &command);
        for text in private_record_texts(root.path()) {
            assert_no_fake_value(&text, C1);
        }
    }
}

#[test]
fn unscoped_run_checked_and_cancel_and_force_share_the_empty_prefix() {
    let _guard = scope_guard();
    reset_scopes_for_tests();
    let command = format!("echo <redacted> {C1}");

    let root = tempfile::tempdir().unwrap();
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    assert!(
        error.to_string().contains("placeholder path detected"),
        "{error}"
    );
    let records = private_records(root.path());
    assert_eq!(records.len(), 1);
    assert_omitted_prefix(&records[0], &command);

    let root = tempfile::tempdir().unwrap();
    let error = bash::run_with_cancel_and_force(&command, root.path(), true, || false, || false)
        .unwrap_err();
    assert!(
        error.to_string().contains("placeholder path detected"),
        "{error}"
    );
    let records = private_records(root.path());
    assert_eq!(records.len(), 1);
    assert_omitted_prefix(&records[0], &command);
}

/// Issue #557 rows 1-2: `run_checked` refuses a placeholder command and returns
/// only the summary, whose prefix identifies the refusal. The command — and so
/// the fake value — never appears, with or without a scope.
#[test]
fn run_checked_placeholder_rejection_error_omits_the_command() {
    let _guard = scope_guard();
    reset_scopes_for_tests();

    let root = tempfile::tempdir().unwrap();
    let command = format!("echo <redacted> {C}");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("bash_path_confinement_error: placeholder path detected;"),
        "{error}"
    );
    assert_error_omits(&error, C);
    let records = private_records(root.path());
    assert_eq!(records.len(), 1);
    assert_omitted_prefix(&records[0], &command);

    let root = tempfile::tempdir().unwrap();
    let mut catalog = SecretCatalog::new();
    catalog.register_credential("FAKE_KEY", R).unwrap();
    install_scope(catalog, Some(root.path()), None);
    let command = format!("echo <redacted> {R}");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("bash_path_confinement_error: placeholder path detected;"),
        "{error}"
    );
    assert_error_omits(&error, R);
    reset_scopes_for_tests();
}

/// Issue #557 rows 3-4: a failure with no diagnostic line reports the fixed
/// `command did not succeed` summary (no command), keeping `command failed`,
/// the outcome kind, and the status line. Scope-less and scoped alike.
#[test]
fn run_checked_failure_error_omits_the_command() {
    let _guard = scope_guard();
    reset_scopes_for_tests();

    let root = tempfile::tempdir().unwrap();
    let command = format!("test -n {C} && false");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("command failed"), "{message}");
    assert!(message.contains("outcome: CommandFailed"), "{message}");
    assert!(
        message.contains("\nsummary: command did not succeed\n"),
        "{message}"
    );
    assert_error_omits(&error, C);

    let root = tempfile::tempdir().unwrap();
    let mut catalog = SecretCatalog::new();
    catalog.register_credential("FAKE_KEY", R).unwrap();
    install_scope(catalog, Some(root.path()), None);
    let command = format!("test -n {R} && false");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("command failed"), "{message}");
    assert!(message.contains("outcome: CommandFailed"), "{message}");
    assert_error_omits(&error, R);
    reset_scopes_for_tests();
}

/// Issue #557 row 5: a failure with an extracted error line keeps that line and
/// the status, and still omits the command.
#[test]
fn run_checked_failure_with_error_line_omits_the_command() {
    let _guard = scope_guard();
    reset_scopes_for_tests();
    let root = tempfile::tempdir().unwrap();
    let command = format!("test -n {C} && echo build error >&2 && exit 3");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("build error"), "{message}");
    assert!(message.contains("status: exit status: 3"), "{message}");
    assert_error_omits(&error, C);
}

/// Issue #557 rows 7-8: a structural or credential refusal returns the summary
/// only, and the summary never contains the command.
#[test]
fn run_checked_refusal_error_is_the_summary_only() {
    let _guard = scope_guard();
    reset_scopes_for_tests();

    let root = tempfile::tempdir().unwrap();
    let command = format!("sudo echo {C}");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    assert_eq!(error.to_string(), "dangerous command blocked");
    assert_error_omits(&error, C);

    let root = tempfile::tempdir().unwrap();
    let command = format!("cat .env # {C}");
    let error = bash::run_checked(&command, root.path(), true).unwrap_err();
    assert!(
        error.to_string().starts_with("workspace_policy_blocked:"),
        "{error}"
    );
    assert_error_omits(&error, C);
}

/// Issue #557 row 9 (K): a successful `run_checked` returns the unchanged body.
#[test]
fn run_checked_success_body_is_unchanged() {
    let _guard = scope_guard();
    reset_scopes_for_tests();
    let root = tempfile::tempdir().unwrap();
    let command = format!("echo ok {C}");
    let body = bash::run_checked(&command, root.path(), true).unwrap();
    assert!(body.contains("outcome: Success"), "{body}");
    assert!(body.contains(&format!("ok {C}")), "{body}");
}

/// Issue #557 rows 12-13 (K): `run` refusals stay summary-only.
#[test]
fn run_refusal_errors_are_the_summary_only() {
    let _guard = scope_guard();
    reset_scopes_for_tests();

    let root = tempfile::tempdir().unwrap();
    let command = format!("echo <redacted> {C}");
    let error = bash::run(&command, root.path(), true).unwrap_err();
    assert!(
        error.to_string().contains("placeholder path detected"),
        "{error}"
    );
    assert_error_omits(&error, C);

    let root = tempfile::tempdir().unwrap();
    let command = format!("sudo echo {C}");
    let error = bash::run(&command, root.path(), true).unwrap_err();
    assert_eq!(error.to_string(), "dangerous command blocked");
    assert_error_omits(&error, C);

    let root = tempfile::tempdir().unwrap();
    let command = format!("cat .env # {C}");
    let error = bash::run(&command, root.path(), true).unwrap_err();
    assert!(
        error.to_string().starts_with("workspace_policy_blocked:"),
        "{error}"
    );
    assert_error_omits(&error, C);
}

/// Issue #557 row 14 (K): `run` returns a failure as `Ok`, and that body is the
/// model tool-result shape, so the `command did not succeed: <command>` summary
/// is preserved.
#[test]
fn run_failure_ok_body_keeps_the_command() {
    let _guard = scope_guard();
    reset_scopes_for_tests();
    let root = tempfile::tempdir().unwrap();
    let command = format!("test -n {C} && false");
    let body = bash::run(&command, root.path(), true).unwrap();
    assert!(body.contains("outcome: CommandFailed"), "{body}");
    assert!(
        body.contains(&format!("summary: command did not succeed: {command}")),
        "{body}"
    );
}

#[test]
fn a_scope_registered_on_another_thread_still_omits_the_raw_prefix() {
    let _guard = scope_guard();
    reset_scopes_for_tests();

    // An empty scope is registered for the root, but the call runs on a fresh
    // thread that never installed the thread-local scope.
    let root = tempfile::tempdir().unwrap();
    install_scope(SecretCatalog::new(), Some(root.path()), None);
    let command = format!("echo {C1} /Users/<user>/x");
    let message = std::thread::spawn({
        let root = root.path().to_path_buf();
        let command = command.clone();
        move || bash::run(&command, &root, true).unwrap_err().to_string()
    })
    .join()
    .unwrap();
    assert!(message.contains("placeholder path detected"), "{message}");
    let records = private_records(root.path());
    assert_eq!(records.len(), 1);
    assert_omitted_prefix(&records[0], &command);
    for text in private_record_texts(root.path()) {
        assert_no_fake_value(&text, C1);
    }
    reset_scopes_for_tests();

    // A registered credential can no longer leak from another thread either.
    let root = tempfile::tempdir().unwrap();
    let mut catalog = SecretCatalog::new();
    catalog.register_credential("FAKE_TOKEN", C2).unwrap();
    install_scope(catalog, Some(root.path()), None);
    let command = format!("echo {C2} /Users/<user>/x");
    let message = std::thread::spawn({
        let root = root.path().to_path_buf();
        let command = command.clone();
        move || bash::run(&command, &root, true).unwrap_err().to_string()
    })
    .join()
    .unwrap();
    assert!(message.contains("placeholder path detected"), "{message}");
    assert!(!message.contains(C2), "{message}");
    let records = private_records(root.path());
    assert_eq!(records.len(), 1);
    assert_omitted_prefix(&records[0], &command);
    for text in private_record_texts(root.path()) {
        assert_no_fake_value(&text, C2);
    }
    reset_scopes_for_tests();
}

#[test]
fn the_record_shape_and_prefix_constant_do_not_depend_on_a_scope() {
    let _guard = scope_guard();
    reset_scopes_for_tests();
    let command = format!("echo {C1} /Users/<user>/x");

    // Without a scope.
    let root = tempfile::tempdir().unwrap();
    bash::run(&command, root.path(), true).unwrap_err();
    let unscoped = private_records(root.path()).remove(0);

    // With a run scope installed on the same thread.
    let root = tempfile::tempdir().unwrap();
    install_scope(SecretCatalog::new(), Some(root.path()), None);
    bash::run(&command, root.path(), true).unwrap_err();
    let scoped = private_records(root.path()).remove(0);
    reset_scopes_for_tests();

    for record in [&unscoped, &scoped] {
        let object = record.as_object().unwrap();
        assert_eq!(object.len(), 5);
        for key in [
            "schema_version",
            "command_sha256",
            "command_bytes",
            "command_prefix_bytes",
            "prefix_encoding",
        ] {
            assert!(object.contains_key(key), "missing key {key}: {record}");
        }
        assert_eq!(record["schema_version"], "1");
        assert!(record["command_prefix_bytes"].is_array());
        assert_eq!(record["command_prefix_bytes"], json!([]));
        assert_eq!(record["prefix_encoding"], PREFIX_ENCODING);
        // The old scope-less encoding string appears nowhere.
        assert!(!record.to_string().contains("raw byte array"), "{record}");
    }
    assert_eq!(unscoped, scoped);
}

#[test]
fn private_evidence_cannot_follow_symlinked_ancestors_or_public_directories() {
    for component in [
        ".commandagent",
        ".commandagent/evidence",
        ".commandagent/evidence/bash-placeholders",
    ] {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = root.path().join(component);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(outside.path(), &path).unwrap();
        let error = bash::run("echo <user> && touch executed", root.path(), true).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("private evidence recording failed")
        );
        assert!(!root.path().join("executed").exists());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join(".commandagent/evidence/bash-placeholders");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        bash::run("echo <user>", root.path(), true)
            .unwrap_err()
            .to_string()
            .contains("private evidence recording failed")
    );
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 0);
}

#[test]
fn saved_recovery_prompts_reports_and_plans_use_relative_workspace_paths() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("Users/alice/my workspace");
    std::fs::create_dir_all(&root).unwrap();
    let absolute = format!("{}/src/app/page.tsx", root.display());
    let redacted = absolute.replace("/Users/alice/", "/Users/<user>/");
    let failure = format!(
        "cat '{absolute}' failed; prior cat '{redacted}' failed; external /Users/bob/elsewhere/page.tsx"
    );
    let handoff = RecoveryHandoff {
        profile: "generic".to_string(),
        original_goal: "Repair the current application".to_string(),
        failure_kind: "bash_path_confinement_error".to_string(),
        failure_evidence: vec![failure.clone()],
        repair_targets: vec![absolute.clone()],
        ..Default::default()
    };
    let report = VerificationReport::missing_path(&absolute);
    let context = RepairContext {
        progress_warning: Some(failure),
        ..Default::default()
    };
    for path in [
        save_ultra_recovery_prompt(&root, "issue441", &handoff).unwrap(),
        save_recovery_ultra_plan(&root, "issue441", &handoff).unwrap(),
        save_repair_report_with_context(&root, "issue441", &report, &context).unwrap(),
    ] {
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("./src/app/page.tsx"),
            "{}: {text}",
            path.display()
        );
        assert!(!text.contains("<user>"), "{text}");
        assert!(!text.contains("alice") && !text.contains("bob"), "{text}");
        assert!(
            text.contains("external or unresolved path omitted"),
            "{text}"
        );
    }
}

#[test]
fn i2_historical_commands_get_placeholder_diagnostics_and_relative_retries_work() {
    let root = tempfile::tempdir().unwrap();
    let records = event_values(EVENTS);
    assert_eq!(records.len(), 18);
    assert_eq!(records.last().unwrap()["event"], "ultra_phase_failed");
    let commands: Vec<_> = records
        .iter()
        .filter_map(|record| record["command_summary"].as_str())
        .collect();
    assert_eq!(commands.len(), 3);
    for command in commands {
        assert!(command.contains("<user>"));
        let error = bash::run(command, root.path(), true).unwrap_err();
        assert!(error.to_string().contains("placeholder path detected"));
        let relative_command = command.split_once(" && ").unwrap().1;
        let relative_path = relative_command.strip_prefix("cat ").unwrap();
        let path = root.path().join(relative_path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "I2 corrected relative read\n").unwrap();
        assert!(
            bash::run(relative_command, root.path(), true)
                .unwrap()
                .contains("outcome: Success")
        );
    }
}

use commandagent::config::{
    Action, Config, ConfigFieldSources, NarrationMode, OllamaThink, OpenAiApi, PlanPreset,
    PromptLayout, Provider,
};
use commandagent::planner::runner::run_step_plan;
use commandagent::planner::step_plan::{PlanStep, StepPlan};
use commandagent::providers::{AssistantReply, ChatClient};
use commandagent::state::{ConversationMessage, ToolCall};
use commandagent::tools::registry::ToolSpec;

#[derive(Clone)]
struct CorrectingClient {
    turn: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    commands: Vec<String>,
}

impl ChatClient for CorrectingClient {
    fn label(&self) -> &str {
        "issue441-scripted-correction"
    }
    fn boxed_clone(&self) -> Box<dyn ChatClient> {
        Box::new(self.clone())
    }
    fn chat(
        &mut self,
        _model: &str,
        messages: &[ConversationMessage],
        _tools: &[ToolSpec],
        _native_tools_enabled: bool,
    ) -> anyhow::Result<AssistantReply> {
        let call = match self.turn.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
            0 => {
                return Ok(AssistantReply {
                    content: String::new(),
                    tool_calls: self
                        .commands
                        .iter()
                        .map(|command| ToolCall::new("Bash", json!({"command": command})))
                        .collect(),
                    prompt_tokens: None,
                    completion_tokens: None,
                });
            }
            1 => {
                let feedback = &messages
                    .iter()
                    .rfind(|message| message.role == "tool")
                    .unwrap()
                    .content;
                assert!(feedback.contains("placeholder path detected"), "{feedback}");
                assert!(feedback.contains("workspace-relative"), "{feedback}");
                assert!(
                    !feedback.contains("used an absolute path outside"),
                    "{feedback}"
                );
                ToolCall::new("Bash", json!({"command": "cat src/lib/data.ts"}))
            }
            2 => ToolCall::new(
                "Write",
                json!({"path": "artifact.txt", "content": "recovered"}),
            ),
            3 => return Ok(AssistantReply::text("done")),
            _ => anyhow::bail!("unexpected model call"),
        };
        Ok(AssistantReply {
            content: String::new(),
            tool_calls: vec![call],
            prompt_tokens: None,
            completion_tokens: None,
        })
    }
}

#[test]
fn runtime_implementation_continues_after_one_placeholder_diagnostic() {
    assert_runtime_correction(1);
}

#[test]
fn runtime_i2_batch_allows_correction_before_generic_repeat_stop() {
    assert_runtime_correction(3);
}

fn assert_runtime_correction(command_count: usize) {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/lib")).unwrap();
    std::fs::write(
        root.path().join("src/lib/data.ts"),
        "export const data = [];\n",
    )
    .unwrap();
    let commands: Vec<_> = event_values(EVENTS)
        .into_iter()
        .filter_map(|record| record["command_summary"].as_str().map(str::to_string))
        .take(command_count)
        .collect();
    let mut client = CorrectingClient {
        turn: Default::default(),
        commands: commands.clone(),
    };
    let events = root.path().join("events.jsonl");
    let plan = StepPlan {
        goal: "Create artifact.txt after reading the data source".to_string(),
        steps: vec![PlanStep {
            id: "implement-main-page".to_string(),
            kind: "implement".to_string(),
            expected_result: "pass".to_string(),
            instruction: "Read src/lib/data.ts, then write artifact.txt".to_string(),
            expected_paths: vec!["artifact.txt".to_string()],
            verify: vec!["test -f artifact.txt".to_string()],
        }],
    };
    run_step_plan(&mut client, &plan, &config(root.path(), &events)).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("artifact.txt")).unwrap(),
        "recovered"
    );
    let values = event_values(&std::fs::read_to_string(events).unwrap());
    let blocked: Vec<_> = values
        .iter()
        .filter(|event| event["event"] == "runtime_bash_policy" && event["blocked"] == true)
        .collect();
    assert_eq!(blocked.len(), command_count);
    assert_eq!(blocked[0]["placeholder_detected"], true);
    assert_eq!(
        blocked[0]["command_sha256"],
        format!("{:x}", Sha256::digest(&commands[0]))
    );
    assert_eq!(
        blocked[0]["verify_command_violation_kind"],
        "workspace_path_placeholder"
    );
    assert!(
        values
            .iter()
            .any(|event| event["event"] == "runtime_bash_policy" && event["blocked"] == false)
    );
    assert!(
        !values
            .iter()
            .any(|event| event["event"] == "plan_step_failed"
                || event["reason"] == "recoverable_tool_error_repeated")
    );
    let private = private_records(root.path());
    assert_eq!(private.len(), command_count);
    for record in &private {
        assert_eq!(record["command_prefix_bytes"], json!([]));
        assert_eq!(record["prefix_encoding"], PREFIX_ENCODING);
        assert_eq!(record["schema_version"], "1");
    }
    for event in blocked {
        assert!(
            private
                .iter()
                .any(|record| record["command_sha256"] == event["command_sha256"])
        );
    }
}

fn config(root: &Path, events: &Path) -> Config {
    Config {
        workspace_root: root.to_path_buf(),
        state_dir: root.join("state"),
        eval_events_path: Some(events.to_path_buf()),
        completion_contract_path: None,
        yes: true,
        offline: false,
        context_budget: 1000,
        model: "test".to_string(),
        provider: Provider::Ollama,
        tool_protocol: None,
        openai_api: OpenAiApi::ChatCompletions,
        prompt_layout: PromptLayout::Stable,
        plan_preset: PlanPreset::None,
        intent_override: None,
        planner_model: "test".to_string(),
        planner_provider: Provider::Ollama,
        planner_think: Some(OllamaThink::False),
        classifier_model: "test".to_string(),
        classifier_provider: Provider::Ollama,
        openai_compatible: None,
        ollama_host: "http://localhost:11434".to_string(),
        ollama_think: None,
        lm_studio_host: "http://localhost:1234".to_string(),
        num_predict: 100,
        max_iterations: 5,
        recovery_plan_auto_runs: 0,
        chat_timeout_secs: 1,
        chat_timeout_source: "override:test".to_string(),
        field_sources: ConfigFieldSources::default(),
        chat_retries: 0,
        stream: false,
        resume: None,
        fresh_session: false,
        no_footer: true,
        narration: NarrationMode::Normal,
        profile: "generic".to_string(),
        profile_explicit: true,
        profile_inference: None,
        style: "default".to_string(),
        action: Action::Repl,
    }
}
