use super::PlanLintError;
use crate::agent::step_runner::StepKind;
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
        return Err(PlanLintError::InvalidStepInstruction {
            step_id: step_id.to_string(),
            reason: format!(
                "inspect expected_paths must already exist in the workspace; missing {}. Use expected_paths: [] and verify: [] for discovery, and enforce final artifacts only at the final boundary",
                missing.join(", ")
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
