//! Artifact paths the final-acceptance repair snapshot compares (Issue #949).
//!
//! Design record:
//!
//! - Origin. `artifact_contract_checks::run_check` records names only after
//!   the operator registry, the workspace separation, and the tooling pins
//!   passed; an earlier stop records nothing. Three kinds, in this order:
//!   1. the declared request: the check's `request` param (a profile or pack
//!      declaration), always, even when it is missing or does not parse;
//!   2. named repair candidates: when the request parses and names exactly
//!      the operator-registered input IDs, each input path that passes the
//!      binding's own lexical guard (`safe_relative`, no credential name).
//!      They are recorded *before* `bind_inputs`, so a missing input, a
//!      symlinked or special input, or one that fails its operator pin is
//!      still a candidate: creating or repairing it is a change, while the
//!      check itself keeps failing on it until it binds;
//!   3. bound inputs: every input that `bind_inputs` then accepts is one of
//!      the names in 2, so it is compared too.
//!
//!   A request that does not parse, or whose input IDs differ from the
//!   operator's, yields only the declared request. The artifacts the
//!   effective profile declares (`expected_scaffold_paths`) are added only
//!   when such names exist, so a run without an admitted check keeps the
//!   legacy candidate set byte for byte. The model's reported
//!   `changed_paths`, plan-step paths, and other workspace JSON/YAML are never
//!   added, and no new parser reads the request.
//! - Identity. A recorded name is compared under the exact string the check
//!   used to open it: it is never trimmed and `\` is never turned into `/`,
//!   so `data/a.json ` and `data\a.json` stay distinct from `data/a.json`. It
//!   is kept only when every component is a plain name that does not start
//!   with `.`, no component is `node_modules`, and the name is not a
//!   credential path. Profile-declared artifacts keep the legacy
//!   `safe_dir_rel_path` form, as the profile's own presence check uses it.
//!   Extensions are not consulted. The global source and rollback helpers
//!   are unchanged.
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

use std::path::{Component, Path};

use super::{resolve_profile_runtime, safe_dir_rel_path};

/// The artifact candidates for `root`: empty unless an admitted artifact
/// check recorded names in this workspace.
pub(super) fn artifact_repair_candidates(root: &Path, profile: &str, goal: &str) -> Vec<String> {
    let bound = crate::planner::artifact_contract_checks::bound_workspace_paths(root);
    if bound.is_empty() {
        return Vec::new();
    }
    let declared = resolve_profile_runtime(profile).expected_scaffold_paths(root, goal);
    artifact_candidates(&declared, &bound)
}

/// Declared artifacts first, then the recorded check names, each once,
/// within the boundary described in the module documentation.
pub(super) fn artifact_candidates(declared: &[String], bound: &[String]) -> Vec<String> {
    let declared = declared
        .iter()
        .filter_map(|raw| safe_dir_rel_path(raw))
        .filter(|rel| plain_identity(rel));
    let bound = bound.iter().filter(|raw| plain_identity(raw)).cloned();
    let mut out = Vec::new();
    for rel in declared.chain(bound) {
        if !out.contains(&rel) {
            out.push(rel);
        }
    }
    out
}

/// A workspace-relative name kept verbatim: plain components only, none
/// hidden or `node_modules`, and not a credential path.
fn plain_identity(raw: &str) -> bool {
    let path = Path::new(raw);
    !raw.is_empty()
        && !raw.contains('\0')
        && path.components().all(|component| match component {
            Component::Normal(part) => {
                let part = part.to_string_lossy();
                !part.starts_with('.') && !part.eq_ignore_ascii_case("node_modules")
            }
            _ => false,
        })
        && !crate::tools::sensitive_path::relative_path_is_sensitive(path)
}

#[cfg(test)]
#[path = "artifact_repair_tests.rs"]
mod tests;
