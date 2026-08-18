use std::path::Path;

use serde_json::Value;

use crate::planner::lint::{PlanQualityContext, PlanQualityReport, PlanQualitySeverity};
use crate::planner::step_plan::{PlanStep, StepPlan};

pub(super) const EVIDENCE_PATH: &str = "evidence/promotion-decision.json";
pub(super) const REASON_CLASSES: &[&str] = &[
    "spec_expression_impossible",
    "registered_function_missing",
    "ui_requirement",
];

fn has_app_zone(root: &Path) -> bool {
    root.join("src/app-zone").exists() || root.join("app-zone").exists()
}

fn expected_zone_path(root: &Path) -> &'static str {
    if root.join("src/app-zone").exists() {
        "src/app-zone"
    } else {
        "app-zone"
    }
}

pub(super) fn verify(root: &Path) -> Result<(), String> {
    if !has_app_zone(root) {
        return Ok(());
    }
    let invalid = || "community_promotion_missing".to_string();
    let document: Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(EVIDENCE_PATH)).map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())?;
    let object = document.as_object().ok_or_else(invalid)?;
    if object.get("evidence_family").and_then(Value::as_str) != Some("promotion_decision")
        || object
            .get("attempt_id")
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        || !matches!(
            object.get("requested_level").and_then(Value::as_str),
            Some("L3" | "L4")
        )
        || object.get("decision").and_then(Value::as_str) != Some("promote")
        || !object
            .get("reason_class")
            .and_then(Value::as_str)
            .is_some_and(|reason| REASON_CLASSES.contains(&reason))
        || object.get("zone_path").and_then(Value::as_str) != Some(expected_zone_path(root))
    {
        return Err(invalid());
    }
    let lower = object
        .get("lower_level_result")
        .and_then(Value::as_object)
        .ok_or_else(invalid)?;
    let artifact_ref = lower
        .get("artifact_ref")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if lower.get("status").and_then(Value::as_str) != Some("pass")
        || crate::tools::path_guard::validate_workspace_relative(artifact_ref).is_err()
        || artifact_ref.starts_with("src/app-zone/")
        || artifact_ref.starts_with("app-zone/")
        || !root.join(artifact_ref).is_file()
    {
        return Err(invalid());
    }
    Ok(())
}

fn step_declares_zone_output(step: &PlanStep) -> bool {
    step.expected_paths.iter().any(|path| {
        let path = path.replace('\\', "/");
        path == "src/app-zone"
            || path.starts_with("src/app-zone/")
            || path == "app-zone"
            || path.starts_with("app-zone/")
    })
}

fn is_promotion_step(step: &PlanStep) -> bool {
    step.expected_paths.iter().any(|path| {
        matches!(
            path.replace('\\', "/").as_str(),
            EVIDENCE_PATH | "promotion_decision.json" | "promotion-decision.json"
        )
    }) || {
        let instruction = step.instruction.to_ascii_lowercase();
        instruction.contains("promotion_decision")
            || instruction.contains("promotion-decision")
            || instruction.contains("promotion decision")
    }
}

pub(crate) fn report_plan_quality(
    report: &mut PlanQualityReport,
    context: &PlanQualityContext,
    plan: &StepPlan,
) {
    if context.profile != super::PROFILE_ID {
        return;
    }
    let mut promotion_seen = false;
    for step in &plan.steps {
        if step_declares_zone_output(step) && !promotion_seen {
            report.push(
                PlanQualitySeverity::RetryableQuality,
                "community_promotion_step_missing",
                "community app-zone/L3 work requires a preceding promotion_decision step that records the passing L2 result and a closed reason_class",
                Some(step.id.clone()),
                Some(step.expected_paths.join(", ")),
            );
            return;
        }
        if is_promotion_step(step) {
            promotion_seen = true;
        }
    }
}
