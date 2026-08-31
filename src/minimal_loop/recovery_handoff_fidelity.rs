use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RecoveryHandoffFidelity {
    pub(crate) original_goal: String,
    pub(crate) verify_commands: Vec<String>,
    pub(crate) repair_targets: Vec<String>,
    pub(crate) goal_source: &'static str,
    pub(crate) contract_bound: bool,
    pub(crate) required_for_fix_recovery: bool,
}

impl RecoveryHandoffFidelity {
    pub(crate) fn resolve(
        config: &Config,
        fallback_goal: &str,
        selected_targets: &[String],
        changed_paths: &[String],
    ) -> anyhow::Result<Self> {
        let contract = CompletionContract::load_for_config(config)?;
        let required_for_fix_recovery =
            crate::planner::recovery_contract_binding::load_fix_origin(config)?.is_some();
        let contract_goal = contract
            .as_ref()
            .and_then(|contract| contract.goal.as_deref())
            .filter(|goal| !goal.trim().is_empty());
        let original_goal = contract_goal.unwrap_or(fallback_goal).to_string();
        let goal_source = if contract_goal.is_some() {
            "completion_contract"
        } else {
            "step_prompt_fallback"
        };
        let verify_commands = contract
            .as_ref()
            .map(|contract| contract.verify_commands.clone())
            .unwrap_or_default();
        let mut repair_targets = Vec::new();
        append_unique(&mut repair_targets, selected_targets);
        append_unique(&mut repair_targets, changed_paths);
        if let Some(contract) = &contract {
            append_unprotected(
                &mut repair_targets,
                &contract.required_paths,
                &contract.protected_paths,
            );
        }
        Ok(Self {
            original_goal,
            verify_commands,
            repair_targets,
            goal_source,
            contract_bound: contract.is_some(),
            required_for_fix_recovery,
        })
    }

    pub(crate) fn is_complete(&self) -> bool {
        !self.original_goal.trim().is_empty()
            && (!self.required_for_fix_recovery
                || (self.goal_source == "completion_contract"
                    && !self.verify_commands.is_empty()
                    && !self.repair_targets.is_empty()))
    }
}

fn append_unique(out: &mut Vec<String>, values: &[String]) {
    for value in values {
        let value = value.trim();
        if !value.is_empty() && !out.iter().any(|existing| existing == value) {
            out.push(value.to_string());
        }
    }
}

fn append_unprotected(out: &mut Vec<String>, values: &[String], protected: &[String]) {
    for value in values {
        let path = value.trim_matches('/');
        let is_protected = protected.iter().any(|protected| {
            let protected = protected.trim_matches('/');
            path == protected || path.starts_with(&format!("{protected}/"))
        });
        if !is_protected {
            append_unique(out, std::slice::from_ref(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_recovery_handoff_fails_closed_when_contract_fields_are_missing() {
        let incomplete = RecoveryHandoffFidelity {
            original_goal: "repair the product".to_string(),
            goal_source: "step_prompt_fallback",
            required_for_fix_recovery: true,
            ..RecoveryHandoffFidelity::default()
        };

        assert!(!incomplete.is_complete());
    }

    #[test]
    fn non_fix_handoff_can_use_a_nonempty_fallback_goal() {
        let fallback = RecoveryHandoffFidelity {
            original_goal: "finish the task".to_string(),
            goal_source: "step_prompt_fallback",
            ..RecoveryHandoffFidelity::default()
        };

        assert!(fallback.is_complete());
    }

    #[test]
    fn repair_targets_exclude_protected_paths_and_descendants() {
        let mut targets = Vec::new();
        append_unprotected(
            &mut targets,
            &[
                "pipeline/main.py".to_string(),
                "data/task.csv".to_string(),
                "tests/test_pipeline.py".to_string(),
            ],
            &["data".to_string(), "tests".to_string()],
        );

        assert_eq!(targets, ["pipeline/main.py"]);
    }
}
