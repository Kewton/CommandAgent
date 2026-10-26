use std::path::Path;

use super::RuntimeBashPolicyDecision;
use crate::planner::verify::VerifyCommandViolationKind;
use crate::tools::bash_write_guard::verifier_mutation;

pub(super) fn apply(
    mut decision: RuntimeBashPolicyDecision,
    command: &str,
    root: &Path,
) -> RuntimeBashPolicyDecision {
    if !crate::tools::placeholder_path::detected(command)
        && crate::tools::bash::blocked_reason(command, false).is_some()
    {
        return decision;
    }
    let Some(rejection) = crate::tools::bash::path_confinement_rejection(command, root) else {
        return reject_verifier_mutation(decision, command);
    };
    decision.verifier_policy_ok = !decision.verifier_policy_checked;
    decision.deterministic_verifier_evidence = false;
    decision.blocked = true;
    decision.policy_error_kind = "bash_path_confinement_error";
    decision.violation_kind = if rejection.operation == "placeholder path" {
        "workspace_path_placeholder"
    } else if matches!(
        rejection.operation.as_str(),
        "path reference" | "working directory"
    ) {
        "workspace_path_outside_root"
    } else {
        "workspace_write_outside_root"
    };
    decision.reason = rejection.reason;
    if crate::tools::placeholder_path::detected(command)
        && crate::tools::placeholder_path::record_rejection(root, command).is_err()
    {
        decision
            .reason
            .push_str(" Private evidence recording failed; command remains blocked.");
    }
    decision.normalized_command = None;
    decision.split_segments.clear();
    decision.normalization_kind = "";
    decision.normalization_reason.clear();
    decision
}

