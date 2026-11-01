//! Artifact paths the final-acceptance repair snapshot compares (Issue #949).
//!
//! Design record:
//!
//! - Origin. Two sources only. (1) The paths an admitted
//!   `artifact_contract_check` bound in this workspace, kept by
//!   `artifact_contract_checks::run_check` once the operator registry, the
//!   workspace separation, and the tooling pins passed: the check's `request`
//!   (a profile or pack declaration) and, when the request names exactly the
//!   operator-registered input IDs, each request input that passes the same
//!   path guard the binding uses (plain relative components, no credential
//!   name). (2) The artifacts the effective profile declares
//!   (`expected_scaffold_paths`), taken only when (1) is non-empty, so a run
//!   without an admitted check keeps the legacy candidate set byte for byte.
//!   The model's reported `changed_paths`, plan-step paths, and any other
//!   workspace JSON/YAML are never added here, and no new parser reads the
//!   request: the existing bounded request reader parses it.
//! - Boundary. Each candidate passes `safe_dir_rel_path` (no absolute,
//!   parent, NUL, `.anvil`, `.commandagent`, or `node_modules` component) and
//!   no component may start with `.`. Extensions are not consulted.
//! - Limits and guards. The candidates are prepended to the existing snapshot
//!   list, so the snapshot's candidate cap, the per-file and total byte
//!   budgets, and its fingerprint rules apply unchanged: a credential path is
//!   dropped unread, a symlink escape, special file, unreadable or oversized
//!   file is `unknown:<n>` and never equal to itself, a missing path is
//!   absent. Values stay `sha256:` digests of raw bytes, so a same-bytes
//!   rewrite or an mtime-only touch is unchanged and a reformat is a change.
//! - Counting. A changed artifact only makes `changed_paths` non-empty in
//!   the existing bounded repair attempt; the loop then reruns the same final
//!   acceptance (and so the same check) inside that attempt. No attempt,
//!   loop, provider call, or budget is added, and the rerun's own receipt
//!   decides the outcome.

use std::path::Path;

use super::{resolve_profile_runtime, safe_dir_rel_path};

/// The artifact candidates for `root`: empty unless an admitted artifact
/// check bound paths in this workspace.
pub(super) fn artifact_repair_candidates(root: &Path, profile: &str, goal: &str) -> Vec<String> {
    let bound = crate::planner::artifact_contract_checks::bound_workspace_paths(root);
    if bound.is_empty() {
        return Vec::new();
    }
    let declared = resolve_profile_runtime(profile).expected_scaffold_paths(root, goal);
    artifact_candidates(&declared, &bound)
}

/// Declared artifacts first, then bound check paths, each once, within the
/// workspace boundary described in the module documentation.
pub(super) fn artifact_candidates(declared: &[String], bound: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for raw in declared.iter().chain(bound) {
        if let Some(rel) = safe_dir_rel_path(raw)
            && !rel.split('/').any(|part| part.starts_with('.'))
            && !out.contains(&rel)
        {
            out.push(rel);
        }
    }
    out
}

#[cfg(test)]
#[path = "artifact_repair_tests.rs"]
mod tests;
