//! Bounded repair action plans.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepairActionStatus {
    Admitted,
    Rejected,
    ExplicitStop,
}

impl RepairActionStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Rejected => "rejected",
            Self::ExplicitStop => "explicit_stop",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllowedToolCategory {
    ReadOnly,
    FileMutation,
    SetupConfigMutation,
    VerifierOwnedSetup,
    ToolProtocol,
    NoMutation,
}

impl AllowedToolCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::FileMutation => "file_mutation",
            Self::SetupConfigMutation => "setup_config_mutation",
            Self::VerifierOwnedSetup => "verifier_owned_setup",
            Self::ToolProtocol => "tool_protocol",
            Self::NoMutation => "no_mutation",
        }
    }

    pub(crate) fn from_projection(value: &str) -> Self {
        match value {
            "read_only" => Self::ReadOnly,
            "setup_config_mutation_only" => Self::SetupConfigMutation,
            "verifier_owned_setup_only" => Self::VerifierOwnedSetup,
            "tool_protocol_correction" => Self::ToolProtocol,
            "explicit_stop" => Self::NoMutation,
            _ => Self::FileMutation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepairActionPlan {
    pub(crate) status: RepairActionStatus,
    pub(crate) target_role: Option<String>,
    pub(crate) target_path: Option<String>,
    pub(crate) allowed_change_kind: String,
    pub(crate) allowed_tool_category: AllowedToolCategory,
    pub(crate) expected_delta: String,
    pub(crate) rejection_reason: Option<String>,
    pub(crate) source_of_truth: String,
}

impl RepairActionPlan {
    pub(crate) fn render_line(&self) -> String {
        let target = self
            .target_path
            .as_deref()
            .or(self.target_role.as_deref())
            .unwrap_or("none");
        let rejection = self
            .rejection_reason
            .as_deref()
            .map(|reason| format!(" rejection={}", compact(reason)))
            .unwrap_or_default();
        format!(
            "status={} target={} allowed_change={} tool_category={} expected_delta={} source_of_truth={}{}",
            self.status.as_str(),
            compact(target),
            compact(&self.allowed_change_kind),
            self.allowed_tool_category.as_str(),
            compact(&self.expected_delta),
            compact(&self.source_of_truth),
            rejection
        )
    }
}

fn compact(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_action_plan_renders_tool_category() {
        let plan = RepairActionPlan {
            status: RepairActionStatus::Admitted,
            target_role: Some("entrypoint".to_string()),
            target_path: Some("app/page.tsx".to_string()),
            allowed_change_kind: "route_or_integration_target_only".to_string(),
            allowed_tool_category: AllowedToolCategory::FileMutation,
            expected_delta: "route imports artifact".to_string(),
            rejection_reason: None,
            source_of_truth: "profile_contract".to_string(),
        };

        assert!(plan.render_line().contains("tool_category=file_mutation"));
        assert!(plan.render_line().contains("target=app/page.tsx"));
    }
}
