//! Phase StepPlan admission of the python-cli entry identity (#896).
//!
//! A create phase that already has exactly one `src/<package>/main.py` keeps
//! it: a proposal that names another package's entry as an output, a
//! verification target or an instruction target is returned to the planner
//! within the existing planner budget, as is a single entry that conflicts
//! with a file the user named in the original goal. The host never renames,
//! rewrites or moves an entry. Workspaces that already hold several entries
//! are not pinned and no winner is chosen; the final `entry_cardinality` gate
//! stays the authority after execution.

use std::collections::BTreeSet;
use std::path::Path;

use crate::config::Config;
use crate::planner::adjudication::contract::IntentId;
use crate::planner::profile::DomainProfile;
use crate::planner::recovery_inspection::verifier_obligations::{FailureClass, failure};
use crate::planner::step_plan::StepPlan;

use super::{PythonCliProfile, explicit_goal_package, safe_python_package_name};

/// Outcome of comparing a phase StepPlan with the workspace entry identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryIdentity {
    /// The proposal keeps the entry identity (or names no entry).
    Admitted,
    /// Returned to the planner within the existing budget; the reason says why.
    Repairable(String),
}

/// Admission hook: applies only to python-cli create phases outside Recovery.
pub(crate) fn admit(config: &Config, phase: Option<&str>, plan: &StepPlan) -> anyhow::Result<()> {
    if phase.is_none()
        || crate::planner::profile::canonical_profile_name(&config.profile)
            != crate::planner::profile_descriptor::PYTHON_CLI_PROFILE_ID
        || config.resolved_run_intent() != IntentId::Create
        || crate::planner::recovery_inspection::has_origin(config)?
    {
        return Ok(());
    }
    match assess(&config.workspace_root, plan) {
        EntryIdentity::Admitted => Ok(()),
        EntryIdentity::Repairable(reason) => Err(failure(FailureClass::ProposalRepairable, reason)),
    }
}

