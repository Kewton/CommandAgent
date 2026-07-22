use std::path::Path;

use crate::minimal_loop::import_scan::{ImportScanIssue, MissingImport, missing_import_target_rel};

use super::{
    RepairTargetPriority, RepairTargetResolutionInput, RepairTargetSelection,
    RepairTargetSelectionReason, resolve_repair_targets,
};

pub(crate) fn fix_profile_invariant_target_guidance(
    root: &Path,
    profile: &str,
    intent: &str,
    missing_imports: &[MissingImport],
) -> String {
    if !crate::planner::adjudication::contract::is_fix_intent(intent) {
        return String::new();
    }
    let diagnosis_paths = missing_imports
        .iter()
        .filter_map(|missing| match &missing.issue {
            ImportScanIssue::MissingModule => missing_import_target_rel(root, missing),
            ImportScanIssue::MissingExport {
                definition_path, ..
            } => Some(definition_path.clone()),
            ImportScanIssue::JsxInTs => Some(missing.source.clone()),
        })
        .collect::<Vec<_>>();
    if diagnosis_paths.is_empty() {
        return String::new();
    }
    let mapped = RepairTargetSelection {
        selected_targets: diagnosis_paths,
        selection_reason: RepairTargetSelectionReason::DiagnosisMapped,
    };
    let Some(selection) = resolve_repair_targets(RepairTargetResolutionInput {
        root,
        profile,
        pending_evidence: &[],
        missing_capabilities: &[],
        contract_attribute_paths: &[],
        repair_changed_paths: &[],
        required_paths: &[],
        fallback_paths: &[],
        mapped_selection: Some(&mapped),
        priority: RepairTargetPriority::FixIntent,
    }) else {
        return String::new();
    };
    format!(
        "\nFix profile-invariant repair target:\n- write-pressure target: {} (selection_reason={})\n",
        selection.primary_target().unwrap_or_default(),
        selection.selection_reason.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minimal_loop::import_scan::scan_relative_imports;

    #[test]
    fn non_export_invariant_maps_definition_before_contract_evidence_and_package() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/app")).unwrap();
        std::fs::write(root.path().join("package.json"), "{}").unwrap();
        std::fs::write(
            root.path().join("src/app/SpaceInvadersGame.tsx"),
            "import { useSpaceInvadersGame } from './game-engine';\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/app/game-engine.ts"),
            "export const startGame = () => {};\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/app/page.tsx"),
            "export default function Page(){ return <main />; }\n",
        )
        .unwrap();
        let missing = scan_relative_imports(
            root.path(),
            &[
                "src/app/SpaceInvadersGame.tsx".to_string(),
                "src/app/game-engine.ts".to_string(),
            ],
        )
        .unwrap();
        let guidance =
            fix_profile_invariant_target_guidance(root.path(), "nextjs", "fix", &missing);
        let mapped = crate::planner::fix_diagnostics::repair_target_from_prompt(&guidance).unwrap();
        let selection = resolve_repair_targets(RepairTargetResolutionInput {
            root: root.path(),
            profile: "nextjs",
            pending_evidence: &["restart_or_recoverable_state_evidence".to_string()],
            missing_capabilities: &[],
            contract_attribute_paths: &["src/app/page.tsx".to_string()],
            repair_changed_paths: &[],
            required_paths: &["package.json".to_string()],
            fallback_paths: &[],
            mapped_selection: Some(&mapped),
            priority: RepairTargetPriority::FixIntent,
        })
        .unwrap();

        assert_eq!(selection.primary_target(), Some("src/app/game-engine.ts"));
        assert_eq!(
            selection.selection_reason,
            RepairTargetSelectionReason::DiagnosisMapped
        );
        assert!(
            !selection
                .selected_targets
                .contains(&"package.json".to_string())
        );

        let contract = resolve_repair_targets(RepairTargetResolutionInput {
            root: root.path(),
            profile: "nextjs",
            pending_evidence: &["restart_or_recoverable_state_evidence".to_string()],
            missing_capabilities: &[],
            contract_attribute_paths: &["src/app/page.tsx".to_string()],
            repair_changed_paths: &[],
            required_paths: &["package.json".to_string()],
            fallback_paths: &[],
            mapped_selection: None,
            priority: RepairTargetPriority::FixIntent,
        })
        .unwrap();
        assert_eq!(
            contract.selection_reason,
            RepairTargetSelectionReason::ContractAttribute
        );

        let evidence = resolve_repair_targets(RepairTargetResolutionInput {
            root: root.path(),
            profile: "nextjs",
            pending_evidence: &["restart_or_recoverable_state_evidence".to_string()],
            missing_capabilities: &[],
            contract_attribute_paths: &[],
            repair_changed_paths: &[],
            required_paths: &["package.json".to_string()],
            fallback_paths: &[],
            mapped_selection: None,
            priority: RepairTargetPriority::FixIntent,
        })
        .unwrap();
        assert_eq!(
            evidence.selection_reason,
            RepairTargetSelectionReason::EvidenceMapped
        );
    }

    #[test]
    fn create_profile_invariant_prompt_bytes_receive_no_fix_guidance() {
        let guidance =
            fix_profile_invariant_target_guidance(Path::new("."), "nextjs", "create", &[]);
        assert!(guidance.is_empty());
    }

    #[test]
    fn verified_catalog_diagnosis_targets_data_producer() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("pipeline")).unwrap();
        std::fs::write(dir.path().join("pipeline/main.py"), "raise SystemExit(1)").unwrap();
        let binding = dir.path().join("binding.json");
        std::fs::write(&binding, r#"{"claims":[{"matched":true}]}"#).unwrap();
        let selection = crate::planner::repair_targeting::verified_diagnosis_target(
            dir.path(),
            &binding,
            Some("inspection_schema_violation"),
        )
        .unwrap();
        assert_eq!(selection.primary_target(), Some("pipeline/main.py"));
        assert_eq!(
            selection.selection_reason,
            RepairTargetSelectionReason::VerifiedDiagnosisMapped
        );
    }
}
