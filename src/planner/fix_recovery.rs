//! Continuation of the original fix-intent acceptance contract inside Recovery.

use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

use crate::config::Config;
use crate::eval_events;
use crate::planner::adjudication::contract::{
    EvidenceStage, ExpectedOutcome, FIX_CONTRACT_REF, FIX_CONTRACT_VERSION,
};
use crate::planner::adjudication::fix::{
    BEFORE_FAILS_ID, FixAdjudication, FixAssurance, FixEvidenceBundle, ProbeOutcome,
};
use crate::planner::fix_runtime::{FixRuntime, ReproducerBinding, regression_binding_lineage};
use crate::planner::ultra_plan::UltraPlan;

#[derive(Debug, Deserialize)]
struct PersistedFixAdjudication {
    schema_version: String,
    intent: String,
    contract_version: String,
    contract_ref: String,
    run_id: String,
    adjudication: FixAdjudication,
    evidence: FixEvidenceBundle,
}

pub(crate) fn resume(
    current: Option<FixRuntime>,
    plan: &UltraPlan,
    config: &Config,
) -> anyhow::Result<Option<FixRuntime>> {
    match current {
        Some(runtime) => Ok(Some(runtime)),
        None => resume_fix(plan, config),
    }
}

fn resume_fix(plan: &UltraPlan, config: &Config) -> anyhow::Result<Option<FixRuntime>> {
    if plan.intent != "recover" {
        return Ok(None);
    }
    let Some(origin) = crate::planner::recovery_contract_binding::load_fix_origin(config)? else {
        return Ok(None);
    };
    let bytes = std::fs::read(config.workspace_root.join(&origin.evidence_path))
        .context("read fix evidence for Recovery continuation")?;
    let persisted: PersistedFixAdjudication =
        serde_json::from_slice(&bytes).context("parse fix evidence for Recovery continuation")?;
    if persisted.schema_version != "1"
        || persisted.intent != "fix"
        || persisted.contract_version != FIX_CONTRACT_VERSION
        || persisted.contract_ref != FIX_CONTRACT_REF
        || persisted.run_id != origin.fix_run_id
        || persisted.evidence.run_id != origin.fix_run_id
        || persisted.adjudication.assurance != FixAssurance::Failed
    {
        anyhow::bail!("Recovery fix evidence contract identity changed");
    }
    let before = persisted
        .evidence
        .before
        .context("Recovery fix evidence is missing the before observation")?;
    if before.run_id != origin.fix_run_id
        || before.requirement_id != BEFORE_FAILS_ID
        || before.stage != EvidenceStage::Before
        || before.expected != ExpectedOutcome::Failure
        || before.outcome != ProbeOutcome::Failure
        || before.binding_id != origin.reproducer_command
        || before.lineage.is_empty()
    {
        anyhow::bail!("Recovery fix before observation is not admissible");
    }
    let regression_contract = crate::planner::fix_regression_contract::resolve(plan, config);
    let regression_source = regression_contract.source;
    let omitted_supplemental_ids = regression_contract.omitted_supplemental_ids;
    let regression_bindings = regression_contract.bindings;
    let regression_ids = regression_bindings
        .iter()
        .map(|binding| binding.id.clone())
        .collect::<Vec<_>>();
    let regression_lineages = regression_bindings
        .iter()
        .map(|binding| (binding.id.clone(), regression_binding_lineage(binding)))
        .collect::<std::collections::BTreeMap<_, _>>();
    if regression_ids != persisted.evidence.bound_regression_ids
        || regression_lineages != persisted.evidence.bound_regression_lineages
    {
        anyhow::bail!("Recovery fix regression binding changed");
    }
    let reproducer = ReproducerBinding {
        command: before.binding_id.clone(),
        lineage: before.lineage.clone(),
    };
    let contract_predicate =
        crate::planner::fix_contract_predicate::FixContractPredicateContext::from_failed_reproducer(
            &config.workspace_root,
            &plan.profile,
            &reproducer.command,
            config.eval_events_path.as_deref(),
        );
    eval_events::emit(
        config.eval_events_path.as_deref(),
        json!({
            "event": "recovery_fix_contract_resumed",
            "original_intent": origin.original_intent,
            "contract_origin": origin.contract_origin,
            "contract_version": origin.contract_version,
            "contract_ref": origin.contract_ref,
            "fix_run_id": origin.fix_run_id,
            "reproducer_command": origin.reproducer_command,
            "before_epoch": before.epoch,
            "origin_evidence_path": origin.evidence_path,
            "origin_evidence_sha256": origin.evidence_sha256,
            "source": "host_owned_recovery_fix_origin",
            "regression_source": regression_source,
            "registered_regression_count": regression_ids.len(),
            "bound_regression_ids": regression_ids,
            "omitted_supplemental_ids": omitted_supplemental_ids,
            "external_oracle_used": false,
        }),
    );
    Ok(Some(FixRuntime {
        terminal_config: config.clone(),
        run_id: persisted.run_id,
        profile: plan.profile.clone(),
        goal: plan.goal.clone(),
        regression_bindings,
        reproducer: Some(reproducer),
        before: Some(before.clone()),
        after: None,
        regressions: Vec::new(),
        contract_predicate,
        diagnostic: None,
        data_role_policy: crate::planner::fix_runtime::data_role::DataRolePolicy::for_plan(plan),
        epoch: before.epoch,
        fix_written: false,
        terminalized: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::planner::step_plan::{PlanStep, StepPlan};
    use crate::planner::ultra_plan::UltraPhase;
    use clap::Parser;
    use sha2::{Digest, Sha256};
    use std::path::Path;

    fn config(root: &Path) -> Config {
        let cwd = root.to_string_lossy().to_string();
        let mut config = Config::from_cli(Cli::parse_from([
            "commandagent",
            "--cwd",
            &cwd,
            "--offline",
            "--profile",
            "generic",
            "--ultra-plan",
            "fix missing marker",
        ]))
        .unwrap();
        config.eval_events_path = Some(root.join("events.jsonl"));
        config
    }

    fn config_with_completion_contract(root: &Path) -> Config {
        let path = root.join("completion-contract.json");
        std::fs::write(
            &path,
            serde_json::to_vec(&json!({
                "verify_commands": [
                    "test -f fixed.marker",
                    "test -f stable.marker"
                ],
                "fix_reproducer_command": "test -f fixed.marker",
                "profile": "generic"
            }))
            .unwrap(),
        )
        .unwrap();
        let mut config = config(root);
        config.completion_contract_path = Some(path);
        config
    }

    fn fix_plan() -> UltraPlan {
        UltraPlan {
            goal: "fix missing marker".to_string(),
            profile: "generic".to_string(),
            style: "default".to_string(),
            intent: "fix".to_string(),
            phases: vec![UltraPhase {
                id: "reproducer-before".to_string(),
                prompt: "Bind the deterministic reproducer".to_string(),
            }],
        }
    }

    fn reproducer_plan() -> StepPlan {
        StepPlan {
            goal: "reproduce".to_string(),
            steps: vec![PlanStep {
                id: "reproduce-before".to_string(),
                kind: "verify".to_string(),
                expected_result: "fail".to_string(),
                instruction: "Run the deterministic reproducer".to_string(),
                expected_paths: Vec::new(),
                verify: vec!["test -f fixed.marker".to_string()],
            }],
        }
    }

    #[test]
    fn resumes_the_same_fix_run_and_reuses_before_lineage() {
        let control = tempfile::tempdir().unwrap();
        let control_config = config_with_completion_contract(control.path());
        let fix_plan = fix_plan();
        let mut initial = FixRuntime::for_plan(&fix_plan, &control_config).unwrap();
        initial
            .run_before_phase(
                &reproducer_plan(),
                &control_config,
                &fix_plan,
                &fix_plan.phases[0],
                0,
            )
            .unwrap();
        let run_id = initial.run_id.clone();
        drop(initial);

        let treatment = tempfile::tempdir().unwrap();
        std::fs::write(treatment.path().join("stable.marker"), "stable\n").unwrap();
        std::fs::create_dir_all(treatment.path().join("evidence")).unwrap();
        std::fs::create_dir_all(treatment.path().join(".commandagent/recovery-runtime")).unwrap();
        let evidence_path = format!("evidence/fix-{run_id}-adjudication.json");
        let evidence = std::fs::read(control.path().join(&evidence_path)).unwrap();
        std::fs::write(treatment.path().join(&evidence_path), &evidence).unwrap();
        let origin = crate::planner::recovery_contract_binding::RecoveryFixOrigin {
            schema_version: "1".to_string(),
            original_intent: "fix".to_string(),
            contract_origin: crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN.to_string(),
            contract_version: FIX_CONTRACT_VERSION.to_string(),
            contract_ref: FIX_CONTRACT_REF.to_string(),
            fix_run_id: run_id.clone(),
            evidence_path: ".commandagent/recovery-runtime/fix-origin-evidence.json".to_string(),
            evidence_sha256: format!("{:x}", Sha256::digest(&evidence)),
            reproducer_command: "test -f fixed.marker".to_string(),
        };
        std::fs::write(
            treatment
                .path()
                .join(".commandagent/recovery-runtime/fix-origin-evidence.json"),
            &evidence,
        )
        .unwrap();
        std::fs::write(
            treatment
                .path()
                .join(".commandagent/recovery-runtime/fix-origin.json"),
            serde_json::to_vec(&origin).unwrap(),
        )
        .unwrap();
        let recovery_plan = UltraPlan {
            goal: fix_plan.goal.clone(),
            profile: fix_plan.profile.clone(),
            style: fix_plan.style.clone(),
            intent: "recover".to_string(),
            phases: vec![UltraPhase {
                id: "verify-recovery".to_string(),
                prompt: "Verify the repair".to_string(),
            }],
        };
        let recovery_config = config_with_completion_contract(treatment.path());
        let resumed = resume(None, &recovery_plan, &recovery_config)
            .unwrap()
            .unwrap();
        assert!(!resumed.is_before_phase(0));
        let resumed = resume(Some(resumed), &recovery_plan, &recovery_config)
            .unwrap()
            .unwrap();
        assert_eq!(resumed.run_id, run_id);
        assert_eq!(resumed.before.as_ref().unwrap().epoch, 1);

        std::fs::write(treatment.path().join("fixed.marker"), "fixed\n").unwrap();
        resumed.finish(&recovery_config, &recovery_plan).unwrap();

        let adjudication = std::fs::read_to_string(
            treatment
                .path()
                .join(format!("evidence/fix-{run_id}-adjudication.json")),
        )
        .unwrap();
        assert!(adjudication.contains("\"after_passes\": \"passed\""));
        assert!(adjudication.contains("\"before_fails\": \"passed\""));
        let events = std::fs::read_to_string(recovery_config.eval_events_path.unwrap()).unwrap();
        assert!(
            events.contains("\"event\":\"recovery_fix_contract_resumed\""),
            "{events}"
        );
        assert!(events.contains("\"contract_origin\":\"fix_intent_v0\""));
        assert!(events.contains("\"regression_source\":\"completion_contract\""));
        assert!(events.contains("\"bound_regression_ids\":[\"completion_contract_verify_2\"]"));
        assert_eq!(events.matches("recovery_fix_contract_resumed").count(), 1);
    }
}