/// Compares the entries a plan names with the `src/<package>/main.py` files
/// already in `root`. Text the host supplied (the phase prompt and the profile
/// guidance) is not read as a request to change the entry.
pub fn assess(root: &Path, plan: &StepPlan) -> EntryIdentity {
    let existing = existing_entries(root);
    let named = named_entries(plan);
    if named.is_empty() {
        return EntryIdentity::Admitted;
    }
    match existing.as_slice() {
        [] => {
            let packages = named.iter().map(|r| &r.package).collect::<BTreeSet<_>>();
            if packages.len() < 2 {
                return EntryIdentity::Admitted;
            }
            EntryIdentity::Repairable(format!(
                "python-cli entry identity: this plan names {} different entries ({}). A python-cli project has exactly one src/<package>/main.py. Choose one entry, create only that one, and use the same path for every producer, verification command and [project.scripts] target. Keep every original feature requirement, output and verification check of this phase. Do not write the path of any entry you did not choose in instructions, not even in a negative sentence such as \"do not create ...\".",
                packages.len(),
                describe(&named),
            ))
        }
        [entry] => {
            if let Some(requested) = original_goal(&plan.goal).and_then(explicit_goal_package)
                && requested != *entry
            {
                return EntryIdentity::Repairable(format!(
                    "python-cli entry identity is ambiguous: the original goal names src/{requested}/main.py, but the workspace's only entry is src/{entry}/main.py. The host does not rename, replace, move or delete either entry, and a second main.py would break the single-entry contract."
                ));
            }
            let conflicting = named
                .iter()
                .filter(|r| r.package != *entry)
                .cloned()
                .collect::<Vec<_>>();
            if conflicting.is_empty() {
                return EntryIdentity::Admitted;
            }
            EntryIdentity::Repairable(format!(
                "python-cli entry identity: the existing entry is src/{entry}/main.py, but this plan names another package's entry ({}). Implement this phase's feature in src/{entry}/main.py (helper modules may sit beside it in src/{entry}/), do not create main.py in another package, and use src/{entry}/main.py as the target of every verification command and of [project.scripts] ({entry}.main:main). Keep every original feature requirement, output and verification check of this phase; change only the entry they target. Do not write any other package's src/<package>/main.py path in instructions, not even in a negative sentence such as \"do not create ...\"; name only src/{entry}/main.py.",
                describe(&conflicting),
            ))
        }
        // Several entries already: no winner is chosen and nothing is pinned;
        // the final `entry_cardinality` gate reports the existing failure.
        _ => EntryIdentity::Admitted,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Named {
    package: String,
    step: String,
    field: &'static str,
}

fn describe(named: &[Named]) -> String {
    named
        .iter()
        .map(|r| format!("step {} {} -> src/{}/main.py", r.step, r.field, r.package))
        .collect::<Vec<_>>()
        .join("; ")
}

fn existing_entries(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join("src")) else {
        return Vec::new();
    };
    let mut packages = entries
        .flatten()
        .filter(|entry| entry.path().join("main.py").is_file())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect::<Vec<_>>();
    packages.sort();
    packages
}

fn named_entries(plan: &StepPlan) -> Vec<Named> {
    let host_text = [
        compact(&plan.goal),
        PythonCliProfile
            .guidance(&plan.goal)
            .map(|guidance| compact(&guidance))
            .unwrap_or_default(),
    ];
    let mut named = BTreeSet::new();
    for step in &plan.steps {
        let mut push = |package: String, field: &'static str| {
            named.insert(Named {
                package,
                step: step.id.clone(),
                field,
            });
        };
        for path in &step.expected_paths {
            if let Some(package) = entry_path_package(path) {
                push(package, "expected_paths");
            }
        }
        for command in &step.verify {
            for token in tokens(command) {
                if let Some(package) =
                    entry_path_package(token).or_else(|| module_package(token, false))
                {
                    push(package, "verify");
                }
            }
        }
        let mut instruction = compact(&step.instruction);
        for text in host_text.iter().filter(|text| !text.is_empty()) {
            instruction = instruction.replace(text.as_str(), " ");
        }
        for token in tokens(&instruction) {
            if let Some(package) = entry_path_package(token).or_else(|| module_package(token, true))
            {
                push(package, "instruction");
            }
        }
    }
    named.into_iter().collect()
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch: char| {
        !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | '\\' | ':'))
    })
    .filter(|token| !token.is_empty())
}

fn trimmed(token: &str) -> String {
    let mut token = token
        .replace('\\', "/")
        .trim_end_matches(['.', ',', ':', ';'])
        .to_string();
    while let Some(rest) = token.strip_prefix("./") {
        token = rest.to_string();
    }
    token
}

/// `src/<package>/main.py`, written with or without `./` and `\`.
fn entry_path_package(token: &str) -> Option<String> {
    let token = trimmed(token);
    let package = token.strip_prefix("src/")?.strip_suffix("/main.py")?;
    safe_python_package_name(package).then(|| package.to_string())
}

/// `<package>.main` (as run by `python -m`) or the script target
/// `<package>.main:<function>`; prose only counts the script-target form.
fn module_package(token: &str, script_target_only: bool) -> Option<String> {
    let token = trimmed(token);
    if token.contains('/') {
        return None;
    }
    let (module, function) = match token.split_once(':') {
        Some((module, function)) => (module, Some(function)),
        None => (token.as_str(), None),
    };
    if script_target_only && !function.is_some_and(safe_python_package_name) {
        return None;
    }
    let module = module.strip_prefix("src.").unwrap_or(module);
    let package = module.strip_suffix(".main")?;
    safe_python_package_name(package).then(|| package.to_string())
}

/// The user's original goal inside a phase prompt; phase tasks, examples and
/// prior context that follow it are not user naming requests.
fn original_goal(prompt: &str) -> Option<&str> {
    let (_, rest) = prompt.split_once("Original ultra goal:")?;
    let end = ["\nProfile:", "\nPhase id:", "\n\n"]
        .iter()
        .filter_map(|marker| rest.trim_start_matches('\n').find(marker))
        .min();
    let rest = rest.trim_start_matches('\n');
    Some(end.map_or(rest, |end| &rest[..end]))
}

#[cfg(test)]
mod tests;