/// Refuses an explicit workspace-update command in a Verify or Report step
/// before it runs (Issue #861). The raw command, the normalized command, and
/// every split segment are read, so the update is refused whichever of them
/// carries it. The rejected command's normalization is dropped so it is never
/// executed.
fn reject_verifier_mutation(
    mut decision: RuntimeBashPolicyDecision,
    command: &str,
) -> RuntimeBashPolicyDecision {
    if !decision.verifier_policy_checked || decision.blocked {
        return decision;
    }
    let mutation = std::iter::once(command)
        .chain(decision.normalized_command.as_deref())
        .chain(
            decision
                .split_segments
                .iter()
                .map(|segment| segment.command.as_str()),
        )
        .find_map(|text| verifier_mutation::rejection(text, true));
    let Some(mutation) = mutation else {
        return decision;
    };
    decision.verifier_policy_ok = false;
    decision.deterministic_verifier_evidence = false;
    decision.blocked = true;
    decision.policy_error_kind = "verify_command_policy_error";
    decision.violation_kind = VerifyCommandViolationKind::Blocked.as_str();
    decision.reason = if mutation.opaque {
        format!(
            "{} step runtime Bash must not run a command whose body cannot be inspected: {} is not accepted as deterministic verifier evidence; run the build, test, or read-only verification command directly (for example `npm test`, `node smoke-check.js`, or `sh smoke-check.sh`) and change files with Write or Edit",
            decision.step_kind,
            mutation.describe()
        )
    } else {
        format!(
            "{} step runtime Bash must not update the workspace: {} is an explicit update command and is not accepted as deterministic verifier evidence; change files with Write or Edit and keep Bash for build, test, or read-only verification commands",
            decision.step_kind,
            mutation.describe()
        )
    };
    decision.normalized_command = None;
    decision.split_segments.clear();
    decision.normalization_kind = "";
    decision.normalization_reason.clear();
    decision
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::loop_run::{RunSessionStepKind, RuntimeBashPolicyDecision};

    #[test]
    fn issue441_only_placeholders_are_exempt_from_generic_error_repeats() {
        let mut state = super::super::state::RecoverableToolErrorState::default();
        let outside = anyhow::anyhow!("bash_path_confinement_error: outside workspace");
        let placeholder = anyhow::anyhow!(
            "bash_path_confinement_error: placeholder path detected; retry with workspace-relative paths"
        );
        assert_eq!(state.record("Bash", &outside), 1);
        for _ in 0..5 {
            assert_eq!(state.record("Bash", &placeholder), 0);
        }
        assert_eq!(state.record("Bash", &outside), 2);
        assert_eq!(state.record("Bash", &outside), 3);
    }

    #[test]
    fn issue441_verify_placeholder_is_blocked_without_verifier_normalization() {
        let root = tempfile::tempdir().unwrap();
        let command = "cat /Users/<user>/project/a.txt";
        let decision = apply(
            RuntimeBashPolicyDecision::for_step(RunSessionStepKind::Verify, command, root.path()),
            command,
            root.path(),
        );
        assert!(decision.blocked);
        assert!(!decision.verifier_policy_ok);
        assert!(!decision.deterministic_verifier_evidence);
        assert!(decision.normalized_command.is_none());
        assert!(decision.split_segments.is_empty());
        assert_eq!(decision.violation_kind, "workspace_path_placeholder");
        assert!(decision.reason.contains("placeholder path detected"));
    }

    #[test]
    fn issue441_evidence_failure_keeps_runtime_rejection_honest() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join(".commandagent"), "not a directory").unwrap();
        let command = "echo <redacted>";
        let decision = apply(
            RuntimeBashPolicyDecision::for_step(
                RunSessionStepKind::Implement,
                command,
                root.path(),
            ),
            command,
            root.path(),
        );
        assert!(decision.blocked);
        assert!(decision.reason.contains("evidence recording failed"));
    }

    #[test]
    fn runtime_policy_marks_outside_writes_blocked_with_reason() {
        let fixture = tempfile::tempdir().unwrap();
        let outside = fixture.path().parent().unwrap().join("issue-206-outside");
        let command = format!("ln -s /usr/bin/python3 {}", outside.display());
        let decision = apply(
            RuntimeBashPolicyDecision::for_step(
                RunSessionStepKind::Implement,
                &command,
                fixture.path(),
            ),
            &command,
            fixture.path(),
        );

        assert!(decision.blocked);
        assert_eq!(decision.policy_error_kind, "bash_path_confinement_error");
        assert_eq!(decision.violation_kind, "workspace_write_outside_root");
        assert!(decision.reason.contains("Gate 1 workspace boundary"));
    }

    #[test]
    fn runtime_policy_keeps_workspace_writes_allowed() {
        let fixture = tempfile::tempdir().unwrap();
        let command = "mkdir -p output && printf ok > output/result.txt";
        let decision = apply(
            RuntimeBashPolicyDecision::for_step(
                RunSessionStepKind::Implement,
                command,
                fixture.path(),
            ),
            command,
            fixture.path(),
        );

        assert!(!decision.blocked);
    }

    fn decide(step: RunSessionStepKind, command: &str, root: &Path) -> RuntimeBashPolicyDecision {
        apply(
            RuntimeBashPolicyDecision::for_step(step, command, root),
            command,
            root,
        )
    }

    #[test]
    fn issue861_verify_and_report_refuse_explicit_update_commands_before_execution() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "sed -i 's/1/2/' src/a.ts",
                "sed -i '' 's/1/2/' src/a.ts",
                "sed --in-place 's/1/2/' src/a.ts",
                "perl -pi -e 's/1/2/' src/a.ts",
                "rm src/a.ts",
                "mv src/a.ts src/b.ts",
                "cp fixtures/a.ts src/a.ts",
                "tee src/a.ts",
                "touch marker",
                "truncate -s 0 src/a.ts",
                "npm run build && sed -i 's/1/2/' src/a.ts",
                "cp a b ; npm run build",
                "env FOO=1 /usr/bin/sed -i s/1/2/ a.ts",
            ] {
                let decision = decide(step, command, root.path());
                assert!(decision.blocked, "{step:?} {command}");
                assert!(!decision.verifier_policy_ok, "{step:?} {command}");
                assert!(
                    !decision.deterministic_verifier_evidence,
                    "{step:?} {command}"
                );
                assert_eq!(decision.policy_error_kind, "verify_command_policy_error");
                assert_eq!(decision.violation_kind, "blocked", "{step:?} {command}");
                assert!(
                    decision.normalized_command.is_none() && decision.split_segments.is_empty(),
                    "{step:?} {command}"
                );
            }
        }
    }

    #[test]
    fn issue861_existing_redirect_rejection_is_kept() {
        let root = tempfile::tempdir().unwrap();
        for command in ["echo x > src/a.ts", "printf x >> src/a.ts"] {
            let decision = decide(RunSessionStepKind::Verify, command, root.path());
            assert!(decision.blocked, "{command}");
            assert!(!decision.deterministic_verifier_evidence, "{command}");
            assert_eq!(decision.violation_kind, "shell_control_syntax", "{command}");
        }
    }

    #[test]
    fn issue861_update_reason_names_the_update_and_the_step() {
        let root = tempfile::tempdir().unwrap();
        let decision = decide(
            RunSessionStepKind::Verify,
            "sed -i 's/1/2/' src/a.ts",
            root.path(),
        );
        assert!(decision.reason.contains("verify step runtime Bash"));
        assert!(decision.reason.contains("`sed -i`"));
        assert!(
            decision
                .reason
                .contains("not accepted as deterministic verifier evidence")
        );
    }

    #[test]
    fn issue861_verify_keeps_build_and_read_only_verifiers() {
        let root = tempfile::tempdir().unwrap();
        for command in [
            "npm run build",
            "npx tsc",
            "npx tsc --noEmit",
            "node smoke-check.js",
            "pytest",
            "python3 -m pytest",
            "python3 -m compileall -q src",
            "sed -n '1,5p' src/a.ts",
            "grep -n 'sed -i' src/a.ts",
        ] {
            let decision = decide(RunSessionStepKind::Verify, command, root.path());
            assert!(!decision.blocked, "{command}: {}", decision.reason);
            assert!(decision.verifier_policy_ok, "{command}");
            assert!(decision.deterministic_verifier_evidence, "{command}");
        }
    }

    #[test]
    fn issue861_implement_and_setup_keep_workspace_updates() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Implement, RunSessionStepKind::Setup] {
            for command in ["sed -i 's/1/2/' src/a.ts", "rm src/a.ts", "cp a b"] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}");
            }
        }
    }

    #[test]
    fn issue861_outside_root_rejection_keeps_priority_over_update_rejection() {
        let root = tempfile::tempdir().unwrap();
        let decision = decide(
            RunSessionStepKind::Verify,
            "cp a /etc/issue861-outside",
            root.path(),
        );
        assert!(decision.blocked);
        assert_eq!(decision.policy_error_kind, "bash_path_confinement_error");
    }

    #[test]
    fn issue861_corpus_decision_cases_match_the_runtime_policy() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/verify-runtime-bash-mutation/fixtures/cases.json");
        let cases: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut checked = 0;
        for case in cases["decision_cases"].as_array().unwrap() {
            let command = case["command"].as_str().unwrap();
            for step in case["steps"].as_array().unwrap() {
                let step_kind = match step.as_str().unwrap() {
                    "verify" => RunSessionStepKind::Verify,
                    "report" => RunSessionStepKind::Report,
                    "implement" => RunSessionStepKind::Implement,
                    "setup" => RunSessionStepKind::Setup,
                    other => panic!("unknown step {other}"),
                };
                let id = format!("{} / {step}", case["id"]);
                let decision = decide(step_kind, command, root.path());
                match case["expect"].as_str().unwrap() {
                    "reject" => {
                        assert!(decision.blocked, "{id}");
                        assert!(!decision.verifier_policy_ok, "{id}");
                        assert!(!decision.deterministic_verifier_evidence, "{id}");
                        assert!(decision.normalized_command.is_none(), "{id}");
                        assert!(decision.split_segments.is_empty(), "{id}");
                        let needle = case["reason_contains"].as_str().unwrap();
                        assert!(
                            decision.reason.contains(needle),
                            "{id}: {}",
                            decision.reason
                        );
                    }
                    "allow" => assert!(!decision.blocked, "{id}: {}", decision.reason),
                    other => panic!("unknown expectation {other}"),
                }
                checked += 1;
            }
        }
        assert!(checked >= 40, "corpus table shrank: {checked}");
    }

    #[test]
    fn issue861_null_sink_redirect_is_not_an_update_but_a_workspace_redirect_is() {
        let root = tempfile::tempdir().unwrap();
        // The refusal is the update classifier's, not the normalizer's.
        assert!(verifier_mutation::find("npm test 2>/dev/null", true).is_none());
        assert!(verifier_mutation::find("npm test > out.txt", true).is_some());
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            let null = decide(step, "npm test 2>/dev/null", root.path());
            assert!(
                !null.reason.contains("must not update the workspace"),
                "{step:?}: {}",
                null.reason
            );
            let file = decide(step, "npm test > out.txt", root.path());
            assert!(file.blocked, "{step:?}");
            assert!(!file.deterministic_verifier_evidence, "{step:?}");
        }
    }

    #[test]
    fn issue861_update_visible_only_after_normalization_is_refused() {
        let root = tempfile::tempdir().unwrap();
        // A vertical tab is an ordinary word character to the lexical guard but
        // a blank to the verify normalization, so only the normalized text
        // names `sed -i`.
        let command = "sed\u{b}-i s/1/2/ src/a.ts";
        assert!(verifier_mutation::find(command, true).is_none());
        let unrejected =
            RuntimeBashPolicyDecision::for_step(RunSessionStepKind::Verify, command, root.path());
        assert!(
            unrejected
                .normalized_command
                .as_deref()
                .is_some_and(|normalized| verifier_mutation::find(normalized, true).is_some()),
            "{:?}",
            unrejected.normalized_command
        );
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            let decision = decide(step, command, root.path());
            assert!(decision.blocked, "{step:?}");
            assert!(decision.reason.contains("`sed -i`"), "{}", decision.reason);
            assert!(decision.normalized_command.is_none());
        }
    }

    const OPAQUE_REFUSALS: [&str; 24] = [
        "sh -c 'cat a.txt'",
        "sh -c 'print(1)'",
        "sh -c 'cp always-pass.sh smoke-check.sh'",
        "bash -lc 'npm test'",
        "/bin/sh -c true",
        "env FOO=1 sh -c true",
        "timeout 5 bash -c true",
        "timeout -sKILL 5 sh -c true",
        "eval true",
        "xargs cat",
        "busybox cat a.txt",
        "find . -exec cat {} +",
        "find . -delete",
        "node -e 'console.log(1)'",
        "node -p '1'",
        "node --eval='1'",
        "node --print 1",
        "python3 -c 'print(1)'",
        "perl -e 'print 1'",
        "perl -ne 'print' a.txt",
        "sed -n '1,5w out.txt' a.txt",
        "sed 's/a/b/' a.txt",
        "gsed -f prog.sed a.txt",
        "(sh -c true)",
    ];

    fn assert_refused_before_execution(decision: &RuntimeBashPolicyDecision, id: &str) {
        assert!(decision.blocked, "{id}");
        assert!(!decision.verifier_policy_ok, "{id}");
        assert!(!decision.deterministic_verifier_evidence, "{id}");
        assert_eq!(
            decision.policy_error_kind, "verify_command_policy_error",
            "{id}"
        );
        assert_eq!(decision.violation_kind, "blocked", "{id}");
        assert!(decision.normalized_command.is_none(), "{id}");
        assert!(decision.split_segments.is_empty(), "{id}");
        assert!(decision.normalization_kind.is_empty(), "{id}");
        assert!(decision.normalization_reason.is_empty(), "{id}");
    }

    #[test]
    fn issue871_verify_and_report_refuse_opaque_forms_before_execution() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in OPAQUE_REFUSALS {
                let id = format!("{step:?} {command}");
                let decision = decide(step, command, root.path());
                assert_refused_before_execution(&decision, &id);
                assert!(
                    decision.reason.contains("cannot be inspected")
                        && decision
                            .reason
                            .contains("not accepted as deterministic verifier evidence"),
                    "{id}: {}",
                    decision.reason
                );
            }
        }
    }

    #[test]
    fn issue871_verify_keeps_canonical_verifiers_and_literal_text() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "npm test",
                "npm run build",
                "npx tsc",
                "npx tsc --noEmit",
                "node smoke-check.js",
                "node smoke-check.js --label=-e",
                "node --check smoke-check.js",
                "pytest",
                "python3 -m pytest",
                "python -m pytest -c cfg.ini",
                "python3 -m compileall -q src",
                "sh smoke-check.sh",
                "sed -n '1,5p' src/a.ts",
                "sed -n 1,10p src/a.ts",
                "find . -name '*.ts'",
                "grep -n 'node -e' src/a.ts",
                "grep -rn \"sh -c\" src",
                "echo 'sh -c true'",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
                assert!(decision.verifier_policy_ok, "{step:?} {command}");
                assert!(
                    decision.deterministic_verifier_evidence,
                    "{step:?} {command}"
                );
            }
        }
    }

    #[test]
    fn issue871_launcher_cluster_perl_node_and_data_url_forms_are_refused() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "bash -oc pipefail 'cp a b'",
                "bash -Oc extglob 'cp a b'",
                "bash -eoc pipefail 'cp a b'",
                "env FOO=1 bash -oc pipefail 'cp a b'",
                "perl -M File::Copy -e 'print 1'",
                "perl -MFile::Copy -m Foo -e 1",
                "flock lockfile sh -c 'cp a b'",
                "chrt 1 sh -c 'cp a b'",
                "taskset 1 sh -c 'cp a b'",
                "unshare sh -c 'cp a b'",
                "strace -o /dev/null sh -c 'cp a b'",
                "watch -n1 sh -c 'cp a b'",
                "npx sh -c 'cp a b'",
                "npm exec -- sh -c 'cp a b'",
                "pnpm exec sh -c 'cp a b'",
                "npx -c 'cp a b'",
                "npx --call 'cp a b'",
                "npm exec -c 'cp a b'",
                "flock lockfile -c 'cp a b'",
                "script -qc 'cp a b' /dev/null",
                "node --import data:text/javascript,1 smoke-check.js",
                "node --import=data:text/javascript,1 smoke-check.js",
                "node -r data:text/javascript,1 smoke-check.js",
                "node --loader=data:text/javascript,1 smoke-check.js",
                "node --experimental-loader data:text/javascript,1 smoke-check.js",
                "perl -M'5;system(1)' script.pl",
                "perl -m'Foo bar' script.pl",
                "node --unknown -e 1",
                "node --foo=1 -p 1",
                "node --inspect-port 1 -e 1",
            ] {
                let id = format!("{step:?} {command}");
                let decision = decide(step, command, root.path());
                assert_refused_before_execution(&decision, &id);
            }
            for command in [
                "bash -o pipefail smoke-check.sh",
                "bash -O extglob smoke-check.sh",
                "perl -M File::Copy script.pl -e",
                "perl -MFile::Copy script.pl",
                "perl -MFoo=a,b script.pl",
                "perl -M-strict script.pl",
                "node --trace-uncaught smoke-check.js -e",
                "node --trace-uncaught smoke-check.js",
                "node --no-warnings --trace-exit smoke-check.js -p",
                "node --require ./setup.js smoke-check.js",
                "node --import ./setup.mjs smoke-check.mjs",
                "npx tsc --noEmit",
                "npx tsc",
                "npx vitest run",
                "npx vitest run -c vitest.config.ts",
                "npm exec -- tsc --noEmit",
                "npm test",
                "pnpm exec tsc --noEmit",
                "echo sh -c",
                "grep -r 'sh -c' src",
                "grep 'node -e' README.md",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
                assert!(decision.verifier_policy_ok, "{step:?} {command}");
            }
        }
    }

    #[test]
    fn issue871_nested_launcher_string_options_are_refused() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "chrt 1 npx -c 'cp a b'",
                "taskset 1 npm exec -c 'cp a b'",
                "setsid script -qc 'cp a b' /dev/null",
                "nice -n 5 chrt 1 flock f -c 'cp a b'",
                "timeout 5 strace -o /dev/null npx -c 'cp a b'",
                "chrt 1 taskset 1 setsid sh -c 'cp a b'",
            ] {
                let id = format!("{step:?} {command}");
                assert_refused_before_execution(&decide(step, command, root.path()), &id);
            }
            for command in [
                "chrt 1 npx tsc --noEmit",
                "taskset -c 0 npm exec -- tsc --noEmit",
                "nice -n 5 chrt 1 npx vitest run",
                "timeout 5 strace -o /dev/null npm test",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
            }
        }
    }

    #[test]
    fn issue871_unresolved_prefix_option_value_and_stdin_shell_forms_are_refused() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "nice -n 5 npx -c 'cp a b'",
                "nice -n 5 flock f -c 'cp a b'",
                "nice -n 5 npm exec -c 'cp a b'",
                "nice -n 5 sh -c 'cp a b'",
                "nice -n5 npx -c 'cp a b'",
                "nice -n5 flock f -c 'cp a b'",
                "nice -n5 npm exec -c 'cp a b'",
                "nice -n5 sh -c 'cp a b'",
                "timeout -s KILL 5 npx -c 'cp a b'",
                "timeout -s KILL 5 flock f -c 'cp a b'",
                "timeout -s KILL 5 npm exec -c 'cp a b'",
                "timeout -s KILL 5 sh -c 'cp a b'",
                "timeout -sKILL 5 npx -c 'cp a b'",
                "timeout -sKILL 5 flock f -c 'cp a b'",
                "timeout -sKILL 5 npm exec -c 'cp a b'",
                "timeout -sKILL 5 sh -c 'cp a b'",
                "npm exec -p -c 'cp a b'",
                "npm x -p --call 'cp a b'",
                "npx -p -c 'cp a b'",
                "chrt -o 0 bash -sc 'cp a b'",
                "chrt -o 0 bash -s -c 'cp a b'",
                "taskset 1 sh -cs 'cp a b'",
                "bash -sc 'cp a b'",
                "bash -s -c 'cp a b'",
            ] {
                let id = format!("{step:?} {command}");
                let decision = decide(step, command, root.path());
                // The attached `-n5`/`-sKILL` forms are also stopped earlier by path
                // confinement; either way the command never runs.
                assert!(decision.blocked, "{id}");
                assert!(!decision.deterministic_verifier_evidence, "{id}");
                if decision.policy_error_kind == "verify_command_policy_error" {
                    assert_refused_before_execution(&decision, &id);
                }
            }
            for command in [
                "npx -p typescript tsc --noEmit",
                "nice -n5 npx tsc --noEmit",
                "timeout -sKILL 5 npx vitest run",
                "chrt 1 npx tsc --noEmit",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
            }
        }
    }

    #[test]
    fn issue871_launcher_option_values_stdin_shells_and_config_flags_are_told_apart() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "npx --cache npm-cache -c 'echo hi'",
                "npx --cache=npm-cache -c 'echo hi'",
                "npm exec --cache npm-cache --call 'echo hi'",
                "npm exec vitest -c 'echo hi'",
                "chrt -o 0 bash -s",
                "chrt -o 0 bash -s -- a b",
                "chrt 1 bash",
                "nice -n 5 chrt 1 bash -s",
                "bash -s --help",
                "bash -s --version",
                "chrt 1 bash -s --version",
                "nice -n 5 npx -c 'true>smoke-check.sh'",
                "timeout -s KILL 5 npx -c 'true>smoke-check.sh'",
                "nice -n5 npx -c 'true>smoke-check.sh'",
                "timeout -sKILL 5 npx -c 'true>smoke-check.sh'",
            ] {
                let id = format!("{step:?} {command}");
                assert_refused_before_execution(&decide(step, command, root.path()), &id);
            }
            for command in [
                "npx -p typescript tsc --noEmit",
                "npx vitest run -c vitest.config.ts",
                "npm exec -- tsc --noEmit",
                "npm exec -- vitest run -c cfg",
                "npx vitest -c vitest.config.ts",
                "npx --yes vitest run -c vitest.config.ts",
                "npx -y jest -c jest.config.js",
                "npm exec -- vitest -c vitest.config.ts",
                "npx --yes vitest -c cfg",
                "chrt 1 bash smoke-check.sh",
                "chrt 1 bash -- smoke-check.sh -c",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
            }
        }
    }

    #[test]
    fn issue871_only_the_closed_option_list_is_read_before_a_package() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "npx -C . -c 'true>smoke-check.sh'",
                "npx -m label -c 'true>smoke-check.sh'",
                "npx --cal 'true>smoke-check.sh'",
                "npm exec --cal 'true>smoke-check.sh'",
                "npm x -C . --call 'true>smoke-check.sh'",
                "npm exec --prefix . -c 'true>smoke-check.sh'",
                "npx -yc 'true>smoke-check.sh'",
                "npx --cac npm-cache --call 'true>smoke-check.sh'",
                "npx --offline vitest -c cfg",
                "npm exec --message label --call 'true>smoke-check.sh'",
                "nice -n5 chrt 1 bash -s",
            ] {
                let id = format!("{step:?} {command}");
                assert_refused_before_execution(&decide(step, command, root.path()), &id);
            }
            for command in [
                "npx --package=typescript tsc",
                "npx -p typescript -y tsc --noEmit",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
            }
        }
    }

    #[test]
    fn issue871_option_values_are_not_taken_for_the_package() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "npx --no true -c 'true>smoke-check.sh'",
                "npx --no false --call 'true>smoke-check.sh'",
                "npx --no true --call 'true>smoke-check.sh'",
                "npx --no false -c 'true>smoke-check.sh'",
                "npx -y true -c 'true>smoke-check.sh'",
                "npm exec --no false --call 'true>smoke-check.sh'",
                "npx -p typescript -c 'true>smoke-check.sh'",
                "npx --package typescript --call 'true>smoke-check.sh'",
                "npx --package=typescript -c 'true>smoke-check.sh'",
                "npx -w web -c 'true>smoke-check.sh'",
                "npm exec --workspace web --call 'true>smoke-check.sh'",
                "npm exec -w web -c 'true>smoke-check.sh'",
            ] {
                let id = format!("{step:?} {command}");
                assert_refused_before_execution(&decide(step, command, root.path()), &id);
            }
            for command in [
                "npx --yes tsc --noEmit",
                "npx --yes vitest run -c vitest.config.ts",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
            }
        }
    }

    #[test]
    fn issue871_before_a_package_only_yes_and_package_options_are_read() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "npx -q --no-install vitest -c cfg",
                "npm exec -p -- tsc --noEmit",
                "npx --package typescript tsc --noEmit",
                "npm exec -w web -- tsc",
                "npm exec --workspace=web -- tsc --noEmit",
                "npx --quiet tsc",
                "npx --yes true",
                "npx --no true tsc --noEmit",
                "npx --quiet false vitest run -c vitest.config.ts",
                "npx --no tsc --noEmit",
                "npx --yes true -c 'true>smoke-check.sh'",
                "npx --yes null -c 'true>smoke-check.sh'",
                "npx -y undefined --call 'true>smoke-check.sh'",
                "npx --yes '' -c 'true>smoke-check.sh'",
                "npx -y -p typescript tsc --noEmit",
                "npx --ignore-existing tsc",
                "npx --package typescript tsc",
                "npm exec vitest",
                "npm exec -p typescript -- tsc",
                "npm x -y tsc --noEmit",
                "npm exec --yes true -- tsc",
                "npm exec --yes -- tsc --call",
                "npm exec --yes -- tsc --noEmit",
                "npm exec -y -- tsc",
                "npx -y -- tsc",
            ] {
                let id = format!("{step:?} {command}");
                assert_refused_before_execution(&decide(step, command, root.path()), &id);
            }
            for command in [
                "npm exec --package=typescript -- tsc --noEmit",
                "npx --yes tsc --noEmit",
                "npx --package=typescript --yes tsc",
            ] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}: {}", decision.reason);
            }
        }
    }

    #[test]
    fn issue871_raw_check_alone_refuses_a_form_the_other_texts_do_not_show() {
        let root = tempfile::tempdir().unwrap();
        let mut decision = RuntimeBashPolicyDecision::for_step(
            RunSessionStepKind::Verify,
            "npm test",
            root.path(),
        );
        decision.normalized_command = Some("npm test".to_string());
        decision.split_segments.clear();
        let decision = apply(decision, "sh -c true", root.path());
        assert_refused_before_execution(&decision, "raw only");
        assert!(decision.reason.contains("`sh -c`"), "{}", decision.reason);
    }

    #[test]
    fn issue871_normalized_check_alone_refuses_a_form_the_raw_text_does_not_show() {
        let root = tempfile::tempdir().unwrap();
        let mut decision = RuntimeBashPolicyDecision::for_step(
            RunSessionStepKind::Verify,
            "npm test",
            root.path(),
        );
        decision.normalized_command = Some("sh -c true".to_string());
        decision.split_segments.clear();
        let decision = apply(decision, "npm test", root.path());
        assert_refused_before_execution(&decision, "normalized only");
        assert!(decision.reason.contains("`sh -c`"), "{}", decision.reason);
    }

    #[test]
    fn issue871_implement_and_setup_keep_opaque_forms() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Implement, RunSessionStepKind::Setup] {
            for command in ["sh -c 'cp a b'", "node -p '1'", "sed -n '1,5w out' a.txt"] {
                let decision = decide(step, command, root.path());
                assert!(!decision.blocked, "{step:?} {command}");
            }
        }
    }

    #[test]
    fn issue871_compound_with_a_prior_sentinel_is_refused_as_a_whole() {
        let root = tempfile::tempdir().unwrap();
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            for command in [
                "sh make-sentinel.sh && sh -c 'cp always-pass.sh smoke-check.sh'",
                "npm test; node -p '1'",
                "sh make-sentinel.sh && bash -lc true",
            ] {
                let id = format!("{step:?} {command}");
                let decision = decide(step, command, root.path());
                assert_refused_before_execution(&decision, &id);
            }
        }
    }

    #[test]
    fn issue871_form_visible_only_after_normalization_is_refused() {
        let root = tempfile::tempdir().unwrap();
        // A vertical tab is an ordinary word character to the lexical guard but
        // a blank to the verify normalization, so only the normalized text
        // names `sh -c`.
        let command = "sh\u{b}-c\u{b}true";
        assert!(verifier_mutation::rejection(command, true).is_none());
        let unrejected =
            RuntimeBashPolicyDecision::for_step(RunSessionStepKind::Verify, command, root.path());
        assert!(
            unrejected
                .normalized_command
                .as_deref()
                .is_some_and(|normalized| verifier_mutation::rejection(normalized, true).is_some()),
            "{:?}",
            unrejected.normalized_command
        );
        for step in [RunSessionStepKind::Verify, RunSessionStepKind::Report] {
            let decision = decide(step, command, root.path());
            assert_refused_before_execution(&decision, &format!("{step:?}"));
            assert!(decision.reason.contains("`sh -c`"), "{}", decision.reason);
        }
    }

    #[test]
    fn issue871_form_visible_only_in_a_split_segment_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let command = "npm install";
        let mut decision =
            RuntimeBashPolicyDecision::for_step(RunSessionStepKind::Verify, command, root.path());
        assert!(!decision.blocked, "{}", decision.reason);
        assert!(verifier_mutation::rejection(command, true).is_none());
        assert!(
            decision
                .normalized_command
                .as_deref()
                .is_none_or(|normalized| verifier_mutation::rejection(normalized, true).is_none())
        );
        // Only the split segment names the opaque form.
        let segment = decision.split_segments.first_mut().expect("one segment");
        match &mut segment.command {
            crate::planner::verify::RuntimeNormalizedCommand::DependencyInstall {
                command, ..
            } => *command = "sh -c true".to_string(),
            other => panic!("expected a dependency install segment: {other:?}"),
        }
        let decision = apply(decision, command, root.path());
        assert_refused_before_execution(&decision, "segment only");
        assert!(decision.reason.contains("`sh -c`"), "{}", decision.reason);
    }

    #[test]
    fn issue871_existing_update_reading_keeps_its_reason_over_an_opaque_form() {
        let root = tempfile::tempdir().unwrap();
        let decision = decide(
            RunSessionStepKind::Verify,
            "sh make-sentinel.sh && cp always-pass.sh smoke-check.sh",
            root.path(),
        );
        assert!(decision.blocked);
        assert!(decision.reason.contains("`cp`"), "{}", decision.reason);
        assert!(decision.reason.contains("must not update the workspace"));
    }

    #[test]
    fn issue871_outside_root_and_secret_rejections_keep_priority() {
        let root = tempfile::tempdir().unwrap();
        let outside = decide(
            RunSessionStepKind::Verify,
            "sh -c true && cp a /etc/issue871-outside",
            root.path(),
        );
        assert!(outside.blocked);
        assert_eq!(outside.policy_error_kind, "bash_path_confinement_error");
        let redirect = decide(
            RunSessionStepKind::Verify,
            "node -p 1 > out.txt",
            root.path(),
        );
        assert!(redirect.blocked);
        assert!(!redirect.deterministic_verifier_evidence);
        assert_eq!(redirect.violation_kind, "shell_control_syntax");
    }

    #[test]
    fn issue871_corpus_decision_cases_match_the_runtime_policy() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/apps/verify-runtime-bash-indirect/fixtures/cases.json");
        let cases: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let root = tempfile::tempdir().unwrap();
        let mut checked = 0;
        for case in cases["decision_cases"].as_array().unwrap() {
            let command = case["command"].as_str().unwrap();
            assert!(
                matches!(
                    case["origin"].as_str().unwrap(),
                    "synthetic" | "recorded-excerpt" | "existing-corpus"
                ),
                "{}",
                case["id"]
            );
            for step in case["steps"].as_array().unwrap() {
                let step_kind = match step.as_str().unwrap() {
                    "verify" => RunSessionStepKind::Verify,
                    "report" => RunSessionStepKind::Report,
                    "implement" => RunSessionStepKind::Implement,
                    other => panic!("unknown step {other}"),
                };
                let id = format!("{} / {step}", case["id"]);
                let decision = decide(step_kind, command, root.path());
                match case["expect"].as_str().unwrap() {
                    "reject" => {
                        assert_refused_before_execution(&decision, &id);
                        let needle = case["reason_contains"].as_str().unwrap();
                        assert!(
                            decision.reason.contains(needle),
                            "{id}: {}",
                            decision.reason
                        );
                    }
                    "allow" => assert!(!decision.blocked, "{id}: {}", decision.reason),
                    other => panic!("unknown expectation {other}"),
                }
                checked += 1;
            }
        }
        assert!(checked >= 60, "corpus table shrank: {checked}");
    }
}
