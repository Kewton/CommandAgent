//! Completion groups follow admitted plan order and registered verifier inputs.
use super::*;
use std::path::Component;

pub(super) struct Group {
    pub(super) owners: Vec<String>,
    pub(super) remaining: Vec<String>,
    pub(super) boundary: String,
}

pub(super) fn normalized(path: &str) -> Option<String> {
    let path = Path::new(path);
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => parts.push(part.to_str()?),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

pub(super) fn group(
    config: &Config,
    plan: &StepPlan,
    step: &PlanStep,
    obligation: &Obligation,
    contract: &CompletionContract,
) -> anyhow::Result<Option<Group>> {
    let commands = contract.verify_commands.join("\n");
    let producer = |s: &PlanStep| {
        s.step_kind() == StepKind::Implement
            && !s.expected_paths.is_empty()
            && s.expected_paths.iter().all(|p| {
                normalized(p).is_some_and(|p| {
                    !obligation.sources.contains_key(&p)
                        && commands.contains(&p)
                        && contract
                            .required_paths
                            .iter()
                            .any(|required| normalized(required).as_ref() == Some(&p))
                })
            })
    };
    if producer(step) {
        return Ok(None);
    }
    let index = plan
        .steps
        .iter()
        .position(|s| s.id == step.id)
        .context("Recovery owner missing from executing plan")?;
    // An omitted or differently spelled source path cannot opt an application
    // Implement step out. Only a proven dedicated registered producer is exempt.
    let owner_indices: Vec<_> = plan
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.step_kind() == StepKind::Implement && !producer(s))
        .map(|(i, _)| i)
        .collect();
    let mut boundary_index = owner_indices.last().copied().unwrap_or(index).max(index);
    let dependencies: Vec<_> = plan
        .steps
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            *i > index
                && producer(s)
                && s.expected_paths.iter().any(|p| {
                    normalized(p).is_some_and(|p| !config.workspace_root.join(p).is_file())
                })
        })
        .map(|(i, _)| i)
        .collect();
    if let Some(last_dependency) = dependencies.last() {
        boundary_index = plan
            .steps
            .iter()
            .enumerate()
            .find(|(i, s)| {
                i > last_dependency
                    && s.step_kind() == StepKind::Verify
                    && contract
                        .verify_commands
                        .iter()
                        .all(|command| s.verify.contains(command))
            })
            .map(|(i, _)| i)
            .context(
                "Recovery verifier producer needs a finite registered confirmation boundary",
            )?;
    }
    let remaining = plan
        .steps
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            *i > index
                && *i <= boundary_index
                && (owner_indices.contains(i) || dependencies.contains(i) || *i == boundary_index)
        })
        .map(|(_, s)| s.id.clone())
        .collect();
    Ok(Some(Group {
        owners: owner_indices
            .iter()
            .map(|i| plan.steps[*i].id.clone())
            .collect(),
        remaining,
        boundary: plan.steps[boundary_index].id.clone(),
    }))
}
