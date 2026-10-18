//! Boundary tables for the completion gate decisions (Issue #508).
//!
//! Each table row pairs an input with the expected gate outcome; rows that
//! straddle a boundary come in pairs so that a flipped condition changes at
//! least one row. `kills:` comments name the `cargo mutants` mutations (file
//! line in `completion.rs` at develop `e78bd81a`) a row or test is meant to
//! catch.

use super::*;
use crate::planner::profile_descriptor::NEXTJS_PROFILE_ID as NEXT;
use crate::planner::verify::VerifyStatus;

fn base() -> CompletionContract {
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
        verify_repair_cap: 2,
    }
}

fn s(value: &str) -> String {
    value.to_string()
}

fn deferred(command: &str, status: &str) -> DeferredVerifyRequirement {
    DeferredVerifyRequirement {
        command: s(command),
        reason: s("issue508"),
        authority: s("postcheck"),
        profile: Some(s(NEXT)),
        status: s(status),
    }
}

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

/// Mark the Next.js build dependencies as present.
fn install_next_dependencies(root: &Path) {
    std::fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
    std::fs::create_dir_all(root.join("node_modules/next")).unwrap();
    std::fs::write(
        root.join("node_modules/next/package.json"),
        r#"{"version":"14.2.0"}"#,
    )
    .unwrap();
    write_executable(&root.join("node_modules/.bin/next"), "#!/bin/sh\nexit 0\n");
}

/// A workspace whose `npm run build` exits with `build_exit` through a
/// workspace-local fake `npm`.
fn next_workspace_with_build_exit(build_exit: i32) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#,
    )
    .unwrap();
    install_next_dependencies(dir.path());
    write_executable(
        &dir.path().join("node_modules/.bin/npm"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"run\" ] && [ \"$2\" = \"build\" ]; then exit {build_exit}; fi\nexit 1\n"
        ),
    );
    dir
}

fn has_failure_prefix(report: &VerificationReport, prefix: &str) -> bool {
    report
        .profile_failures
        .iter()
        .any(|failure| failure.starts_with(prefix))
}

// kills: completion.rs:195 replace has_verify with true/false, 195 delete !,
// 196-201 replace || with &&, 197-201 delete !
#[test]
fn has_verify_is_true_only_when_a_gate_source_is_declared() {
    let rows: Vec<(&str, CompletionContract, bool)> = vec![
        ("empty contract has no gate", base(), false),
        (
            "required_paths alone is not a verify gate",
            CompletionContract {
                required_paths: vec![s("a.txt")],
                ..base()
            },
            false,
        ),
        (
            "generic profile alone is not a verify gate",
            CompletionContract {
                profile: Some(s("generic")),
                ..base()
            },
            false,
        ),
        (
            "verify_commands",
            CompletionContract {
                verify_commands: vec![s("true")],
                ..base()
            },
            true,
        ),
        (
            "non-generic profile",
            CompletionContract {
                profile: Some(s(NEXT)),
                ..base()
            },
            true,
        ),
        (
            "required_capabilities",
            CompletionContract {
                required_capabilities: vec![s("stateful_interaction")],
                ..base()
            },
            true,
        ),
        (
            "required_evidence",
            CompletionContract {
                required_evidence: vec![s("test_artifact")],
                ..base()
            },
            true,
        ),
        (
            "required_obligations",
            CompletionContract {
                required_obligations: vec![s("implementation")],
                ..base()
            },
            true,
        ),
        (
            "deterministic_oracles",
            CompletionContract {
                deterministic_oracles: vec![s("oracle")],
                ..base()
            },
            true,
        ),
        (
            "deferred_verify_requirements",
            CompletionContract {
                deferred_verify_requirements: vec![deferred("echo ok", "pending")],
                ..base()
            },
            true,
        ),
    ];
    for (name, contract, expected) in rows {
        assert_eq!(contract.has_verify(), expected, "{name}");
    }
}

