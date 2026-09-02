//! Typed side-effect policy for isolated Recovery acceptance observations.

use std::path::Path;

use crate::minimal_loop::completion::CompletionContract;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RecoveryObservationPolicy {
    pub(crate) allowed_generated_paths: Vec<String>,
}

impl RecoveryObservationPolicy {
    pub(crate) fn for_contract(contract: &CompletionContract) -> Self {
        let Some(profile) = contract.profile.as_deref() else {
            return Self::default();
        };
        if crate::planner::profile::domain_profile(profile).id() != "data" {
            return Self::default();
        }

        let artifacts = crate::planner::profiles::data::manifest::required_artifacts();
        let source_paths = crate::planner::profiles::data::manifest::source_paths();
        let allowed_generated_paths = contract
            .required_paths
            .iter()
            .filter(|path| artifacts.contains(path))
            .filter(|path| !source_paths.contains(path))
            .filter(|path| !is_protected(path, &contract.protected_paths))
            .cloned()
            .collect();
        Self {
            allowed_generated_paths,
        }
    }
}

pub(crate) fn registered_data_input_fixture(contract: &CompletionContract) -> Option<String> {
    if contract
        .profile
        .as_deref()
        .is_none_or(|profile| crate::planner::profile::domain_profile(profile).id() != "data")
    {
        return None;
    }
    let commands = contract
        .fix_reproducer_command
        .iter()
        .chain(contract.verify_commands.iter())
        .collect::<Vec<_>>();
    let mut candidates = contract
        .required_paths
        .iter()
        .filter(|path| {
            matches!(
                Path::new(path).extension().and_then(|value| value.to_str()),
                Some("csv" | "tsv")
            )
        })
        .filter(|path| {
            commands
                .iter()
                .any(|command| command.contains(path.as_str()))
        })
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    (candidates.len() == 1).then(|| candidates.remove(0))
}

fn is_protected(path: &str, protected_paths: &[String]) -> bool {
    protected_paths
        .iter()
        .any(|protected| Path::new(path).starts_with(protected))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> CompletionContract {
        CompletionContract {
            required_paths: vec![
                "pipeline/main.py".to_string(),
                "data/task-02.csv".to_string(),
                "output/inspection.json".to_string(),
                "output/results.json".to_string(),
                "output/report.md".to_string(),
            ],
            protected_paths: vec!["data/task-02.csv".to_string()],
            verify_commands: vec!["python3 scripts/repro.py data/task-02.csv".to_string()],
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
    fn data_policy_allows_only_registered_generated_artifacts() {
        assert_eq!(
            RecoveryObservationPolicy::for_contract(&contract()).allowed_generated_paths,
            [
                "output/inspection.json",
                "output/results.json",
                "output/report.md"
            ]
        );
    }

    #[test]
    fn data_fixture_is_bound_from_the_registered_contract() {
        assert_eq!(
            registered_data_input_fixture(&contract()).as_deref(),
            Some("data/task-02.csv")
        );
    }
}
