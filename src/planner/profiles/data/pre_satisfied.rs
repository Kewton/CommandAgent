use std::path::Path;

use serde_json::json;

use crate::eval_events;
use crate::planner::profile::domain_profile;
use crate::planner::step_plan::{PlanStep, StepKind};
use crate::planner::verify::VerificationReport;

pub(crate) fn profile_applies(profile: &str) -> bool {
    domain_profile(profile).id() == "data"
}

pub(crate) fn verify_first_applicable(root: &Path, step: &PlanStep) -> bool {
    matches!(step.step_kind(), StepKind::Implement | StepKind::Verify)
        && !step.verify.is_empty()
        && step
            .expected_paths
            .iter()
            .all(|path| crate::tools::path_guard::resolve_existing(root, path).is_ok())
}

pub(crate) fn emit_short_circuited(
    eval_events_path: Option<&Path>,
    step: &PlanStep,
    phase_scope: Option<&str>,
    report: &VerificationReport,
) {
    debug_assert!(report.is_pass());
    let failure_count = report.missing_paths.len()
        + report.command_failures.len()
        + report.verifier_command_false_negatives.len()
        + report.dependency_missing.len()
        + report.profile_failures.len()
        + report.compile_errors.len()
        + report.python_tracebacks.len();
    eval_events::emit(
        eval_events_path,
        json!({
            "event": "step_short_circuited",
            "at": "start",
            "step_id": step.id,
            "step_kind": step.kind,
            "phase_scope": phase_scope.unwrap_or(""),
            "reason": "pre_satisfied_verified",
            "required_paths": step.expected_paths,
            "verify_commands": step.verify,
            "verification_summary": {
                "status": "pass",
                "expected_paths_checked": step.expected_paths.len(),
                "verify_commands_executed": step.verify.len(),
                "runtime_normalization_count": report.runtime_command_normalizations.len(),
                "failure_count": failure_count,
            },
            "session_scope": "plan-run-step",
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(kind: &str, expected_paths: &[&str], verify: &[&str]) -> PlanStep {
        PlanStep {
            id: "observed-step".to_string(),
            kind: kind.to_string(),
            expected_result: "pass".to_string(),
            instruction: "Use the measured artifacts".to_string(),
            expected_paths: expected_paths
                .iter()
                .map(|path| (*path).to_string())
                .collect(),
            verify: verify
                .iter()
                .map(|command| (*command).to_string())
                .collect(),
        }
    }

    #[test]
    fn requires_supported_kind_declared_verify_and_existing_paths() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ready.json"), "{}\n").unwrap();

        assert!(verify_first_applicable(
            dir.path(),
            &step("implement", &["ready.json"], &["test -f ready.json"]),
        ));
        assert!(verify_first_applicable(
            dir.path(),
            &step("verify", &[], &["test -f ready.json"]),
        ));
        assert!(!verify_first_applicable(
            dir.path(),
            &step("inspect", &["ready.json"], &["test -f ready.json"]),
        ));
        assert!(!verify_first_applicable(
            dir.path(),
            &step("implement", &["missing.json"], &["test -f missing.json"]),
        ));
        assert!(!verify_first_applicable(
            dir.path(),
            &step("verify", &["ready.json"], &[]),
        ));
    }
}
