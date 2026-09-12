//! Host-owned repair evidence, independent of intent and tool-call progress.
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::minimal_loop::loop_run::{RunSessionOptions, RunSessionStepKind};
use crate::planner::step_plan::{PlanStep, StepKind, StepPlan};
use crate::planner::verify::VerificationReport;
use crate::tools::path_guard::resolve_existing;

pub(crate) mod authority;
mod boundary;
mod configuration;
mod preservation;
#[cfg(test)]
pub(crate) mod tests;

const RECORD: &str = ".commandagent/recovery-runtime/repair-obligation.json";
pub(crate) type RepairObligation = Option<Arc<BoundRepair>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Obligation {
    attempt_id: String,
    contract_sha256: String,
    diagnostics: String,
    targets: Vec<String>,
    sources: BTreeMap<String, String>,
    frozen: BTreeMap<String, String>,
}

/// Called only by the transaction's host provenance binder. Continuations keep
/// the original failure and preservation baseline, never a model's new baseline.
pub(super) fn bind(
    source: &Config,
    treatment: &Config,
    targets: &[String],
    diagnostics: &str,
    contract: &CompletionContract,
    inherited: Option<&Obligation>,
) -> anyhow::Result<Option<Obligation>> {
    let stored = load(source)?;
    let mut obligation = if let Some(inherited) = inherited.or(stored.as_ref()) {
        let mut inherited = inherited.clone();
        inherited
            .diagnostics
            .push_str(&format!("\nContinuation evidence:\n{diagnostics}"));
        inherited
    } else {
        let mut target_contract = contract.clone();
        target_contract.required_paths.clear();
        let paths = super::recovery_inspection::related_paths(
            &treatment.workspace_root.canonicalize()?,
            targets,
            &target_contract,
        );
        let sources = paths
            .into_iter()
            .filter(|p| preservation::is_source(p))
            .map(|p| read_source(&treatment.workspace_root, &p).map(|text| (p, text)))
            .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
        if sources.is_empty() {
            return Ok(None);
        }
        Obligation {
            attempt_id: String::new(),
            contract_sha256: contract_hash(treatment)?,
            diagnostics: diagnostics.to_string(),
            targets: targets
                .iter()
                .filter_map(|p| boundary::normalized(p))
                .collect(),
            frozen: preservation::freeze_verifiers(treatment, contract, &sources)?,
            sources,
        }
    };
    ensure!(
        obligation.contract_sha256 == contract_hash(treatment)?,
        "Recovery repair contract changed"
    );
    obligation.attempt_id = uuid::Uuid::now_v7().to_string();
    super::recovery_contract_binding::write_read_only_bytes(
        &treatment.workspace_root.join(RECORD),
        &serde_json::to_vec_pretty(&obligation)?,
        "Recovery repair obligation",
    )?;
    authority::remember(treatment, &obligation)?;
    Ok(Some(obligation))
}

fn load(config: &Config) -> anyhow::Result<Option<Obligation>> {
    let expected = authority::expected(config)?;
    let path = config.workspace_root.join(RECORD);
    if !path.try_exists()? {
        ensure!(
            expected.is_none(),
            "Recovery repair host record missing for bound attempt"
        );
        return Ok(None);
    }
    ensure!(
        path.canonicalize()? == config.workspace_root.canonicalize()?.join(RECORD),
        "Recovery repair obligation redirected"
    );
    let obligation: Obligation = serde_json::from_slice(&std::fs::read(path)?)?;
    ensure!(
        expected.as_ref() == Some(&obligation),
        "Recovery repair record differs from host attempt provenance"
    );
    ensure!(
        obligation.contract_sha256 == contract_hash(config)?,
        "Recovery repair contract changed"
    );
    Ok(Some(obligation))
}

fn contract_hash(config: &Config) -> anyhow::Result<String> {
    let path = CompletionContract::configured_path_for_config(config)?
        .context("Recovery repair contract missing")?;
    Ok(format!("{:x}", Sha256::digest(std::fs::read(path)?)))
}

fn read_source(root: &Path, path: &str) -> anyhow::Result<String> {
    Ok(std::fs::read_to_string(resolve_existing(root, path)?)?)
}

#[derive(Debug)]
pub(crate) struct BoundRepair {
    obligation: Obligation,
    contract: CompletionContract,
    initial: BTreeMap<String, String>,
    owners: Vec<String>,
    remaining_owners: Vec<String>,
    boundary: String,
    last_pending: Mutex<bool>,
}

/// Bind to the admitted executing plan, not to instructions or model testimony.
pub(crate) fn configure(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    options: &mut RunSessionOptions,
) -> anyhow::Result<()> {
    if !matches!(step.step_kind(), StepKind::Implement | StepKind::Verify) {
        return Ok(());
    }
    let Some(obligation) = load(config)? else {
        return Ok(());
    };
    let contract =
        CompletionContract::load_for_config(config)?.context("Recovery repair contract missing")?;
    let Some(group) = boundary::group(config, plan, step, &obligation, &contract)? else {
        return Ok(());
    };
    let initial = obligation
        .sources
        .keys()
        .map(|p| read_source(&config.workspace_root, p).map(|text| (p.clone(), text)))
        .collect::<anyhow::Result<_>>()?;
    options.recovery_obligation = Some(Arc::new(BoundRepair {
        obligation,
        contract,
        initial,
        owners: group.owners,
        remaining_owners: group.remaining,
        boundary: group.boundary,
        last_pending: Mutex::new(false),
    }));
    if step.step_kind() == StepKind::Implement {
        options.completion_contract_verification =
            crate::minimal_loop::loop_run::CompletionContractVerification::DisabledDuringStep;
        options.completion_contract_path_merge =
            crate::minimal_loop::loop_run::CompletionContractPathMerge::Disabled;
    }
    Ok(())
}

