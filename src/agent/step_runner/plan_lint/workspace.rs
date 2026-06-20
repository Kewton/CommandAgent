use super::PlanLintError;
use crate::agent::step_runner::StepKind;
use crate::agent::step_runner::artifact_graph::{ArtifactGraph, ArtifactLifecycle};
use crate::agent::step_runner::correction_evidence::PlanCorrectionEvidence;
use std::path::Path;

pub(super) fn lint_inspect_expected_paths_exist(
    step_id: &str,
    kind: StepKind,
    expected_paths: &[String],
    cwd: Option<&Path>,
) -> Result<(), PlanLintError> {
    if !matches!(kind, StepKind::Inspect) || expected_paths.is_empty() {
        return Ok(());
    }
    let Some(cwd) = cwd else {
        return Ok(());
    };
    let missing: Vec<_> = expected_paths
        .iter()
        .filter(|path| !cwd.join(path).exists())
        .cloned()
        .collect();
    if !missing.is_empty() {
        let mut graph = ArtifactGraph::new();
        for path in &missing {
            graph.add_path(
                path,
                ArtifactLifecycle::Required,
                "plan_lint.inspect_expected_paths",
            );
        }
        let reason = format!(
            "inspect expected_paths must already exist in the workspace; missing {}. Use expected_paths: [] and verify: [] for discovery, and enforce final artifacts only at the final boundary",
            missing.join(", ")
        );
        return Err(PlanLintError::ContractViolation {
            step_id: step_id.to_string(),
            reason: reason.clone(),
            evidence: Box::new(
                PlanCorrectionEvidence::new("plan_lint.artifact_graph")
                    .with_failed_step(step_id.to_string())
                    .with_violated_contract("inspect_future_artifact")
                    .with_reason_code("inspect_future_artifact")
                    .with_target_field("expected_paths")
                    .with_missing_paths(missing.clone())
                    .with_active_job("explicit_stop")
                    .with_repair_action("stop_no_admitted_target")
                    .with_required_action(
                        "do not inspect missing future artifacts; use expected_paths: [] and verify: [] for discovery, then create/edit the artifact in a later mutation step"
                    )
                    .with_disallowed_actions(vec![
                        "Do not create files inside an inspect step.",
                        "Do not keep missing future artifacts in inspect expected_paths.",
                    ])
                    .with_target_admission("rejected: inspect step cannot target missing future artifacts")
                    .with_target_priority("none: inspect future artifact has no admitted repair target")
                    .with_explicit_stop_reason("inspect_future_artifact")
                    .with_artifact_graph_summary(graph.summary())
                    .with_diagnostic(reason),
            ),
        });
    }
    Ok(())
}

pub(super) fn paths_exist(cwd: Option<&Path>, paths: &[String]) -> bool {
    let Some(cwd) = cwd else {
        return false;
    };
    !paths.is_empty() && paths.iter().all(|path| cwd.join(path).exists())
}
