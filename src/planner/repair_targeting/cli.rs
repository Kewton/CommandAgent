use std::path::Path;

pub(super) fn manifest_cli_candidates(root: &Path, profile: &str) -> Option<Vec<String>> {
    is_manifest_cli_profile(profile).then(|| {
        ["cli/main.py", "README.md", "USAGE.md"]
            .into_iter()
            .filter(|path| root.join(path).is_file())
            .map(str::to_string)
            .collect()
    })
}

pub(super) fn is_manifest_cli_profile(profile: &str) -> bool {
    crate::planner::profile::canonical_profile_name(profile) == "cli"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::repair_targeting::{
        FinalAcceptanceRepairTargetInput, default_repair_target_candidates,
        final_acceptance_snapshot_candidates, resolve_final_acceptance_repair_targets,
    };

    const MEASURED_FIXTURE: &str = "tests/corpus/apps/test0725_cli_elev_003/fixtures";

    #[test]
    fn measured_cli_final_repair_never_falls_back_to_nextjs_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        for relative in ["README.md", "cli/main.py", "data/sample.txt"] {
            let target = dir.path().join(relative);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(Path::new(MEASURED_FIXTURE).join(relative), target).unwrap();
        }

        let defaults = default_repair_target_candidates(dir.path(), "cli");
        let snapshots = final_acceptance_snapshot_candidates(dir.path(), "cli");
        let selection = resolve_final_acceptance_repair_targets(FinalAcceptanceRepairTargetInput {
            root: dir.path(),
            profile: "cli",
            pending_evidence: &[],
            contract_attribute_paths: &[],
            repair_changed_paths: &[],
            required_paths: &[],
            diagnosis_path: None,
        });

        assert_eq!(defaults, ["cli/main.py", "README.md"]);
        assert_eq!(snapshots, defaults);
        assert_eq!(selection.selected_targets, defaults);
        assert_eq!(selection.selection_reason, "fallback");
        assert!(selection.selected_targets.iter().all(|path| {
            !path.contains("page.")
                && !path.starts_with("app/")
                && !path.starts_with("pages/")
                && path != "package.json"
        }));
    }
}
