//! Close only explicitly separated verifier producers from an admitted plan.
use super::*;
use crate::planner::step_plan::{ExpectedResult, PlanStep, StepKind, StepPlan};
use std::collections::BTreeSet;

pub(crate) fn scripts(commands: &[String]) -> BTreeSet<String> {
    commands
        .iter()
        .flat_map(|c| {
            crate::planner::verify::normalize_planner_verify_command(c).unwrap_or_default()
        })
        .filter_map(|c| crate::planner::recovery_contract_authority::local_script_path(&c))
        .collect()
}

pub(crate) fn commands(plan: &StepPlan) -> Vec<String> {
    plan.steps
        .iter()
        .filter(|s| {
            s.expected_result_kind() == ExpectedResult::Pass
                && matches!(s.step_kind(), StepKind::Implement | StepKind::Verify)
        })
        .flat_map(|s| s.verify.clone())
        .collect()
}

pub(crate) fn admitted_commands(
    config: &Config,
    contract: &CompletionContract,
    plan: &StepPlan,
) -> anyhow::Result<Vec<String>> {
    admitted_final_success_commands(
        contract.profile.as_deref().unwrap_or(&config.profile),
        contract.goal.as_deref().unwrap_or(&plan.goal),
        &commands(plan),
    )
}

pub(crate) fn register(config: &Config, plan: &StepPlan) -> anyhow::Result<()> {
    let commands = commands(plan);
    if CompletionContract::configured_path_for_config(config)?.is_some()
        || !provenance::owns_run_contract(&generated_path(config, ULTRA_RUN_CONTRACT))
    {
        return register_step_plan_commands(config, &commands);
    }
    let Some(mut contract) = load_for_handoff(config)? else {
        return Ok(());
    };
    if !admits_generated_commands(contract.profile.as_deref().unwrap_or_default()) {
        return Ok(());
    }
    let original_contract = contract.clone();
    contract
        .verify_commands
        .extend(admitted_commands(config, &contract, plan)?);
    let scripts = scripts(&contract.verify_commands);
    let mut registered = provenance::verifier_steps(config);
    for step in &plan.steps {
        if step.step_kind() != StepKind::Implement
            || !step
                .expected_paths
                .iter()
                .any(|p| normalized(p).is_ok_and(|p| scripts.contains(&p)))
        {
            continue;
        }
        validate_producer(config, &contract, step)?;
        if registered.contains(step) {
            continue;
        }
        anyhow::ensure!(
            !registered.iter().any(|old| overlaps(old, step)),
            "conflicting registered verifier creation obligations"
        );
        registered.push(step.clone());
    }
    let path = generated_path(config, ULTRA_RUN_CONTRACT);
    contract
        .required_paths
        .extend(plan.steps.iter().flat_map(|s| s.expected_paths.clone()));
    let contract = contract.validate(&config.workspace_root)?;
    if original_contract != contract {
        persist_generated_commands(config, &path, &contract, "admitted_step_plan")?;
    }
    provenance::set_verifier_steps(config, registered);
    Ok(())
}

pub(crate) fn generated(config: &Config) -> Vec<PlanStep> {
    if CompletionContract::configured_path_for_config(config)
        .is_ok_and(|p| p.is_some_and(|p| !provenance::owns_run_contract(&p)))
    {
        return Vec::new();
    }
    provenance::verifier_steps(config)
}

pub(crate) fn normalized(path: &str) -> anyhow::Result<String> {
    crate::tools::path_guard::validate_workspace_relative(path)?;
    let normalized = std::path::Path::new(path)
        .components()
        .filter(|p| !matches!(p, std::path::Component::CurDir))
        .map(|p| p.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    anyhow::ensure!(!normalized.is_empty(), "output path is workspace root");
    Ok(normalized)
}

pub(crate) fn overlaps(a: &PlanStep, b: &PlanStep) -> bool {
    a.expected_paths.iter().any(|a| {
        b.expected_paths.iter().any(|b| {
            normalized(a)
                .ok()
                .zip(normalized(b).ok())
                .is_some_and(|(a, b)| a == b)
        })
    })
}

pub(crate) fn checked_paths(
    config: &Config,
    contract: &CompletionContract,
    step: &PlanStep,
) -> anyhow::Result<BTreeSet<String>> {
    use crate::tools::path_guard::resolve_optional_existing;
    use crate::tools::workspace_policy::{WorkspacePolicy, ensure_tool_path_allowed};
    let root = config.workspace_root.canonicalize()?;
    let mut paths = BTreeSet::new();
    for raw in &step.expected_paths {
        let path = normalized(raw)?;
        anyhow::ensure!(
            paths.insert(path.clone()),
            "duplicate or aliased output: {raw}"
        );
        let resolved = resolve_optional_existing(&root, raw)?;
        anyhow::ensure!(
            resolved == root.join(&path),
            "redirected verifier output: {raw}"
        );
        ensure_tool_path_allowed(&root, &resolved, WorkspacePolicy::for_task_request())?;
        anyhow::ensure!(!contract.protected_paths.iter().any(|p|
            resolve_optional_existing(&root, p).is_ok_and(|p| resolved.starts_with(p))),
            "verifier creation targets protected path: {raw}");
        anyhow::ensure!(
            !std::path::Path::new(&path).components().any(|p| {
                let s = p.as_os_str().to_string_lossy();
                s.starts_with(".env")
                    || matches!(
                        s.as_ref(),
                        ".npmrc" | ".pypirc" | "credentials" | "credentials.json"
                    )
            }),
            "verifier creation targets private configuration: {raw}"
        );
        if resolved.exists() {
            anyhow::ensure!(resolved.is_file(), "verifier output is not a file: {raw}");
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                anyhow::ensure!(
                    resolved.metadata()?.nlink() == 1,
                    "hard-linked verifier output: {raw}"
                );
            }
        }
    }
    Ok(paths)
}

pub(crate) fn validate_producer(
    config: &Config,
    contract: &CompletionContract,
    step: &PlanStep,
) -> anyhow::Result<()> {
    let paths = checked_paths(config, contract, step)?;
    let scripts = scripts(&contract.verify_commands);
    anyhow::ensure!(
        step.step_kind() == StepKind::Implement
            && step.expected_result_kind() == ExpectedResult::Pass,
        "verifier producer must be Implement/pass"
    );
    anyhow::ensure!(
        !paths.is_empty() && paths.is_subset(&scripts),
        "verifier producer mixes application/configuration scope; retain all duties in separate owners: {:?}",
        step.expected_paths
    );
    for command in &step.verify {
        for command in crate::planner::verify::normalize_planner_verify_command(command)? {
            crate::planner::verify::validate_verify_command(&command)?;
        }
    }
    Ok(())
}
