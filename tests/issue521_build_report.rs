//! Pins how a `BuildVerifierStatus` is mapped to a `VerificationReport` by the
//! three public entries that do the mapping: the completion contract's required
//! commands, its deferred requirements, and the planner's step verification.
//!
//! Every expectation is a literal prefix plus a reason read back from the
//! lifecycle the same call returned, so the strings are compared whole while the
//! volatile parts of a command output (elapsed time) stay out of the fixture.

use commandagent::minimal_loop::build_verifier::{
    BuildVerifierLifecycleObservation, BuildVerifierStatus, CompileError,
};
use commandagent::minimal_loop::completion::{CompletionContract, DeferredVerifyRequirement};
use commandagent::minimal_loop::dependency_setup::NodeDependencySetupAuthority;
use commandagent::planner::step_plan::PlanStep;
use commandagent::planner::verify::{
    CommandFailure, VerificationReport, VerifyStatus,
    verify_setup_dependency_state_with_setup_observed_with_offline,
    verify_step_with_profile_setup_observed_with_offline,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Required,
    Deferred,
    Planner,
}

const MODES: [Mode; 3] = [Mode::Required, Mode::Deferred, Mode::Planner];
const PROFILE: &str = "python-cli";

struct Cell {
    name: &'static str,
    files: Vec<(&'static str, &'static str)>,
    command: &'static str,
    status: BuildVerifierStatus,
    compile_errors: bool,
    authority: NodeDependencySetupAuthority,
    offline: bool,
}

fn cell(
    name: &'static str,
    command: &'static str,
    status: BuildVerifierStatus,
    extra: Vec<(&'static str, &'static str)>,
) -> Cell {
    let mut files = vec![
        ("pyproject.toml", "[project]\nname = \"x\"\n"),
        ("src/x/main.py", "print(1)\n"),
    ];
    files.extend(extra);
    Cell {
        name,
        files,
        command,
        status,
        compile_errors: false,
        authority: NodeDependencySetupAuthority::None,
        offline: false,
    }
}

fn dependency_files() -> Vec<(&'static str, &'static str)> {
    vec![(
        "pyproject.toml",
        "[project]\nname = \"x\"\ndependencies = [\"requests\"]\n",
    )]
}

fn cells() -> Vec<Cell> {
    let mut dependency_missing = cell(
        "dependency_missing_setup_unauthorized",
        "python3 -m py_compile src/x/main.py",
        BuildVerifierStatus::DependencyMissing,
        Vec::new(),
    );
    dependency_missing.files = dependency_files();
    dependency_missing
        .files
        .push(("src/x/main.py", "print(1)\n"));
    let mut offline = cell(
        "dependency_missing_setup_blocked_offline",
        "python3 -m py_compile src/x/main.py",
        BuildVerifierStatus::DependencyMissing,
        Vec::new(),
    );
    offline.files = dependency_files();
    offline.files.push(("src/x/main.py", "print(1)\n"));
    offline.authority = NodeDependencySetupAuthority::CompletionContract;
    offline.offline = true;
    let mut compile = cell(
        "failed_with_compile_errors",
        "python3 -m py_compile bad.py",
        BuildVerifierStatus::Failed,
        vec![("bad.py", "def (:\n")],
    );
    compile.compile_errors = true;
    vec![
        cell(
            "passed",
            "python3 -m py_compile ok.py",
            BuildVerifierStatus::Passed,
            vec![("ok.py", "x = 1\n")],
        ),
        dependency_missing,
        offline,
        cell(
            "blocked",
            "python3 -m py_compile blocked.py",
            BuildVerifierStatus::Blocked,
            Vec::new(),
        ),
        cell(
            "failed_without_compile_errors",
            "python3 -m py_compile missing.py",
            BuildVerifierStatus::Failed,
            Vec::new(),
        ),
        compile,
        cell(
            "failed_invalid_semver",
            "python3 -m py_compile \"invalid version 8.0.C\"",
            BuildVerifierStatus::Failed,
            vec![(
                "package.json",
                "{\"dependencies\":{\"react-markdown\":\"8.0.C\"}}",
            )],
        ),
    ]
}

