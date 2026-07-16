use std::path::Path;

use crate::eval_events::{CompletionProjection, CompletionSnapshot};
use crate::planner::profile::canonical_profile_name;
use crate::planner::profiles::data::runtime::{DataAssurance, assurance_from_evidence};

pub(super) fn apply_snapshot(root: &Path, snapshot: &mut CompletionSnapshot) {
    let (assurance, reason) = completion_assurance(root);
    snapshot.assurance_level = assurance.as_str().to_string();
    snapshot.assurance_reason = reason.to_string();
}

pub(super) fn apply_terminal_projection(root: &Path, projection: &mut CompletionProjection) {
    if projection.contract_origin == crate::planner::fix_runtime::FIX_CONTRACT_ORIGIN
        || canonical_profile_name(&projection.effective_profile) != "data"
        || projection.final_acceptance != "full_success"
    {
        return;
    }
    let (assurance, reason) = completion_assurance(root);
    projection.assurance_level = assurance.as_str().to_string();
    projection.assurance_reason = reason.to_string();
}

fn completion_assurance(root: &Path) -> (DataAssurance, &'static str) {
    let assurance = assurance_from_evidence(root);
    let reason = match assurance {
        DataAssurance::Full => "",
        DataAssurance::Partial => "data_assurance_partial",
        DataAssurance::Static => "data_profile_probe_not_run",
        DataAssurance::Failed if !root.join("pipeline/main.py").is_file() => {
            "data_profile_script_not_generated"
        }
        DataAssurance::Failed => "data_assurance_failed",
    };
    (assurance, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_events::project_completion;

    const FIXTURE_ROOT: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/corpus/apps/test0715_data_b2j_terminal_projection/fixtures"
    );
    const EVIDENCE_FILES: [&str; 5] = [
        "pipeline-run.json",
        "reconciliation.json",
        "claims-binding.json",
        "rerun-consistency.json",
        "results-schema.json",
    ];

    #[test]
    fn run3_and_run4_evidence_restore_full_terminal_assurance() {
        for case in ["data7_gemma31_profile_001", "data7_qwen35_none_001"] {
            let root = Path::new(FIXTURE_ROOT).join(case);
            let mut projection = data_projection(&root, "full_success", "pass", true);
            assert_eq!(projection.assurance_level, "partial", "{case}");
            assert_eq!(projection.assurance_reason, "completion_contract_not_bound");

            apply_terminal_projection(&root, &mut projection);

            assert_eq!(projection.assurance_level, "full", "{case}");
            assert!(projection.assurance_reason.is_empty(), "{case}");
        }
    }

    #[test]
    fn full_success_without_complete_evidence_never_projects_full() {
        let source = Path::new(FIXTURE_ROOT).join("data7_gemma31_profile_001");
        let dir = tempfile::tempdir().unwrap();
        copy_fixture(&source, dir.path(), Some("rerun-consistency.json"));
        let mut projection = data_projection(dir.path(), "full_success", "pass", true);

        apply_terminal_projection(dir.path(), &mut projection);

        assert_eq!(projection.assurance_level, "failed");
        assert_eq!(projection.assurance_reason, "data_assurance_failed");
    }

    #[test]
    fn data_early_failure_remains_conservative_with_complete_evidence() {
        let root = Path::new(FIXTURE_ROOT).join("data7_gemma31_profile_001");
        let mut projection = data_projection(&root, "failed", "failed", false);
        let before = projection.clone();

        apply_terminal_projection(&root, &mut projection);

        assert_eq!(projection, before);
        assert_eq!(projection.assurance_level, "partial");
        assert_eq!(projection.assurance_reason, "acceptance_not_full_success");
    }

    #[test]
    fn nextjs_completion_projections_are_unchanged() {
        for (final_acceptance, release_gate, ok) in
            [("full_success", "pass", true), ("failed", "failed", false)]
        {
            let mut snapshot = CompletionSnapshot::empty();
            snapshot.profile = "nextjs".to_string();
            snapshot.effective_profile = "nextjs".to_string();
            snapshot.assurance_level = "full".to_string();
            snapshot.final_acceptance_status = final_acceptance.to_string();
            snapshot.release_gate_status = release_gate.to_string();
            snapshot.completion_contract_verification_enabled = true;
            snapshot.external_contract_checked = true;
            let mut projection = project_completion(ok, &snapshot);
            let before = projection.clone();

            apply_terminal_projection(Path::new(FIXTURE_ROOT), &mut projection);

            assert_eq!(projection, before);
        }
    }

    fn data_projection(
        root: &Path,
        final_acceptance: &str,
        release_gate: &str,
        ok: bool,
    ) -> CompletionProjection {
        let mut snapshot = CompletionSnapshot::empty();
        snapshot.profile = "data".to_string();
        snapshot.effective_profile = "data".to_string();
        snapshot.runtime_acceptance_status = if ok { "pass" } else { "failed" }.to_string();
        snapshot.final_acceptance_status = final_acceptance.to_string();
        snapshot.release_gate_status = release_gate.to_string();
        apply_snapshot(root, &mut snapshot);
        project_completion(ok, &snapshot)
    }

    fn copy_fixture(source: &Path, target: &Path, omitted: Option<&str>) {
        std::fs::create_dir_all(target.join("pipeline")).unwrap();
        std::fs::create_dir_all(target.join("evidence")).unwrap();
        std::fs::copy(
            source.join("pipeline/main.py"),
            target.join("pipeline/main.py"),
        )
        .unwrap();
        for name in EVIDENCE_FILES {
            if omitted == Some(name) {
                continue;
            }
            std::fs::copy(
                source.join("evidence").join(name),
                target.join("evidence").join(name),
            )
            .unwrap();
        }
    }
}