// kills: completion.rs:1992 replace profile_is_generic with true/false
#[test]
fn profile_is_generic_matches_only_the_generic_aliases() {
    let rows = [
        ("", true),
        ("generic", true),
        ("default", true),
        ("none", true),
        (NEXT, false),
        ("python-cli", false),
        ("Generic", false),
        (" generic", false),
        ("generic2", false),
    ];
    for (profile, expected) in rows {
        assert_eq!(profile_is_generic(profile), expected, "{profile:?}");
    }
}

// kills: completion.rs:603 active_profile -> None/Some("")/Some("xyzzy"),
// 605 delete !, 609 profile_requires_completion_gate -> true/false
#[test]
fn active_profile_skips_generic_aliases_and_drives_the_completion_gate() {
    let rows: Vec<(Option<&str>, Option<&str>)> = vec![
        (None, None),
        (Some(""), None),
        (Some("generic"), None),
        (Some("default"), None),
        (Some("none"), None),
        (Some(NEXT), Some(NEXT)),
        (Some("python-cli"), Some("python-cli")),
    ];
    for (profile, expected) in rows {
        let contract = CompletionContract {
            profile: profile.map(s),
            ..base()
        };
        assert_eq!(contract.active_profile(), expected, "{profile:?}");
        assert_eq!(
            contract.profile_requires_completion_gate(),
            expected.is_some(),
            "{profile:?}"
        );
    }
}

// kills: completion.rs:618 deferred_requirement_covered -> true/false,
// 618:31 replace == with !=
#[test]
fn deferred_requirement_is_covered_only_by_the_static_profile_status() {
    let dir = tempfile::tempdir().unwrap();
    let contract = base();
    let rows = [
        ("covered_by_static_profile_check", true),
        ("pending", false),
        ("blocked_by_dependency_setup", false),
        ("optional", false),
        ("", false),
    ];
    for (status, expected) in rows {
        for profile_passed in [false, true] {
            let requirement = deferred("echo ok", status);
            assert_eq!(
                contract.deferred_requirement_covered(dir.path(), &requirement, profile_passed),
                expected,
                "{status:?} profile_passed={profile_passed}"
            );
        }
    }
}

// kills: completion.rs:205 has_registered_fix_reproducer -> true/false
#[test]
fn fix_reproducer_is_registered_only_when_it_is_a_verify_command() {
    let rows: Vec<(&str, CompletionContract, bool)> = vec![
        ("no reproducer", base(), false),
        (
            "reproducer without verify commands",
            CompletionContract {
                fix_reproducer_command: Some(s("python3 cli.py 7")),
                ..base()
            },
            false,
        ),
        (
            "reproducer missing from verify commands",
            CompletionContract {
                fix_reproducer_command: Some(s("python3 cli.py 7")),
                verify_commands: vec![s("python3 cli.py 8")],
                ..base()
            },
            false,
        ),
        (
            "verify commands without reproducer",
            CompletionContract {
                verify_commands: vec![s("python3 cli.py 7")],
                ..base()
            },
            false,
        ),
        (
            "reproducer registered among verify commands",
            CompletionContract {
                fix_reproducer_command: Some(s("python3 cli.py 7")),
                verify_commands: vec![s("true"), s("python3 cli.py 7")],
                ..base()
            },
            true,
        ),
    ];
    for (name, contract, expected) in rows {
        assert_eq!(contract.has_registered_fix_reproducer(), expected, "{name}");
    }
}

// kills: completion.rs:211 dependency_precondition_active -> true/false,
// 213:17 replace && with ||, 213:20 delete !
#[test]
fn dependency_precondition_needs_a_next_build_command_and_missing_dependencies() {
    let cases: Vec<(&str, Vec<&str>, bool, bool)> = vec![
        // (name, verify commands, next dependencies installed, expected)
        (
            "next build, deps missing",
            vec!["npm run build"],
            false,
            true,
        ),
        ("next build, deps ready", vec!["npm run build"], true, false),
        ("non-next command, deps missing", vec!["true"], false, false),
        ("non-next command, deps ready", vec!["true"], true, false),
        ("no commands, deps missing", vec![], false, false),
        (
            "next build among others, deps missing",
            vec!["true", "npm run build"],
            false,
            true,
        ),
    ];
    for (name, commands, deps_ready, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        if deps_ready {
            install_next_dependencies(dir.path());
        }
        let contract = CompletionContract {
            verify_commands: commands.into_iter().map(s).collect(),
            ..base()
        };
        assert_eq!(
            contract.dependency_precondition_active(dir.path()),
            expected,
            "{name}"
        );
    }
}

