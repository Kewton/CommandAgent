use std::path::Path;

use super::RuntimeBashPolicyDecision;

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
        return decision;
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
}