impl BoundRepair {
    fn check(
        &self,
        config: &Config,
        options: &RunSessionOptions,
    ) -> anyhow::Result<(VerificationReport, bool)> {
        *self.last_pending.lock().unwrap() = false;
        ensure!(
            contract_hash(config)? == self.obligation.contract_sha256,
            "Recovery repair contract changed"
        );
        ensure!(
            load(config)?.as_ref() == Some(&self.obligation),
            "Recovery repair attempt changed during step"
        );
        preservation::verify(config, &self.obligation)?;
        let hash = self.source_hash(config)?;
        // Do not cache confirmation: registered checks can read generated JSON,
        // runtime inputs or environment state outside the source fingerprint.
        let changed = self.initial.iter().any(|(p, before)| {
            read_source(&config.workspace_root, p).is_ok_and(|after| after != *before)
        });
        let pending = changed
            && !self.remaining_owners.is_empty()
            && options.step_kind == Some(RunSessionStepKind::Implement);
        let report = if pending {
            VerificationReport::pass()
        } else {
            ensure!(
                self.contract.verify_commands.iter().any(|command| {
                    !crate::minimal_loop::evidence::is_artifact_only_verify_command(command)
                }),
                "Recovery repair has no registered target confirmation beyond path existence"
            );
            // Full contract confirmation at the related group's boundary. This
            // also admits fresh evidence for a repair completed by a prior step.
            self.contract
                .verify_with_goal_observed_with_setup_authority(
                    &config.workspace_root,
                    self.contract.goal.as_deref().unwrap_or_default(),
                    options.dependency_setup_authority,
                    config.offline,
                )
                .0
        };
        let after = self.source_hash(config)?;
        ensure!(
            load(config)?.as_ref() == Some(&self.obligation),
            "Recovery repair attempt changed during confirmation"
        );
        preservation::verify(config, &self.obligation)?;
        ensure!(
            hash == after,
            "Recovery repair confirmation mutated source inputs"
        );
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event":"recovery_repair_obligation_observed",
                "attempt_id":self.obligation.attempt_id,
                "contract_sha256":self.obligation.contract_sha256,
                "step_id":options.step_id,
                "phase_scope":options.phase_scope,
                "source_sha256":hash,
                "diagnostics":self.obligation.diagnostics,
                "target_paths":self.obligation.sources.keys().collect::<Vec<_>>(),
                "owners":self.owners, "remaining_owners":self.remaining_owners,
                "confirmation_boundary":self.boundary,
                "remaining_step_budget":self.remaining_owners.len(),
                "iterations_per_step_cap":config.max_iterations,
                "status":if pending { "pending" } else if report.is_pass() { "resolved" } else { "unresolved" },
                "registered_verify_commands":self.contract.verify_commands,
                "failure_reason":report.primary_reason(),
                "command_failures":report.command_failures.iter().map(|f| json!({"command":f.command,"reason":f.reason})).collect::<Vec<_>>(),
            }),
        );
        *self.last_pending.lock().unwrap() = pending;
        Ok((report, pending))
    }

    pub(crate) fn feedback(&self, config: &Config, options: &RunSessionOptions) -> Option<String> {
        let reason = match self.check(config, options) {
            Ok((report, _)) if report.is_pass() => return None,
            Ok((report, _)) => format!(
                "{}\n{}",
                report.primary_reason(),
                report
                    .command_failures
                    .iter()
                    .map(|f| f.reason.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            Err(error) => error.to_string(),
        };
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event":"step_completion_blocked", "reason":"recovery_repair_unresolved",
                "step_id":options.step_id, "phase_scope":options.phase_scope,
                "attempt_id":self.obligation.attempt_id, "failure_reason":reason,
                "original_failure_evidence":self.obligation.diagnostics,
            }),
        );
        Some(format!(
            "Recovery repair remains unresolved: {reason}\nOriginal failure: {}\nRepair the target relationship in {} (including related definitions), then confirm it with the registered checks. Reads, no-op writes and unrelated changes do not resolve it. Remaining owners: {:?}; confirmation boundary: {}.",
            self.obligation.diagnostics,
            self.obligation
                .sources
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            self.remaining_owners,
            self.boundary
        ))
    }

    pub(crate) fn pending(&self) -> bool {
        *self.last_pending.lock().unwrap()
    }

    fn source_hash(&self, config: &Config) -> anyhow::Result<String> {
        let policy = super::recovery_observation_policy::RecoveryObservationPolicy::for_contract_at_workspace(&self.contract, &config.workspace_root);
        super::recovery_snapshot::current_preflight_source_sha256(
            &config.workspace_root,
            &policy.allowed_generated_paths,
        )
    }
}