// kills: completion.rs:663 setup_authority_for_deferred -> Default::default(),
// 664:35, 666:48, 668:41 replace && with ||
#[test]
fn deferred_setup_authority_needs_both_the_actor_and_setup_words() {
    let fallback = NodeDependencySetupAuthority::PlanSetupStep;
    let rows = [
        ("eval setup", NodeDependencySetupAuthority::EvalExplicit),
        ("EVAL SETUP", NodeDependencySetupAuthority::EvalExplicit),
        (
            "completion setup",
            NodeDependencySetupAuthority::CompletionContract,
        ),
        (
            "Completion-Contract Setup",
            NodeDependencySetupAuthority::CompletionContract,
        ),
        ("tui setup", NodeDependencySetupAuthority::TuiConfirmed),
        (
            "eval completion tui setup",
            NodeDependencySetupAuthority::EvalExplicit,
        ),
        (
            "completion tui setup",
            NodeDependencySetupAuthority::CompletionContract,
        ),
        // Only one of the two words: falls back.
        ("eval", fallback),
        ("completion", fallback),
        ("tui", fallback),
        ("setup", fallback),
        ("eval completion tui", fallback),
        ("postcheck", fallback),
        ("", fallback),
    ];
    for (authority, expected) in rows {
        let requirement = DeferredVerifyRequirement {
            authority: s(authority),
            ..deferred("npm run build", "pending")
        };
        assert_eq!(
            setup_authority_for_deferred(&requirement, fallback),
            expected,
            "{authority:?}"
        );
    }
}

// kills: completion.rs:1948 normalize_contract_file_path -> Ok(Default::default()),
// 1951:34 replace == with !=, 1973:16 delete !
// (1973:13 replace && with || does not compile inside the let chain, and
// pushing a duplicate allowed root would be equivalent anyway.)
#[test]
fn contract_file_path_must_live_under_the_workspace_or_a_temp_dir() {
    let root = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("contract.json"), "{}").unwrap();
    std::fs::write(root.path().join("x.env"), "{}").unwrap();
    std::fs::write(root.path().join(".env"), "SECRET=1").unwrap();
    std::fs::create_dir_all(root.path().join("sub")).unwrap();
    std::fs::write(root.path().join("sub/.env"), "SECRET=1").unwrap();
    std::fs::write(elsewhere.path().join("contract.json"), "{}").unwrap();

    let canonical_root = root.path().canonicalize().unwrap();
    let canonical_elsewhere = elsewhere.path().canonicalize().unwrap();

    // Accepted: relative to the workspace, absolute inside it, and a file in
    // another temp directory.
    let accepted = [
        (
            Path::new("contract.json").to_path_buf(),
            canonical_root.join("contract.json"),
        ),
        (
            root.path().join("contract.json"),
            canonical_root.join("contract.json"),
        ),
        (
            Path::new("x.env").to_path_buf(),
            canonical_root.join("x.env"),
        ),
        (
            elsewhere.path().join("contract.json"),
            canonical_elsewhere.join("contract.json"),
        ),
    ];
    for (raw, expected) in accepted {
        let got = normalize_contract_file_path(root.path(), &raw)
            .unwrap_or_else(|err| panic!("{raw:?} rejected: {err}"));
        assert_eq!(got, expected, "{raw:?}");
    }

    // Rejected: .env by file name, outside every allowed root, and missing.
    let rejected = [
        (Path::new(".env").to_path_buf(), "may not be .env"),
        (Path::new("sub/.env").to_path_buf(), "may not be .env"),
        (
            Path::new("/").to_path_buf(),
            "must be under workspace or temp directory",
        ),
        (
            Path::new("missing.json").to_path_buf(),
            "failed to canonicalize",
        ),
    ];
    for (raw, message) in rejected {
        let err = normalize_contract_file_path(root.path(), &raw)
            .expect_err(&format!("{raw:?} should be rejected"));
        assert!(err.to_string().contains(message), "{raw:?}: {err}");
    }
}

