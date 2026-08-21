use std::path::Path;

use super::RuntimeBashPolicyDecision;

pub(super) fn apply(
    mut decision: RuntimeBashPolicyDecision,
    command: &str,
    root: &Path,
) -> RuntimeBashPolicyDecision {
    if crate::tools::bash::blocked_reason(command, false).is_some() {
        return decision;
    }
    let Some(rejection) = crate::tools::bash::path_confinement_rejection(command, root) else {
        return decision;
    };
    decision.verifier_policy_ok = !decision.verifier_policy_checked;
    decision.deterministic_verifier_evidence = false;
    decision.blocked = true;
    decision.policy_error_kind = "bash_path_confinement_error";
    decision.violation_kind = if matches!(
        rejection.operation.as_str(),
        "path reference" | "working directory"
    ) {
        "workspace_path_outside_root"
    } else {
        "workspace_write_outside_root"
    };
    decision.reason = rejection.reason;
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
