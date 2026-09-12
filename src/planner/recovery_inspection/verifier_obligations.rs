//! Classify closed obligations separately from correctable proposed ownership.
use super::*;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::step_plan::{ExpectedResult, PlanStep, StepKind, StepPlan};
use serde_json::json;

pub(crate) mod authority;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureClass {
    ProposalRepairable,
    OriginalInconsistent,
    Unsafe,
}

#[derive(Debug, thiserror::Error)]
#[error("Recovery verifier binding {class:?}: {reason}")]
pub(crate) struct BindingFailure {
    pub(crate) class: FailureClass,
    pub(crate) reason: String,
}
pub(crate) fn failure(class: FailureClass, reason: impl Into<String>) -> anyhow::Error {
    BindingFailure {
        class,
        reason: reason.into(),
    }
    .into()
}

pub(crate) fn bind(
    config: &Config,
    contract: &CompletionContract,
    plan: &mut StepPlan,
) -> anyhow::Result<Vec<PlanStep>> {
    let Some(context) = load(config)? else {
        return Ok(Vec::new());
    };
    let mut bound = plan.clone();
    let mut preserved = Vec::new();
    let scripts = scope::scripts(&contract.verify_commands);
    for (index, original) in context.verifier_steps.iter().enumerate() {
        if context.verifier_steps[..index]
            .iter()
            .any(|old| scope::overlaps(old, original))
        {
            return Err(failure(
                FailureClass::OriginalInconsistent,
                "overlapping closed verifier obligations; preserve original scope and stop",
            ));
        }
    }
    // Even configured/legacy contexts without creation authority cannot assign
    // a registered verifier to arbitrary new implementation work.
    for step in &bound.steps {
        if step.step_kind() != StepKind::Verify {
            for path in &step.expected_paths {
                let normalized = scope::normalized(path)
                    .map_err(|e| failure(FailureClass::Unsafe, e.to_string()))?;
                if scripts.contains(&normalized)
                    && !context.verifier_steps.iter().any(|s| {
                        s.expected_paths
                            .iter()
                            .any(|p| scope::normalized(p).ok().as_ref() == Some(&normalized))
                    })
                {
                    return Err(failure(
                        FailureClass::Unsafe,
                        format!("unregistered/configured verifier owner {}: {path}", step.id),
                    ));
                }
            }
        }
    }
    for original in &context.verifier_steps {
        let owners: Vec<_> = bound
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.step_kind() != StepKind::Verify && scope::overlaps(s, original))
            .map(|(i, _)| i)
            .collect();
        let candidate_owners: Vec<_> = owners.iter().map(|i| bound.steps[*i].clone()).collect();
        let result = bind_one(
            config,
            contract,
            original,
            &owners,
            &mut bound,
            context.verifier_seal.is_some(),
        );
        crate::eval_events::emit(
            config.eval_events_path.as_deref(),
            json!({
                "event":"recovery_verifier_owner_binding", "original":original,
                "candidate_owners": candidate_owners,
                "contract_sha256":context.contract_sha256,
                "classification":result.as_ref().err().and_then(|e| e.downcast_ref::<BindingFailure>()).map(|e| e.class),
                "reason":result.as_ref().err().map(|e| e.to_string()),
                "status":if result.is_ok() {"bound"} else {"rejected"},
            }),
        );
        if let Some(step) = result? {
            preserved.push(step);
        }
    }
    if context.verifier_seal.is_none()
        && scripts
            .iter()
            .any(|p| !config.workspace_root.join(p).is_file())
    {
        return Err(failure(
            FailureClass::Unsafe,
            "legacy context has no host provenance for missing verifiers; original scope cannot be reduced or inferred",
        ));
    }
    *plan = bound;
    Ok(preserved)
}

fn bind_one(
    config: &Config,
    contract: &CompletionContract,
    original: &PlanStep,
    owners: &[usize],
    plan: &mut StepPlan,
    sealed: bool,
) -> anyhow::Result<Option<PlanStep>> {
    let paths = scope::checked_paths(config, contract, original)
        .map_err(|e| failure(FailureClass::Unsafe, e.to_string()))?;
    let present = paths
        .iter()
        .filter(|p| config.workspace_root.join(p).is_file())
        .count();
    if present > 0 && present < paths.len() {
        return Err(failure(
            FailureClass::OriginalInconsistent,
            format!(
                "closed producer {} partially exists ({present}/{}); preserve original scope {:?}; proposal retries cannot repair original obligations",
                original.id,
                paths.len(),
                original.expected_paths
            ),
        ));
    }
    if !sealed {
        return Err(failure(
            FailureClass::Unsafe,
            "unsealed legacy verifier obligation cannot grant creation authority; preserve the original closed scope",
        ));
    }
    scope::validate_producer(config, contract, original)
        .map_err(|e| failure(FailureClass::OriginalInconsistent, e.to_string()))?;
    if present == paths.len() {
        if !owners.is_empty() {
            return Err(failure(
                FailureClass::ProposalRepairable,
                format!(
                    "preserve existing verifier {:?}; remove its proposed write owner; final host checks remain required",
                    original.expected_paths
                ),
            ));
        }
        return Ok(None);
    }
    if owners.len() > 1 {
        return Err(failure(
            FailureClass::ProposalRepairable,
            format!(
                "multiple owners for {} {:?}; provide one dedicated Implement containing every original output",
                original.id, paths
            ),
        ));
    }
    if let [index] = owners {
        let owner = &mut plan.steps[*index];
        let outputs = scope::checked_paths(config, contract, owner)
            .map_err(|e| failure(FailureClass::Unsafe, e.to_string()))?;
        if owner.step_kind() != StepKind::Implement
            || owner.expected_result_kind() != ExpectedResult::Pass
            || outputs != paths
        {
            return Err(failure(
                FailureClass::ProposalRepairable,
                format!(
                    "owner {} kind={} outputs={outputs:?}; original {} requires one dedicated Implement/pass with all and only {paths:?}; preserve remaining application/configuration duties separately",
                    owner.id, owner.kind, original.id
                ),
            ));
        }
        // Preserve proposal identity and additional instructions, with original
        // requirements taking precedence. Original checks survive final binding.
        if !owner.instruction.contains(&original.instruction) {
            owner.instruction.push_str(&format!("\nOriginal admitted verifier requirements (preserve every assertion and failure condition):\n{}", original.instruction));
        }
        for command in &original.verify {
            if !owner.verify.contains(command) {
                owner.verify.push(command.clone());
            }
        }
        return Ok(Some(owner.clone()));
    }
    let digest = format!("{:x}", Sha256::digest(serde_json::to_vec(original)?));
    let mut restored = original.clone();
    restored.id = format!("recovery-create-verifier-{}", &digest[..24]);
    if plan.steps.iter().any(|s| s.id == restored.id) {
        return Err(failure(
            FailureClass::Unsafe,
            "proposal collides with host producer identity",
        ));
    }
    plan.steps.push(restored.clone());
    Ok(Some(restored))
}