fn write_files(root: &std::path::Path, files: &[(&str, &str)]) {
    for (path, body) in files {
        let target = root.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, body).unwrap();
        #[cfg(unix)]
        if path.ends_with("main.py") {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}

fn contract() -> CompletionContract {
    CompletionContract {
        required_paths: Vec::new(),
        protected_paths: Vec::new(),
        verify_commands: Vec::new(),
        fix_reproducer_command: None,
        profile: None,
        goal: None,
        required_capabilities: Vec::new(),
        deterministic_oracles: Vec::new(),
        required_evidence: Vec::new(),
        evidence_hint_tokens: Vec::new(),
        required_obligations: Vec::new(),
        deferred_verify_requirements: Vec::new(),
        verify_repair_cap: 3,
    }
}

fn deferred(command: &str, status: &str) -> DeferredVerifyRequirement {
    DeferredVerifyRequirement {
        command: command.to_string(),
        reason: "table".to_string(),
        authority: "completion_contract".to_string(),
        profile: Some(PROFILE.to_string()),
        status: status.to_string(),
    }
}

fn plan_step(commands: &[&str]) -> PlanStep {
    PlanStep {
        id: "verify".to_string(),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "verify".to_string(),
        expected_paths: Vec::new(),
        verify: commands.iter().map(|c| c.to_string()).collect(),
    }
}

fn run(
    mode: Mode,
    root: &std::path::Path,
    command: &str,
    authority: NodeDependencySetupAuthority,
    offline: bool,
    deferred_status: &str,
) -> (VerificationReport, Vec<BuildVerifierLifecycleObservation>) {
    match mode {
        Mode::Required => {
            let mut contract = contract();
            contract.profile = Some(PROFILE.to_string());
            contract.verify_commands = vec![command.to_string()];
            contract.verify_with_goal_observed_with_setup_authority(root, "", authority, offline)
        }
        Mode::Deferred => {
            let mut contract = contract();
            contract.deferred_verify_requirements = vec![deferred(command, deferred_status)];
            contract.verify_with_goal_observed_with_setup_authority(root, "", authority, offline)
        }
        Mode::Planner => verify_step_with_profile_setup_observed_with_offline(
            root,
            &plan_step(&[command]),
            Some(PROFILE),
            authority,
            offline,
        ),
    }
}

/// What the report must hold for one failing lifecycle.
#[derive(Debug, Default, PartialEq, Eq)]
struct Expected {
    dependency_missing: Vec<String>,
    command_failures: Vec<CommandFailure>,
    profile_failures: Vec<String>,
    compile_errors: Vec<CompileError>,
}

fn failure(command: &str, reason: String) -> CommandFailure {
    CommandFailure {
        command: command.to_string(),
        reason,
    }
}

fn expected_for(
    mode: Mode,
    cell: &Cell,
    lifecycle: &BuildVerifierLifecycleObservation,
) -> Expected {
    let command = cell.command;
    let reason = lifecycle.final_reason.clone();
    let mut expected = Expected::default();
    match cell.status {
        BuildVerifierStatus::Passed => {}
        BuildVerifierStatus::DependencyMissing => {
            let reason = if mode == Mode::Planner && cell.offline {
                "dependency_setup_blocked_offline".to_string()
            } else {
                reason
            };
            expected
                .dependency_missing
                .push(format!("dependency_setup_missing: {reason}"));
        }
        BuildVerifierStatus::PolicyRejected => expected.command_failures.push(failure(
            command,
            format!("build_verify_policy_rejected: {reason}"),
        )),
        BuildVerifierStatus::Blocked => expected.profile_failures.push(format!(
            "build_verify_blocked: command `{command}` reason `{reason}`"
        )),
        BuildVerifierStatus::Failed if cell.compile_errors => {
            expected.compile_errors = lifecycle.final_observation().compile_errors.clone();
            expected.command_failures.push(failure(
                command,
                "implementation_compile_error: bad.py:1 SyntaxError: invalid syntax".to_string(),
            ));
        }
        BuildVerifierStatus::Failed => {
            let text = match mode {
                Mode::Required | Mode::Deferred => format!("build_verify_failed: {reason}"),
                Mode::Planner if cell.name == "failed_invalid_semver" => format!(
                    "dependency_setup_lifecycle_failed: invalid_semver_manifest_entry: \"react-markdown\": \"8.0.C\" is not valid semver - use e.g. \"8.0.0\"; {reason}"
                ),
                Mode::Planner => format!("dependency_setup_lifecycle_failed: {reason}"),
            };
            expected.command_failures.push(failure(command, text));
        }
    }
    expected
}

fn assert_cell(mode: Mode, cell: &Cell) {
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &cell.files);
    let (report, lifecycles) = run(
        mode,
        dir.path(),
        cell.command,
        cell.authority,
        cell.offline,
        "required",
    );
    let label = format!("{mode:?}/{}", cell.name);
    assert_eq!(lifecycles.len(), 1, "{label}: one lifecycle per command");
    let lifecycle = &lifecycles[0];
    assert_eq!(
        lifecycle.final_status, cell.status,
        "{label}: fixture status"
    );
    assert!(
        lifecycle.requirement.required_for_completion,
        "{label}: required_for_completion"
    );
    let expected = expected_for(mode, cell, lifecycle);
    assert_eq!(
        report.compile_errors, expected.compile_errors,
        "{label}: compile errors"
    );
    assert_eq!(
        report.command_failures, expected.command_failures,
        "{label}: command failures"
    );
    // A contract with a profile also asks the profile for its own failures, so
    // the lists the profile can repeat are compared by membership.
    for reason in &expected.dependency_missing {
        assert!(
            report.dependency_missing.contains(reason),
            "{label}: dependency_missing {reason:?} not in {:?}",
            report.dependency_missing
        );
    }
    for reason in &expected.profile_failures {
        assert!(
            report.profile_failures.contains(reason),
            "{label}: profile_failures {reason:?} not in {:?}",
            report.profile_failures
        );
    }
    if expected.dependency_missing.is_empty() {
        assert!(report.dependency_missing.is_empty(), "{label}");
    }
    let expected_status = if let Some(reason) = report.dependency_missing.first() {
        VerifyStatus::DependencyMissing(reason.clone())
    } else if let Some(failure) = expected.command_failures.first() {
        VerifyStatus::CommandFailed(failure.reason.clone())
    } else if let Some(reason) = report.profile_failures.first() {
        VerifyStatus::ProfileContractFailed(reason.clone())
    } else {
        VerifyStatus::Pass
    };
    assert_eq!(report.status, expected_status, "{label}: stop reason");
    assert_eq!(
        report.is_pass(),
        cell.status == BuildVerifierStatus::Passed,
        "{label}: pass/fail"
    );
}

