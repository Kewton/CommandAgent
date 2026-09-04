//! Typed side-effect policy for isolated Recovery acceptance observations.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::minimal_loop::completion::CompletionContract;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RecoveryObservationPolicy {
    pub(crate) allowed_generated_paths: Vec<String>,
}

impl RecoveryObservationPolicy {
    pub(crate) fn for_contract_at_workspace(
        contract: &CompletionContract,
        workspace: &Path,
    ) -> Self {
        Self::for_contract_in_workspace(contract, Some(workspace))
    }

    fn for_contract_in_workspace(contract: &CompletionContract, workspace: Option<&Path>) -> Self {
        let Some(profile) = contract.profile.as_deref() else {
            return Self::default();
        };
        match crate::planner::profile::domain_profile(profile).id() {
            "data" => Self::for_data_contract(contract),
            id if id == crate::planner::profiles::nextjs::PROFILE_ID => workspace
                .map(|workspace| Self {
                    allowed_generated_paths: nextjs_registered_json_outputs(workspace, contract),
                })
                .unwrap_or_default(),
            _ => Self::default(),
        }
    }

    fn for_data_contract(contract: &CompletionContract) -> Self {
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

fn nextjs_registered_json_outputs(workspace: &Path, contract: &CompletionContract) -> Vec<String> {
    let source_text = crate::minimal_loop::import_scan::nextjs_route_bound_closure(workspace)
        .into_iter()
        .filter_map(|path| std::fs::read_to_string(workspace.join(path)).ok())
        .filter(|content| {
            let lower = content.to_ascii_lowercase();
            lower.contains("writefile(")
                || lower.contains("writefilesync(")
                || lower.contains("fs.promises.writefile")
        })
        .collect::<Vec<_>>()
        .join("\n");
    if source_text.is_empty() {
        return Vec::new();
    }

    let mut candidates = BTreeSet::new();
    collect_json_files(workspace, workspace, &mut candidates);
    candidates
        .into_iter()
        .filter_map(|path| {
            let relative = path.strip_prefix(workspace).ok()?;
            let relative_text = relative.to_string_lossy().replace('\\', "/");
            Some((path, relative_text))
        })
        .filter(|(_, relative)| !is_nextjs_config_json(relative))
        .filter(|(_, relative)| {
            !contract
                .required_paths
                .iter()
                .chain(contract.protected_paths.iter())
                .any(|registered| Path::new(relative).starts_with(registered))
        })
        .filter(|(path, relative)| {
            source_text.contains(relative)
                || path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| source_text.contains(name))
        })
        .map(|(_, relative)| relative)
        .collect()
}

fn collect_json_files(root: &Path, directory: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        if relative.components().any(|part| {
            matches!(
                part.as_os_str().to_str(),
                Some(".commandagent" | ".anvil" | ".git" | ".next" | "node_modules")
            )
        }) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_json_files(root, &path, out);
        } else if file_type.is_file()
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            out.insert(path);
        }
    }
}

fn is_nextjs_config_json(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "package.json" | "package-lock.json" | "tsconfig.json" | "jsconfig.json"
    ) || name.ends_with(".config.json")
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
            RecoveryObservationPolicy::for_contract_in_workspace(&contract(), None)
                .allowed_generated_paths,
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

    #[test]
    fn nextjs_policy_allows_only_existing_route_bound_json_output() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/app/api/tasks")).unwrap();
        std::fs::create_dir_all(root.path().join("src/lib")).unwrap();
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        std::fs::write(root.path().join("tsconfig.json"), "{}").unwrap();
        std::fs::write(root.path().join("tasks.json"), "[]\n").unwrap();
        std::fs::write(root.path().join("unbound.json"), "[]\n").unwrap();
        std::fs::write(
            root.path().join("src/app/api/tasks/route.ts"),
            r#"import { save } from "@/lib/tasks"; export async function POST(){ await save([]); return Response.json([]); }"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/lib/tasks.ts"),
            r#"import { writeFile } from "fs/promises"; export const save = (items: unknown[]) => writeFile("tasks.json", JSON.stringify(items));"#,
        )
        .unwrap();
        let mut contract = contract();
        contract.profile = Some("nextjs".to_string());
        contract.required_paths = vec!["package.json".to_string(), "tsconfig.json".to_string()];
        contract.protected_paths.clear();

        let policy = RecoveryObservationPolicy::for_contract_at_workspace(&contract, root.path());

        assert_eq!(policy.allowed_generated_paths, ["tasks.json"]);
    }
}
