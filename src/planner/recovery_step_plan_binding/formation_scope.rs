//! Provenance captured at the actual profile mutation boundary, before
//! sanitization or preset conversion can erase an implementation duty.
use std::collections::BTreeSet;

use super::*;
use crate::planner::recovery_contract_authority::verifier_obligations as scope;
use crate::planner::recovery_inspection::verifier_obligations::{FailureClass, failure};

#[derive(Default)]
pub(crate) struct InstructionProtection {
    pub(crate) indices: BTreeSet<usize>,
    pub(crate) error: Option<String>,
}

pub(crate) struct ProfileAddition {
    model: PlanStep,
    host: PlanStep,
}

impl ProfileAddition {
    pub(crate) fn new(model: PlanStep, augmented: &PlanStep, guidance: String) -> Self {
        let mut paths: Vec<_> = augmented
            .expected_paths
            .iter()
            .filter(|p| !model.expected_paths.contains(p))
            .cloned()
            .collect();
        // Without a new output, retain the original owner's output boundary.
        // Text on an unrelated owner must not discharge the host's duty.
        if paths.is_empty() {
            paths = model.expected_paths.clone();
        }
        let host = PlanStep {
            id: model.id.clone(),
            kind: augmented.kind.clone(),
            expected_result: augmented.expected_result.clone(),
            instruction: guidance,
            expected_paths: paths,
            verify: Vec::new(),
        };
        Self { model, host }
    }

    pub(crate) fn preserve(&self, plan: &StepPlan) -> anyhow::Result<()> {
        super::admission::preserve(
            &StepPlan {
                goal: plan.goal.clone(),
                steps: vec![self.host.clone()],
            },
            plan,
        )
    }

    pub(crate) fn instruction_protection(&self, plan: &StepPlan) -> InstructionProtection {
        instruction_protection(&self.model.id, &self.host.instruction, plan)
    }
}

fn instruction_protection(
    owner_id: &str,
    host_instruction: &str,
    plan: &StepPlan,
) -> InstructionProtection {
    if host_instruction.is_empty() {
        return InstructionProtection::default();
    }
    let indices = plan
        .steps
        .iter()
        .enumerate()
        .filter_map(|(index, step)| {
            (step.id == owner_id && step.instruction.contains(host_instruction)).then_some(index)
        })
        .collect::<BTreeSet<_>>();
    let error = match indices.len() {
        1 => None,
        0 => Some(format!(
            "profile instruction provenance lost host guidance owner {owner_id} before sanitization"
        )),
        count => Some(format!(
            "profile instruction provenance for owner {owner_id} is ambiguous across {count} steps"
        )),
    };
    InstructionProtection { indices, error }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct FormationScope {
    pub(super) model: StepPlan,
    pub(super) host: StepPlan,
}

impl FormationScope {
    pub(crate) fn capture(plan: &StepPlan, addition: Option<&ProfileAddition>) -> Self {
        let mut model = plan.clone();
        let mut host = StepPlan {
            goal: plan.goal.clone(),
            steps: Vec::new(),
        };
        if let Some(addition) = addition {
            if let Some(step) = model.steps.iter_mut().find(|s| s.id == addition.model.id) {
                *step = addition.model.clone();
            }
            if !addition.host.instruction.is_empty() || !addition.host.expected_paths.is_empty() {
                host.steps.push(addition.host.clone());
            }
        }
        Self { model, host }
    }

    pub(crate) fn retained_instruction_indices(&self, plan: &StepPlan) -> BTreeSet<usize> {
        self.host
            .steps
            .iter()
            .filter(|host| !host.instruction.is_empty())
            .flat_map(|host| {
                plan.steps.iter().enumerate().filter_map(|(index, step)| {
                    step.instruction
                        .contains(&host.instruction)
                        .then_some(index)
                })
            })
            .collect()
    }

    pub(crate) fn preserve(&self, original: &StepPlan, proposed: &StepPlan) -> anyhow::Result<()> {
        if let Some((model_view, host_view)) =
            super::package_owner_scope::views(&self.model, &self.host, original, proposed)
        {
            super::admission::preserve(&self.model, &model_view)?;
            super::admission::preserve(&self.host, &host_view)?;
        } else {
            super::admission::preserve(&self.model, proposed)?;
            super::admission::preserve(&self.host, proposed)?;
        }
        preserve_boundaries(original, proposed)
    }
}

fn preserve_boundaries(original: &StepPlan, proposed: &StepPlan) -> anyhow::Result<()> {
    let mut preceding_outputs = Vec::new();
    for step in &original.steps {
        // Explicit Verify steps often have no expected_paths. Retain their
        // original order relative to all preceding executable output owners.
        for command in &step.verify {
            if !proposed.steps.iter().enumerate().any(|(index, candidate)| {
                matches!(
                    candidate.step_kind(),
                    StepKind::Implement | StepKind::Verify
                ) && candidate.expected_result == step.expected_result
                    && candidate.verify.contains(command)
                    && preceding_outputs.iter().all(|path| {
                        proposed
                            .steps
                            .iter()
                            .enumerate()
                            .filter(|(_, owner)| {
                                matches!(owner.step_kind(), StepKind::Implement | StepKind::Setup)
                                    && owner
                                        .expected_paths
                                        .iter()
                                        .any(|p| scope::normalized(p).ok().as_ref() == Some(path))
                            })
                            .all(|(owner_index, _)| owner_index < index)
                    })
            }) {
                return Err(failure(
                    FailureClass::ProposalRepairable,
                    format!("formation moved original check before its output owners: {command}"),
                ));
            }
        }
        if matches!(step.step_kind(), StepKind::Implement | StepKind::Setup) {
            preceding_outputs.extend(
                step.expected_paths
                    .iter()
                    .map(|p| scope::normalized(p))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            );
        }
    }
    Ok(())
}
