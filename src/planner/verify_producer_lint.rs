//! A Verify or Implement step must not run a local script that nothing produces.
//!
//! The script has to exist in the workspace already, or be named in the
//! `expected_paths` of the step itself or of an earlier non-Verify step.
use std::collections::BTreeSet;
use std::path::Path;

use crate::config::Config;
use crate::minimal_loop::completion::CompletionContract;
use crate::planner::lint::{PlanLintReport, lint_exempting};
use crate::planner::recovery_contract_authority::{local_script_path, verifier_obligations};
use crate::planner::step_plan::{StepKind, StepPlan};

/// Returns one message per verify command that runs an unproduced local script.
/// Without a workspace root the existing files are unknown, so nothing is flagged.
/// Commands in `declared` are obligations of the host or the user (the completion
/// contract), not planner proposals, so their scripts are another gate's concern.
pub(crate) fn diagnose_script_without_producer(
    plan: &StepPlan,
    work_root: Option<&Path>,
    declared: &[String],
) -> Vec<String> {
    let Some(work_root) = work_root else {
        return Vec::new();
    };
    let mut offenses = Vec::new();
    let mut produced = BTreeSet::new();
    for (index, step) in plan.steps.iter().enumerate() {
        let kind = step.step_kind();
        let produces = kind != StepKind::Verify;
        if produces {
            produced.extend(
                step.expected_paths
                    .iter()
                    .filter_map(|path| verifier_obligations::normalized(path).ok()),
            );
        }
        if !matches!(kind, StepKind::Verify | StepKind::Implement) {
            continue;
        }
        for command in &step.verify {
            if declared.contains(command) {
                continue;
            }
            let Some(script) = local_script_path(command) else {
                continue;
            };
            if produced.contains(&script) || work_root.join(&script).exists() {
                continue;
            }
            let later = plan.steps[index + 1..].iter().any(|later| {
                later.step_kind() != StepKind::Verify
                    && later.expected_paths.iter().any(|path| {
                        verifier_obligations::normalized(path).is_ok_and(|path| path == script)
                    })
            });
            let hint = if later {
                "it is only produced by a later step; move that step before this one"
            } else {
                "no step produces it and it is not in the workspace; add an earlier step \
                 that creates it and lists it in expected_paths, or verify with an existing file"
            };
            offenses.push(format!(
                "verify runs local script {script} but {hint}: step {} command `{command}`",
                step.id
            ));
        }
    }
    offenses
}

/// Lint for a host-synthesized plan: the commands the completion contract names are
/// the host's or the user's obligations, so only planner-introduced scripts count.
pub(crate) fn lint_step_plan_report_for_host(plan: &StepPlan, config: &Config) -> PlanLintReport {
    let declared = contract_commands(config);
    lint_exempting(plan, Some(&config.workspace_root), &declared)
}

/// Commands the completion contract itself names; a host-synthesized plan may
/// run them even when their scripts are not in the workspace yet.
fn contract_commands(config: &Config) -> Vec<String> {
    let Ok(Some(contract)) = CompletionContract::load_for_config(config) else {
        return Vec::new();
    };
    let mut commands = contract.verify_commands;
    commands.extend(contract.fix_reproducer_command);
    commands.extend(contract.deterministic_oracles);
    commands.extend(
        contract
            .deferred_verify_requirements
            .into_iter()
            .map(|requirement| requirement.command),
    );
    commands
}

#[cfg(test)]
pub(crate) mod tests;