/// Which bucket of a verification report holds the failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Pass,
    CommandFailure,
    DependencyMissing,
}

fn bucket(report: &VerificationReport) -> Bucket {
    if report.is_pass() {
        Bucket::Pass
    } else if !report.dependency_missing.is_empty() {
        assert!(report.command_failures.is_empty(), "{report:?}");
        Bucket::DependencyMissing
    } else {
        assert!(!report.command_failures.is_empty(), "{report:?}");
        Bucket::CommandFailure
    }
}

// kills: completion.rs:280 verify_with_goal_observed_inner -> (Default, vec![])
// / (Default, vec![Default]) (no build verifier ran: report passes, no
// observations), 354:48 replace && with ||, 362:29 match guard -> true/false
#[test]
fn verify_commands_are_classified_by_their_outcome() {
    let rows = [
        ("true", Bucket::Pass, None),
        // `npm` in the command alone does not reject the output.
        ("echo npm", Bucket::Pass, None),
        // "0 tests" in the output alone does not reject a non-npm command.
        ("echo 0 tests", Bucket::Pass, None),
        (
            "echo npm 0 tests",
            Bucket::CommandFailure,
            Some("Node 0 tests rejected"),
        ),
        (
            "false",
            Bucket::CommandFailure,
            Some("command failed: false"),
        ),
        (
            "nonexistent-bin-issue508 --flag",
            Bucket::DependencyMissing,
            Some("nonexistent-bin-issue508"),
        ),
    ];
    let dir = tempfile::tempdir().unwrap();
    for (command, expected, detail) in rows {
        let contract = CompletionContract {
            verify_commands: vec![s(command)],
            ..base()
        };
        let (report, observations) = contract.verify_with_goal_observed(dir.path(), "");
        assert_eq!(bucket(&report), expected, "{command}: {report:?}");
        assert!(observations.is_empty(), "{command}");
        if let Some(detail) = detail {
            let text = format!("{report:?}");
            assert!(text.contains(detail), "{command}: {text}");
        }
    }
}

// kills: completion.rs:280 (Default, vec![]) / (Default, vec![Default]),
// 304:39 replace != with ==
#[test]
fn build_verify_command_outcome_decides_the_report_and_records_a_lifecycle() {
    let rows = [(0, true), (1, false)];
    for (build_exit, passes) in rows {
        let dir = next_workspace_with_build_exit(build_exit);
        let contract = CompletionContract {
            verify_commands: vec![s("npm run build")],
            ..base()
        };
        let (report, observations) = contract.verify_with_goal_observed(dir.path(), "");
        assert_eq!(report.is_pass(), passes, "exit {build_exit}: {report:?}");
        assert_eq!(observations.len(), 1, "exit {build_exit}");
        let expected_status = if passes {
            BuildVerifierStatus::Passed
        } else {
            BuildVerifierStatus::Failed
        };
        assert_eq!(observations[0].final_status, expected_status);
        if !passes {
            assert!(
                report.primary_reason().contains("build_verify_failed"),
                "{report:?}"
            );
        }
    }
    // Dependencies missing: blocked rather than passing.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts":{"build":"next build"},"dependencies":{"next":"^14.2.0","react":"^18.3.0","react-dom":"^18.3.0"}}"#,
    )
    .unwrap();
    let contract = CompletionContract {
        verify_commands: vec![s("npm run build")],
        ..base()
    };
    let (report, observations) = contract.verify_with_goal_observed(dir.path(), "");
    assert!(!report.is_pass(), "{report:?}");
    assert!(matches!(report.status, VerifyStatus::DependencyMissing(_)));
    assert_eq!(observations.len(), 1);
}

