//! Host-owned verification binding for StepPlans inside a Recovery UltraPlan.

use serde_json::json;

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};

const INSPECTION_PHASE_ID: &str = "inspect-current-state";
const CONTRACT_VERIFY_STEP_ID: &str = "recovery-contract-verify";

pub(crate) fn bind_generated(
    config: &Config,
    phase_id: Option<&str>,
    step_plan: &mut StepPlan,
) -> anyhow::Result<bool> {
    let Some(phase_id) = phase_id else {
        return Ok(false);
    };
    if crate::planner::recovery_contract_binding::load_fix_origin(config)?.is_none() {
        return Ok(false);
    }
    bind_contract(config, phase_id, step_plan)
}

fn bind_contract(
    config: &Config,
    phase_id: &str,
    step_plan: &mut StepPlan,
) -> anyhow::Result<bool> {
    let Some(contract) = CompletionContract::load_for_config(config)? else {
        return Ok(false);
    };
    if contract.verify_commands.is_empty() {
        return Ok(false);
    }

    let original_steps = step_plan.steps.clone();
    let original_commands = commands(&original_steps);
    let removed_step_ids = if phase_id == INSPECTION_PHASE_ID {
        bind_read_only_inspection(step_plan, &contract)
    } else {
        bind_final_success_verification(step_plan, &contract)
    };
    let bound_commands = commands(&step_plan.steps);

    crate::eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_step_plan_verify_commands_bound",
            "phase_id": phase_id,
            "binding_mode": if phase_id == INSPECTION_PHASE_ID {
                "read_only_inspection"
            } else {
                "completion_contract_final_success"
            },
            "binding_stage": "pre_lint",
            "source": "product_visible_completion_contract",
            "external_oracle_used": false,
            "original_step_count": original_steps.len(),
            "bound_step_count": step_plan.steps.len(),
            "original_verify_commands": original_commands,
            "bound_verify_commands": bound_commands,
            "registered_verify_commands": contract.verify_commands,
            "removed_step_ids": removed_step_ids,
        }),
    );
    Ok(true)
}

fn bind_read_only_inspection(plan: &mut StepPlan, contract: &CompletionContract) -> Vec<String> {
    let removed = plan.steps.iter().map(|step| step.id.clone()).collect();
    let required = contract.required_paths.join(", ");
    plan.steps = vec![PlanStep {
        id: "inspect-recovery-state".to_string(),
        kind: "inspect".to_string(),
        expected_result: "pass".to_string(),
        instruction: format!(
            "Inspect only the current workspace-relative implementation paths required by the completion contract ({required}). Do not use absolute paths, .commandagent, or .anvil paths, and do not modify the workspace."
        ),
        expected_paths: Vec::new(),
        verify: Vec::new(),
    }];
    removed
}

fn bind_final_success_verification(
    plan: &mut StepPlan,
    contract: &CompletionContract,
) -> Vec<String> {
    let mut removed = Vec::new();
    plan.steps.retain_mut(|step| {
        if step.step_kind() == StepKind::Verify {
            removed.push(step.id.clone());
            return false;
        }
        step.expected_result = "pass".to_string();
        step.verify.clear();
        true
    });
    plan.steps.push(PlanStep {
        id: unique_contract_verify_id(&plan.steps),
        kind: "verify".to_string(),
        expected_result: "pass".to_string(),
        instruction: "Run the host-registered final-success checks after the Recovery changes."
            .to_string(),
        expected_paths: contract.required_paths.clone(),
        verify: contract.verify_commands.clone(),
    });
    removed
}