#[test]
fn status_mode_compile_error_table_matches_baseline() {
    for mode in MODES {
        for cell in cells() {
            assert_cell(mode, &cell);
        }
    }
}

#[test]
fn literal_reasons_are_pinned() {
    let dir = tempfile::tempdir().unwrap();
    let missing = cell(
        "m",
        "python3 -m py_compile src/x/main.py",
        BuildVerifierStatus::DependencyMissing,
        Vec::new(),
    );
    write_files(dir.path(), &dependency_files());
    write_files(dir.path(), &[("src/x/main.py", "print(1)\n")]);
    for mode in MODES {
        let (report, _) = run(
            mode,
            dir.path(),
            missing.command,
            NodeDependencySetupAuthority::None,
            false,
            "required",
        );
        assert!(
            report.dependency_missing.contains(
                &"dependency_setup_missing: Python CLI dependency setup missing: .venv/bin/python"
                    .to_string()
            ),
            "{mode:?}: {:?}",
            report.dependency_missing
        );
    }
    let bad = tempfile::tempdir().unwrap();
    write_files(bad.path(), &[("bad.py", "def (:\n")]);
    for mode in MODES {
        let (report, _) = run(
            mode,
            bad.path(),
            "python3 -m py_compile bad.py",
            NodeDependencySetupAuthority::None,
            false,
            "required",
        );
        assert_eq!(
            report.command_failures,
            vec![failure(
                "python3 -m py_compile bad.py",
                "implementation_compile_error: bad.py:1 SyntaxError: invalid syntax".to_string()
            )],
            "{mode:?}"
        );
        assert_eq!(report.compile_errors.len(), 1, "{mode:?}");
    }
}

#[test]
fn deferred_policy_rejected_uses_policy_prefix() {
    let dir = tempfile::tempdir().unwrap();
    write_files(dir.path(), &[("src/x/main.py", "print(1)\n")]);
    let command = "python3 -m py_compile a.py; rm -rf x";
    let (report, lifecycles) = run(
        Mode::Deferred,
        dir.path(),
        command,
        NodeDependencySetupAuthority::None,
        false,
        "required",
    );
    assert_eq!(lifecycles.len(), 1);
    assert_eq!(
        lifecycles[0].final_status,
        BuildVerifierStatus::PolicyRejected
    );
    assert_eq!(
        report.command_failures,
        vec![failure(
            command,
            format!(
                "build_verify_policy_rejected: {}",
                lifecycles[0].final_reason
            )
        )]
    );
    assert!(report.dependency_missing.is_empty());
    assert!(report.profile_failures.is_empty());
    assert_eq!(
        report.status,
        VerifyStatus::CommandFailed(report.command_failures[0].reason.clone())
    );
}

