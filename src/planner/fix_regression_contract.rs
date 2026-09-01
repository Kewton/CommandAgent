//! Bind fix regressions to the product-visible completion contract.

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::profile::{ProfileFixRegressionAdapter, ProfileFixRegressionBinding};
use crate::planner::ultra_plan::UltraPlan;

#[derive(Debug, Clone)]
pub(crate) struct FixRegressionContractBinding {
    pub(crate) bindings: Vec<ProfileFixRegressionBinding>,
    pub(crate) source: &'static str,
    pub(crate) omitted_supplemental_ids: Vec<String>,
}

pub(crate) fn resolve(plan: &UltraPlan, config: &Config) -> FixRegressionContractBinding {
    let supplemental = crate::planner::profile::resolve_profile_runtime(&plan.profile)
        .fix_regression_bindings(&config.workspace_root, &plan.goal);
    let Ok(Some(contract)) = CompletionContract::load_for_config(config) else {
        return supplemental_binding(supplemental);
    };
    if contract.profile.as_deref().is_some_and(|profile| {
        crate::planner::profile::domain_profile(profile).id()
            != crate::planner::profile::domain_profile(&plan.profile).id()
    }) {
        return supplemental_binding(supplemental);
    }
    bind_registered(&contract, supplemental)
}

fn supplemental_binding(
    bindings: Vec<ProfileFixRegressionBinding>,
) -> FixRegressionContractBinding {
    FixRegressionContractBinding {
        bindings,
        source: "profile_catalog",
        omitted_supplemental_ids: Vec::new(),
    }
}

fn bind_registered(
    contract: &CompletionContract,
    supplemental: Vec<ProfileFixRegressionBinding>,
) -> FixRegressionContractBinding {
    let reproducer = contract.fix_reproducer_command.as_deref();
    let bindings = contract
        .verify_commands
        .iter()
        .enumerate()
        .filter(|(_, command)| Some(command.as_str()) != reproducer)
        .map(|(index, command)| ProfileFixRegressionBinding {
            id: format!("completion_contract_verify_{}", index + 1),
            adapter: ProfileFixRegressionAdapter::VerifyCommand(command.clone()),
        })
        .collect();
    FixRegressionContractBinding {
        bindings,
        source: "completion_contract",
        omitted_supplemental_ids: supplemental.into_iter().map(|binding| binding.id).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> CompletionContract {
        CompletionContract {
            required_paths: Vec::new(),
            protected_paths: Vec::new(),
            verify_commands: vec![
                "python3 scripts/repro.py data/task-02.csv".to_string(),
                "python3 -m pytest -q tests".to_string(),
                "python3 scripts/contract_check.py".to_string(),
            ],
            fix_reproducer_command: Some("python3 scripts/repro.py data/task-02.csv".to_string()),
            profile: Some("data".to_string()),
            goal: None,
            required_capabilities: Vec::new(),
            deterministic_oracles: Vec::new(),
            required_evidence: Vec::new(),
            evidence_hint_tokens: Vec::new(),
            required_obligations: Vec::new(),
            deferred_verify_requirements: Vec::new(),
            verify_repair_cap: 1,
        }
    }

    #[test]
    fn registered_contract_omits_reproducer_and_supplemental_profile_probes() {
        let supplemental = vec![ProfileFixRegressionBinding {
            id: "pipeline_probe".to_string(),
            adapter: ProfileFixRegressionAdapter::DataManifestCheck,
        }];

        let binding = bind_registered(&contract(), supplemental);

        assert_eq!(binding.source, "completion_contract");
        assert_eq!(binding.omitted_supplemental_ids, ["pipeline_probe"]);
        assert_eq!(
            binding
                .bindings
                .iter()
                .map(|binding| binding.id.as_str())
                .collect::<Vec<_>>(),
            [
                "completion_contract_verify_2",
                "completion_contract_verify_3"
            ]
        );
        assert!(binding.bindings.iter().all(|binding| matches!(
            binding.adapter,
            ProfileFixRegressionAdapter::VerifyCommand(_)
        )));
    }
}