// kills: completion.rs:425:21 replace && with ||, 425:43 replace != with ==
#[test]
fn deferred_build_requirement_blocks_only_when_required_and_not_passing() {
    // (status, build exit code, blocks completion)
    let rows = [
        ("pending", 0, false),
        ("pending", 1, true),
        ("blocked_by_dependency_setup", 0, false),
        ("blocked_by_dependency_setup", 1, true),
        ("optional", 0, false),
        ("optional", 1, false),
    ];
    for (status, build_exit, blocks) in rows {
        let dir = next_workspace_with_build_exit(build_exit);
        let contract = CompletionContract {
            deferred_verify_requirements: vec![deferred("npm run build", status)],
            ..base()
        };
        let (report, observations) = contract.verify_with_goal_observed(dir.path(), "");
        assert_eq!(
            !report.is_pass(),
            blocks,
            "{status} exit {build_exit}: {report:?}"
        );
        assert_eq!(observations.len(), 1, "{status} exit {build_exit}");
        if blocks {
            assert!(
                report.primary_reason().contains("build_verify_failed"),
                "{status}: {report:?}"
            );
        }
    }
}

// kills: completion.rs:618 deferred_requirement_covered -> true/false and
// 618:31 replace == with != (through the verify path)
#[test]
fn deferred_non_build_requirement_passes_only_when_covered_by_the_static_profile() {
    let dir = tempfile::tempdir().unwrap();
    let rows = [
        ("covered_by_static_profile_check", true),
        ("pending", false),
        ("blocked_by_dependency_setup", false),
    ];
    for (status, passes) in rows {
        let contract = CompletionContract {
            deferred_verify_requirements: vec![deferred("echo ok", status)],
            ..base()
        };
        let (report, observations) = contract.verify_with_goal_observed(dir.path(), "");
        assert_eq!(report.is_pass(), passes, "{status}: {report:?}");
        assert!(observations.is_empty());
        if !passes {
            assert!(
                has_failure_prefix(&report, "deferred verify requirement pending"),
                "{status}: {report:?}"
            );
        }
    }
}

// kills: completion.rs:603/605/609 through the verify path: a non-generic
// profile runs its final check, the generic aliases do not.
#[test]
fn only_a_non_generic_profile_runs_the_profile_check() {
    let rows = [
        (None, true),
        (Some("generic"), true),
        (Some("default"), true),
        (Some("none"), true),
        (Some(NEXT), false),
        (Some("python-cli"), false),
    ];
    for (profile, passes) in rows {
        let dir = tempfile::tempdir().unwrap();
        let contract = CompletionContract {
            profile: profile.map(s),
            ..base()
        };
        let report = contract.verify(dir.path());
        assert_eq!(report.is_pass(), passes, "{profile:?}: {report:?}");
    }
}

// kills: completion.rs:394:24 delete ! (profile compile errors are copied into
// the report when the report does not already hold them)
#[test]
fn profile_compile_errors_are_copied_into_the_report() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pyproject.toml"),
        "[project]\nname = \"app\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("src/app")).unwrap();
    let entrypoint = dir.path().join("src/app/main.py");
    write_executable(&entrypoint, "def broken(:\n    pass\n");
    let contract = CompletionContract {
        profile: Some(s("python-cli")),
        ..base()
    };
    let report = contract.verify(dir.path());
    assert!(!report.is_pass(), "{report:?}");
    assert!(!report.compile_errors.is_empty(), "{report:?}");
    assert_eq!(report.compile_errors.len(), 1, "{report:?}");
    assert!(report.compile_errors[0].path.ends_with("main.py"));
}

const GATE_KEYS: [&str; 6] = [
    "missing_required_capabilities:",
    "missing_required_evidence:",
    "missing_required_evidence:required_obligation:",
    "missing_required_obligation_target:",
    "weak_verification_evidence:",
    "inconclusive_acceptance:",
];

/// The runtime acceptance failure kinds present in a report.
fn gate_keys(report: &VerificationReport) -> Vec<&'static str> {
    GATE_KEYS
        .iter()
        .copied()
        .filter(|key| {
            report.profile_failures.iter().any(|failure| {
                if *key == "missing_required_evidence:" {
                    failure.starts_with(key)
                        && !failure.starts_with("missing_required_evidence:required_obligation:")
                } else {
                    failure.starts_with(key)
                }
            })
        })
        .collect()
}