#[test]
fn deferred_optional_requirement_is_observed_but_not_reported() {
    for cell in cells() {
        if cell.status == BuildVerifierStatus::Passed {
            continue;
        }
        let dir = tempfile::tempdir().unwrap();
        write_files(dir.path(), &cell.files);
        let (report, lifecycles) = run(
            Mode::Deferred,
            dir.path(),
            cell.command,
            cell.authority,
            cell.offline,
            "optional",
        );
        assert_eq!(lifecycles.len(), 1, "{}", cell.name);
        assert_eq!(lifecycles[0].final_status, cell.status, "{}", cell.name);
        assert!(!lifecycles[0].requirement.required_for_completion);
        assert_eq!(report, VerificationReport::pass(), "{}", cell.name);
    }
}

#[test]
fn lifecycles_and_failures_keep_command_order() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[
            ("pyproject.toml", "[project]\nname = \"x\"\n"),
            ("src/x/main.py", "print(1)\n"),
            ("ok.py", "x = 1\n"),
            ("bad.py", "def (:\n"),
        ],
    );
    let mut contract = contract();
    contract.profile = Some(PROFILE.to_string());
    contract.verify_commands = vec![
        "python3 -m py_compile ok.py".to_string(),
        "python3 -m py_compile bad.py".to_string(),
        "python3 -m py_compile missing.py".to_string(),
    ];
    contract.deferred_verify_requirements = vec![
        deferred("python3 -m py_compile blocked.py", "required"),
        deferred("python3 -m py_compile ok.py", "required"),
    ];
    let (report, lifecycles) = contract.verify_with_goal_observed_with_setup_authority(
        dir.path(),
        "",
        NodeDependencySetupAuthority::None,
        false,
    );
    let statuses = lifecycles
        .iter()
        .map(|l| l.final_status)
        .collect::<Vec<_>>();
    assert_eq!(
        statuses,
        vec![
            BuildVerifierStatus::Passed,
            BuildVerifierStatus::Failed,
            BuildVerifierStatus::Failed,
            BuildVerifierStatus::Blocked,
            BuildVerifierStatus::Passed,
        ]
    );
    let commands = report
        .command_failures
        .iter()
        .map(|f| f.command.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        vec![
            "python3 -m py_compile bad.py",
            "python3 -m py_compile missing.py"
        ]
    );
    assert_eq!(
        report.command_failures[1].reason,
        format!("build_verify_failed: {}", lifecycles[2].final_reason)
    );
    assert_eq!(
        report.profile_failures,
        vec![format!(
            "build_verify_blocked: command `python3 -m py_compile blocked.py` reason `{}`",
            lifecycles[3].final_reason
        )]
    );
}

#[test]
fn node_zero_tests_are_rejected_in_every_mode() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[("package.json", "{\"scripts\":{\"check\":\"echo 0 tests\"}}")],
    );
    for mode in [Mode::Required, Mode::Planner] {
        let (report, lifecycles) = run(
            mode,
            dir.path(),
            "npm run check",
            NodeDependencySetupAuthority::None,
            false,
            "required",
        );
        assert!(lifecycles.is_empty(), "{mode:?}");
        assert_eq!(
            report.command_failures,
            vec![failure(
                "npm run check",
                "Node 0 tests rejected".to_string()
            )],
            "{mode:?}"
        );
    }
}

#[test]
fn setup_step_without_node_modules_reports_dependency_missing_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    write_files(
        dir.path(),
        &[(
            "package.json",
            "{\"dependencies\":{\"left-pad\":\"^1.3.0\"}}",
        )],
    );
    let (report, lifecycles) = verify_setup_dependency_state_with_setup_observed_with_offline(
        dir.path(),
        NodeDependencySetupAuthority::None,
        false,
    );
    assert_eq!(lifecycles.len(), 1);
    assert_eq!(
        lifecycles[0].final_status,
        BuildVerifierStatus::DependencyMissing
    );
    assert_eq!(
        report.dependency_missing,
        vec![format!(
            "dependency_setup_missing: {}",
            lifecycles[0].final_reason
        )]
    );
    assert!(report.command_failures.is_empty());
}
