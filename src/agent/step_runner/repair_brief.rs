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
    pub(crate) selected_target: Option<String>,
    pub(crate) allowed_change_kind: String,
    pub(crate) allowed_tool_category: AllowedToolCategory,
    pub(crate) disallowed_actions: Vec<String>,
    pub(crate) must_preserve: Vec<String>,
    pub(crate) success_check: String,
    pub(crate) rerun_authority: Vec<String>,
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
        vec![format!(
            "status={} active_job={} owner={} action={} cluster={} selected_target={} allowed_change={} tool_category={} success_check={} action_envelope_status={}{}",
            self.status.as_str(),
            compact(&self.active_job),
            compact(&self.recovery_owner),
            compact(&self.repair_action),
            compact(&self.selected_failure_cluster),
            self.selected_target.as_deref().unwrap_or("none"),
            compact(&self.allowed_change_kind),
            self.allowed_tool_category.as_str(),
            compact(&self.success_check),
            self.action_envelope_status.as_str(),
            stop
        )]
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
        ]
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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
            selected_target: Some("app/page.tsx".to_string()),
            allowed_change_kind: "route_or_integration_target_only".to_string(),
            allowed_tool_category: AllowedToolCategory::FileMutation,
            disallowed_actions: vec!["Do not create placeholders.".to_string()],
            must_preserve: vec!["profile verification".to_string()],
            success_check: "profile verification".to_string(),
            rerun_authority: vec!["npm run build".to_string()],
            explicit_stop_reason: None,
            action_envelope_status: ActionEnvelopeStatus::Admitted,
        };

        let rendered = brief.render_lines().join("\n");

        assert!(rendered.contains("status=admitted"));
        assert!(rendered.contains("selected_target=app/page.tsx"));
        assert!(rendered.contains("action_envelope_status=admitted"));
    }
}
