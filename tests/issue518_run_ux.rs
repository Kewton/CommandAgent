//! Issue #518: the run-adjacent UX inconsistencies.
//!
//! - a clean Git workspace must not warn about uncommitted changes that
//!   CommandAgent itself created (the `.commandagent/lock` taken before the
//!   workspace inspection);
//! - `/resume <run-id>` for an interrupted run without a recovery UltraPlan
//!   must name the missing artifact and a step that can actually proceed;
//! - the REPL `/ultra-plan` without a goal must be rejected before the planner
//!   provider is called.

use std::path::Path;
use std::process::{Command, Output};

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?}");
}

fn commit_all(root: &Path) {
    git(root, &["add", "."]);
    git(
        root,
        &[
            "-c",
            "user.name=CommandAgent Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-q",
            "-m",
            "initial",
        ],
    );
}

/// Run the binary through a path that acquires the workspace lock and starts the
/// Git reporter, then fails at the provider (no key). The failure is irrelevant;
/// what matters is the workspace inspection the run performs around it.
fn run_offline_prompt(workspace: &Path, state: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commandagent"))
        .args([
            "--offline",
            "--provider",
            "openai",
            "--model",
            "test-model",
            "--prompt",
            "inspect the workspace",
            "--cwd",
            workspace.to_str().unwrap(),
            "--state-dir",
            state.to_str().unwrap(),
            "--no-footer",
        ])
        .env_remove("OPENAI_API_KEY")
        .output()
        .unwrap()
}

fn prepare_workspace(fixture: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let workspace = fixture.join("workspace");
    let state = fixture.join("state");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    git(&workspace, &["init", "-q"]);
    (workspace, state)
}

#[test]
fn clean_workspace_does_not_warn_about_its_own_runtime_state() {
    let fixture = tempfile::tempdir().unwrap();
    let (workspace, state) = prepare_workspace(fixture.path());
    std::fs::write(workspace.join("tracked.txt"), "committed\n").unwrap();
    commit_all(&workspace);

    let output = run_offline_prompt(&workspace, &state);
    let stderr = String::from_utf8(output.stderr).unwrap();

    // The reproduction is only meaningful if the run actually took the lock
    // before inspecting the workspace.
    assert!(
        workspace.join(".commandagent/lock").is_file(),
        "the run must have created its workspace lock: {stderr}"
    );
    assert!(
        !stderr.contains(commandagent::tools::git_state::DIRTY_WARNING),
        "a clean workspace must not warn about the tool's own lock: {stderr}"
    );
    assert!(
        !stderr.contains(commandagent::tools::git_state::EXIT_REPORT_HEADING),
        "a clean workspace must not report runtime state as changes at exit: {stderr}"
    );
}

#[test]
fn genuine_uncommitted_changes_still_warn_before_the_run() {
    let fixture = tempfile::tempdir().unwrap();
    let (workspace, state) = prepare_workspace(fixture.path());
    std::fs::write(workspace.join("tracked.txt"), "committed\n").unwrap();
    commit_all(&workspace);
    std::fs::write(workspace.join("user-note.txt"), "mine\n").unwrap();

    let output = run_offline_prompt(&workspace, &state);
    let stderr = String::from_utf8(output.stderr).unwrap();

    assert!(
        stderr.contains(commandagent::tools::git_state::DIRTY_WARNING),
        "a real user change must still warn: {stderr}"
    );
    assert!(
        stderr.contains(commandagent::tools::git_state::EXIT_REPORT_HEADING),
        "{stderr}"
    );
    assert!(stderr.contains("user-note.txt"), "{stderr}");
}

/// Write an interrupted run, optionally recording the action that started it.
///
/// The action string mirrors the `Debug` form the product stores in
/// `run_start.action` (for example `RunPlan("saved-plan.yaml")`).
fn interrupted_run(root: &Path, id: &str, action: Option<&str>) {
    let dir = root.join(".commandagent/runs").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let mut events = String::new();
    if let Some(action) = action {
        let encoded = serde_json::to_string(action).unwrap();
        events.push_str(&format!(
            "{{\"event\":\"run_start\",\"action\":{encoded}}}\n"
        ));
    }
    events.push_str(concat!(
        "{\"event\":\"planner_quality_retry_exhausted\",\"stop_class\":\"planner_quality_exhausted\"}\n",
        "{\"event\":\"tui_command_stop\",\"ok\":false,\"status\":\"interrupted\",",
        "\"failure_kind\":\"direct_cli_command_interrupted\",",
        "\"stop_reason\":\"interrupted by user\",",
        "\"next_action\":\"resume_or_rerun_command\"}\n"
    ));
    std::fs::write(dir.join("events.jsonl"), events).unwrap();
}

fn resume_error(root: &Path, id: &str) -> String {
    commandagent::runs::prepare_resume(root, id)
        .unwrap_err()
        .to_string()
}

#[test]
fn resume_without_a_recovery_ultra_plan_names_the_missing_artifact_generically() {
    let root = tempfile::tempdir().unwrap();
    interrupted_run(root.path(), "issue518-interrupted", None);

    let error = resume_error(root.path(), "issue518-interrupted");

    assert!(error.contains("recovery UltraPlan"), "{error}");
    assert!(
        !error.contains("path is empty"),
        "the error must not leak the raw resolution failure: {error}"
    );
    assert!(
        error.contains("--run-plan <path>") && error.contains("re-run"),
        "without an identifiable plan the guidance must stay general: {error}"
    );
}

#[test]
fn resume_does_not_guess_a_plan_from_the_newest_saved_file() {
    let root = tempfile::tempdir().unwrap();
    interrupted_run(root.path(), "issue518-interrupted", None);
    let plans = root.path().join(".commandagent/plans");
    std::fs::create_dir_all(&plans).unwrap();
    let newest = plans.join("plan-00000000-0000-0000-0000-000000000000.yaml");
    std::fs::write(&newest, "goal: x\n").unwrap();

    let error = resume_error(root.path(), "issue518-interrupted");

    assert!(
        !error.contains("plan-00000000-0000-0000-0000-000000000000.yaml"),
        "a plan that cannot be pinned to the run must not be named: {error}"
    );
    assert!(
        !error.contains("plan-"),
        "no saved-file scan may leak into the guidance: {error}"
    );
    assert!(error.contains("--run-plan <path>"), "{error}");
}

#[test]
fn resume_names_the_plan_the_run_recorded_for_itself() {
    let root = tempfile::tempdir().unwrap();
    interrupted_run(
        root.path(),
        "issue518-interrupted",
        Some(r#"RunPlan("saved-plan.yaml")"#),
    );
    std::fs::write(root.path().join("saved-plan.yaml"), "goal: x\n").unwrap();

    let error = resume_error(root.path(), "issue518-interrupted");

    assert!(error.contains("recovery UltraPlan"), "{error}");
    assert!(
        error.contains("`--run-plan saved-plan.yaml`"),
        "the run's own recorded plan path must be named: {error}"
    );
}
