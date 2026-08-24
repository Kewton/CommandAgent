use std::path::Path;

use super::{RepairTargetSelection, RepairTargetSelectionReason};

// Target-resolution prerequisite audit (D-3a-3k):
// verified_diagnosis_mapped | matched claim + workspace-relative path | context+format | retain claim match and confinement only
// diagnosis               | diagnosis text is carried             | context        | retain carried text; no target existence check
// traceback               | parsed file/line                        | format         | retain parser and confinement only
// contract producer       | catalog check identity                 | context        | map all data catalog IDs to pipeline/main.py
// r_command_mapped        | path token in R                        | format         | normalize and confine; no file/parent check
// evidence                | evidence key                          | context        | preserve existing evidence mapping
// required_path           | declared required path                | format         | normalize and confine; no worktree check
// This table is a permanent audit boundary: adding a prerequisite requires
// updating it. Existence is a write-stage concern, never a resolver gate.

pub(crate) fn verified_diagnosis_target(
    _root: &Path,
    binding_path: &Path,
    reproducer_command: Option<&str>,
) -> Option<RepairTargetSelection> {
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(binding_path).ok()?).ok()?;
    let claims = value.get("claims")?.as_array()?;
    for claim in claims {
        if claim.get("matched").and_then(serde_json::Value::as_bool) != Some(true) {
            continue;
        }
        for key in ["subject_path", "file", "path"] {
            if let Some(path) = claim.get(key).and_then(serde_json::Value::as_str) {
                let path = path.trim_start_matches("./");
                if !path.is_empty()
                    && crate::tools::path_guard::validate_workspace_relative(path).is_ok()
                {
                    return Some(RepairTargetSelection {
                        selected_targets: vec![path.to_string()],
                        selection_reason: RepairTargetSelectionReason::VerifiedDiagnosisMapped,
                    });
                }
            }
        }
    }
    let catalog = reproducer_command.is_some_and(|command| {
        [
            "data_results_schema",
            "data_inspection_schema",
            "data_reconciliation",
            "data_claims_binding",
        ]
        .iter()
        .any(|id| command.contains(id))
            || command.contains("inspection_schema_violation")
            || command.contains("CommandFailed")
    });
    catalog.then(|| RepairTargetSelection {
        selected_targets: vec!["pipeline/main.py".to_string()],
        selection_reason: RepairTargetSelectionReason::VerifiedDiagnosisMapped,
    })
}

pub(crate) fn r_command_target(_root: &Path, command: &str) -> Option<RepairTargetSelection> {
    for token in command.split_whitespace() {
        let path = token
            .trim_matches(|ch: char| matches!(ch, '\'' | '"' | '`' | ')' | '(' | ','))
            .strip_prefix("./")
            .unwrap_or(token);
        if path.contains('/') && crate::tools::path_guard::validate_workspace_relative(path).is_ok()
        {
            return Some(RepairTargetSelection {
                selected_targets: vec![path.to_string()],
                selection_reason: RepairTargetSelectionReason::RCommandMapped,
            });
        }
    }
    None
}
