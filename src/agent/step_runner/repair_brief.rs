#![allow(dead_code)]

use crate::agent::step_runner::repair_action_plan::AllowedToolCategory;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepairBriefStatus {
    Admitted,
    ExplicitStop,
}

impl RepairBriefStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::ExplicitStop => "explicit_stop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionEnvelopeStatus {
    Admitted,
    ExplicitStop,
    Rejected,
}

impl ActionEnvelopeStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::ExplicitStop => "explicit_stop",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairBrief {
    pub(crate) status: RepairBriefStatus,
    pub(crate) active_job: String,
    pub(crate) recovery_owner: String,
    pub(crate) repair_action: String,
    pub(crate) selected_failure_cluster: String,
    pub(crate) root_cause: String,
    pub(crate) repair_hypothesis: String,
    pub(crate) expected_improvement: String,
    pub(crate) selected_target: Option<String>,
    pub(crate) target_confidence: String,
    pub(crate) allowed_change_kind: String,
    pub(crate) allowed_tool_category: AllowedToolCategory,
    pub(crate) disallowed_actions: Vec<String>,
    pub(crate) must_preserve: Vec<String>,
    pub(crate) success_check: String,
    pub(crate) rerun_authority: Vec<String>,
    pub(crate) rejection_reason: Option<String>,
    pub(crate) explicit_stop_reason: Option<String>,
    pub(crate) action_envelope_status: ActionEnvelopeStatus,
}

impl RepairBrief {
    pub(crate) fn render_lines(&self) -> Vec<String> {
        let stop = self
            .explicit_stop_reason
            .as_deref()
            .map(|reason| format!(" explicit_stop_reason={}", compact(reason)))
            .unwrap_or_default();
        let rejection = self
            .rejection_reason
            .as_deref()
            .map(|reason| format!(" rejection={}", compact(reason)))
            .unwrap_or_default();
        vec![
            format!(
                "status={} active_job={} owner={} action={} cluster={} selected_target={} target_confidence={} allowed_change={} tool_category={} success_check={} action_envelope_status={}{}{}",
                self.status.as_str(),
                compact(&self.active_job),
                compact(&self.recovery_owner),
                compact(&self.repair_action),
                compact(&self.selected_failure_cluster),
                self.selected_target.as_deref().unwrap_or("none"),
                compact(&self.target_confidence),
                compact(&self.allowed_change_kind),
                self.allowed_tool_category.as_str(),
                compact(&self.success_check),
                self.action_envelope_status.as_str(),
                rejection,
                stop
            ),
            format!(
                "root_cause={} hypothesis={} expected_improvement={}",
                compact(&self.root_cause),
                compact(&self.repair_hypothesis),
                compact(&self.expected_improvement)
            ),
            format!(
                "must_preserve={} disallowed_actions={} rerun_authority={}",
                render_list(&self.must_preserve),
                render_list(&self.disallowed_actions),
                render_list(&self.rerun_authority)
            ),
        ]
    }

    pub(crate) fn eval_report_fields(&self) -> Vec<String> {
        vec![
            format!("repair_brief_status={}", self.status.as_str()),
            format!(
                "selected_failure_cluster={}",
                compact(&self.selected_failure_cluster)
            ),
            format!(
                "selected_target={}",
                self.selected_target.as_deref().unwrap_or("none")
            ),
            format!(
                "action_envelope_status={}",
                self.action_envelope_status.as_str()
            ),
            format!("allowed_change_kind={}", compact(&self.allowed_change_kind)),
            format!(
                "allowed_tool_category={}",
                self.allowed_tool_category.as_str()
            ),
            format!("repair_root_cause={}", compact(&self.root_cause)),
            format!("repair_hypothesis={}", compact(&self.repair_hypothesis)),
            format!(
                "expected_improvement={}",
                compact(&self.expected_improvement)
            ),
            format!("target_confidence={}", compact(&self.target_confidence)),
            format!("must_preserve={}", render_list(&self.must_preserve)),
            format!(
                "disallowed_actions={}",
                render_list(&self.disallowed_actions)
            ),
            format!("success_check={}", compact(&self.success_check)),
            format!(
                "repair_plan_rejection_reason={}",
                self.rejection_reason
                    .as_deref()
                    .or(self.explicit_stop_reason.as_deref())
                    .map(compact)
                    .unwrap_or_else(|| "none".to_string())
            ),
        ]
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn render_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values
            .iter()
            .map(|value| compact(value).replace(' ', "_"))
            .collect::<Vec<_>>()
            .join("|")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_brief_renders_envelope() {
        let brief = RepairBrief {
            status: RepairBriefStatus::Admitted,
            active_job: "route_integration_repair".to_string(),
            recovery_owner: "route_integration".to_string(),
            repair_action: "connect_existing_artifact_to_entrypoint".to_string(),
            selected_failure_cluster: "artifact_path:nextjs_route_not_integrated".to_string(),
            root_cause: "route is not connected to generated artifact".to_string(),
            repair_hypothesis: "connect existing artifact to route".to_string(),
            expected_improvement: "profile verification passes".to_string(),
            selected_target: Some("app/page.tsx".to_string()),
            target_confidence: "deterministic".to_string(),
            allowed_change_kind: "route_or_integration_target_only".to_string(),
            allowed_tool_category: AllowedToolCategory::FileMutation,
            disallowed_actions: vec!["Do not create placeholders.".to_string()],
            must_preserve: vec!["profile verification".to_string()],
            success_check: "profile verification".to_string(),
            rerun_authority: vec!["npm run build".to_string()],
            rejection_reason: None,
            explicit_stop_reason: None,
            action_envelope_status: ActionEnvelopeStatus::Admitted,
        };

        let rendered = brief.render_lines().join("\n");

        assert!(rendered.contains("status=admitted"));
        assert!(rendered.contains("selected_target=app/page.tsx"));
        assert!(rendered.contains("action_envelope_status=admitted"));
        assert!(rendered.contains("root_cause=route is not connected"));
        assert!(
            brief
                .eval_report_fields()
                .iter()
                .any(|field| field == "allowed_tool_category=file_mutation")
        );
    }
}