fn unique_contract_verify_id(steps: &[PlanStep]) -> String {
    if !steps.iter().any(|step| step.id == CONTRACT_VERIFY_STEP_ID) {
        return CONTRACT_VERIFY_STEP_ID.to_string();
    }
    let mut suffix = 2usize;
    loop {
        let candidate = format!("{CONTRACT_VERIFY_STEP_ID}-{suffix}");
        if !steps.iter().any(|step| step.id == candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn commands(steps: &[PlanStep]) -> Vec<String> {
    steps
        .iter()
        .flat_map(|step| step.verify.iter().cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use clap::Parser;
    use sha2::{Digest, Sha256};

    use super::*;

    fn config(root: &Path) -> Config {
        let mut config =
            Config::from_cli(crate::cli::Cli::parse_from(["commandagent", "--ux-demo"])).unwrap();
        config.workspace_root = root.to_path_buf();
        config.eval_events_path = None;
        let contract = root.join("completion-contract.json");
        std::fs::write(
            &contract,
            r#"{"required_paths":["cli.py"],"verify_commands":["python3 cli.py 16","python3 -m pytest -q tests"],"profile":"cli"}"#,
        )
        .unwrap();
        config.completion_contract_path = Some(contract);
        config
    }

    fn step(id: &str, kind: &str, verify: &[&str]) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            kind: kind.to_string(),
            expected_result: "fail".to_string(),
            instruction: id.to_string(),
            expected_paths: vec!["cli.py".to_string()],
            verify: verify.iter().map(|value| value.to_string()).collect(),
        }
    }

    #[test]
    fn recovery_inspection_is_read_only_and_does_not_run_failure_checks() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let mut steps = StepPlan {
            goal: "inspect".to_string(),
            steps: vec![
                step("inspect-cli", "inspect", &[]),
                step("reproduce-failure", "verify", &["python3 cli.py 16"]),
                step("premature-repair", "implement", &[]),
            ],
        };

        bind_contract(&config, INSPECTION_PHASE_ID, &mut steps).unwrap();

        assert_eq!(steps.steps.len(), 1);
        assert_eq!(steps.steps[0].id, "inspect-recovery-state");
        assert!(steps.steps[0].verify.is_empty());
        assert!(steps.steps[0].expected_paths.is_empty());
        assert_eq!(steps.steps[0].expected_result, "pass");
        assert!(steps.steps[0].instruction.contains("workspace-relative"));
        assert!(steps.steps[0].instruction.contains("cli.py"));
    }

    #[test]
    fn recovery_repair_uses_only_the_complete_registered_command_set_at_the_end() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let mut steps = StepPlan {
            goal: "repair".to_string(),
            steps: vec![
                step("inspect-cli", "inspect", &["cat cli.py"]),
                step("repair-cli", "implement", &["test $? -eq 2"]),
                step("model-verify", "verify", &["test $? -eq 0"]),
            ],
        };

        bind_contract(&config, "repair-unknown", &mut steps).unwrap();

        assert_eq!(steps.steps.len(), 3);
        assert_eq!(steps.steps[0].id, "inspect-cli");
        assert!(steps.steps[0].verify.is_empty());
        assert_eq!(steps.steps[1].id, "repair-cli");
        assert!(steps.steps[1].verify.is_empty());
        assert_eq!(steps.steps[2].id, CONTRACT_VERIFY_STEP_ID);
        assert_eq!(
            steps.steps[2].verify,
            ["python3 cli.py 16", "python3 -m pytest -q tests"]
        );
        assert!(
            steps
                .steps
                .iter()
                .all(|step| step.expected_result == "pass")
        );
    }

    #[test]
    fn generated_plan_without_recovery_origin_is_unchanged() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let mut plan = StepPlan {
            goal: "repair".to_string(),
            steps: vec![step("model-verify", "verify", &["test $? -eq 0"])],
        };
        let original = plan.clone();

        assert!(!bind_generated(&config, Some("repair"), &mut plan).unwrap());

        assert_eq!(plan, original);
    }

    #[test]
    fn generated_recovery_plan_is_bound_before_lint_when_origin_exists() {
        let root = tempfile::tempdir().unwrap();
        let config = config(root.path());
        let runtime = root.path().join(".commandagent/recovery-runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        let evidence = b"{}\n";
        std::fs::write(runtime.join("fix-origin-evidence.json"), evidence).unwrap();
        std::fs::write(
            runtime.join("fix-origin.json"),
            serde_json::to_vec(
                &crate::planner::recovery_contract_binding::RecoveryFixOrigin {
                    schema_version: "1".to_string(),
                    original_intent: "fix".to_string(),
                    contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
                    contract_version: crate::planner::adjudication::contract::FIX_CONTRACT_VERSION
                        .to_string(),
                    contract_ref: crate::planner::adjudication::contract::FIX_CONTRACT_REF
                        .to_string(),
                    fix_run_id: "a15-a2-test".to_string(),
                    evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json"
                        .to_string(),
                    evidence_sha256: format!("{:x}", Sha256::digest(evidence)),
                    reproducer_command: "python3 cli.py 16".to_string(),
                },
            )
            .unwrap(),
        )
        .unwrap();
        let mut plan = StepPlan {
            goal: "repair".to_string(),
            steps: vec![step("model-verify", "verify", &[])],
        };

        assert!(bind_generated(&config, Some("repair-unknown"), &mut plan).unwrap());
        assert_eq!(plan.steps.last().unwrap().id, CONTRACT_VERIFY_STEP_ID);
        assert_eq!(
            plan.steps.last().unwrap().verify,
            ["python3 cli.py 16", "python3 -m pytest -q tests"]
        );
    }
}