// kills: completion.rs:478:12 delete !, 479:16 delete !, 489:39 replace != with
// ==, 490:25 replace || with &&, 490:28 delete !, 494:16, 500:16, 506:16,
// 518:16 delete !
#[test]
fn runtime_acceptance_failures_are_reported_per_kind() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "x").unwrap();
    let rows: Vec<(&str, CompletionContract, Vec<&str>)> = vec![
        ("no acceptance requirements", base(), vec![]),
        (
            "unsupported capability",
            CompletionContract {
                required_capabilities: vec![s("bogus_cap")],
                ..base()
            },
            vec!["missing_required_capabilities:"],
        ),
        (
            "bound verify command missing",
            CompletionContract {
                required_evidence: vec![s("bound_verify_command")],
                ..base()
            },
            vec!["missing_required_evidence:"],
        ),
        (
            "bound verify command missing, reproducer not registered",
            CompletionContract {
                required_evidence: vec![s("bound_verify_command")],
                verify_commands: vec![s("true")],
                fix_reproducer_command: Some(s("echo reproducer")),
                ..base()
            },
            vec!["missing_required_evidence:"],
        ),
        (
            "bound verify command waived by a registered reproducer",
            CompletionContract {
                required_evidence: vec![s("bound_verify_command")],
                verify_commands: vec![s("true")],
                fix_reproducer_command: Some(s("true")),
                ..base()
            },
            vec![],
        ),
        (
            "other evidence is not waived by a registered reproducer",
            CompletionContract {
                required_evidence: vec![s("test_artifact")],
                verify_commands: vec![s("true")],
                fix_reproducer_command: Some(s("true")),
                ..base()
            },
            vec!["missing_required_evidence:"],
        ),
        (
            "missing obligation",
            CompletionContract {
                required_obligations: vec![s("implementation")],
                ..base()
            },
            vec![
                "missing_required_evidence:required_obligation:",
                "missing_required_obligation_target:",
            ],
        ),
        (
            "weak verification with missing evidence",
            CompletionContract {
                required_paths: vec![s("a.txt")],
                required_evidence: vec![s("implementation_artifact")],
                verify_commands: vec![s("test -f a.txt")],
                ..base()
            },
            vec!["missing_required_evidence:", "weak_verification_evidence:"],
        ),
        (
            "unavailable browser readiness",
            CompletionContract {
                required_capabilities: vec![s("browser_interaction")],
                ..base()
            },
            vec!["missing_required_evidence:", "inconclusive_acceptance:"],
        ),
    ];
    for (name, contract, expected) in rows {
        let report = contract.verify(dir.path());
        assert_eq!(gate_keys(&report), expected, "{name}: {report:?}");
        assert_eq!(report.is_pass(), expected.is_empty(), "{name}: {report:?}");
    }

    // The waived evidence is named in the failure only when it is not waived.
    let waived = CompletionContract {
        required_evidence: vec![s("bound_verify_command"), s("test_artifact")],
        verify_commands: vec![s("true")],
        fix_reproducer_command: Some(s("true")),
        ..base()
    };
    let report = waived.verify(dir.path());
    let failure = report
        .profile_failures
        .iter()
        .find(|failure| failure.starts_with("missing_required_evidence:"))
        .expect("test_artifact is still missing");
    assert!(failure.contains("test_artifact"), "{failure}");
    assert!(!failure.contains("bound_verify_command"), "{failure}");

    let unwaived = CompletionContract {
        fix_reproducer_command: None,
        ..waived
    };
    let report = unwaived.verify(dir.path());
    let failure = report
        .profile_failures
        .iter()
        .find(|failure| failure.starts_with("missing_required_evidence:"))
        .expect("evidence is missing");
    assert!(failure.contains("bound_verify_command"), "{failure}");
    assert!(failure.contains("test_artifact"), "{failure}");
}
