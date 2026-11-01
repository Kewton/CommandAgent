use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::*;

mod repair_artifact_completion;
use repair_artifact_completion::{LegacyRead, RepairArtifactCompletion};

impl RunSessionOptions {
    pub(crate) fn final_acceptance_repair() -> Self {
        let mut options = Self::plan_step(RunSessionStepKind::Implement);
        options.require_mutation_before_contract_short_circuit = true;
        options
    }
}

/// Step-local source identity, deliberately captured before run-contract paths
/// are merged. A backend artifact cannot discharge a UI step's placeholder.
/// The targets a host bound to a final repair (Issue #956) are read only
/// through the bounded reader of `repair_artifact_completion`, here too.
#[derive(Default)]
pub(super) struct ImplementationCompletion {
    initial_hashes: BTreeMap<String, Option<String>>,
    repair: RepairArtifactCompletion,
}

impl ImplementationCompletion {
    pub(super) fn blocking_reason(feedback: &str) -> &'static str {
        if feedback.starts_with("Recovery repair remains unresolved:") {
            "recovery_repair_unresolved"
        } else if feedback.starts_with("Final repair is incomplete:") {
            "final_repair_artifact_unchanged"
        } else {
            "implementation_scaffold_placeholder"
        }
    }

    pub(super) fn capture(config: &Config, options: &RunSessionOptions, paths: &[String]) -> Self {
        let root = &config.workspace_root;
        // Taken first, so a session of another kind leaves no binding behind;
        // it reads nothing unless the session is eligible.
        let repair = RepairArtifactCompletion::capture(root, options);
        if options.step_kind != Some(RunSessionStepKind::Implement) {
            return Self::default();
        }
        let initial_hashes = paths
            .iter()
            .map(|path| {
                let hash = match repair.initial(path) {
                    Some(fingerprint) => fingerprint.hash_hex(),
                    None => legacy_hash(&repair, root, path),
                };
                (path.clone(), hash)
            })
            .collect();
        Self {
            initial_hashes,
            repair,
        }
    }

    /// The step paths with their current reading: bound targets through the
    /// bounded reader (bytes kept when `keep`), every other path as before.
    fn current(&self, root: &Path, keep: bool) -> Vec<(String, Reading)> {
        let (bound, other): (Vec<String>, Vec<String>) = self
            .initial_hashes
            .keys()
            .cloned()
            .partition(|path| self.repair.initial(path).is_some());
        let read = self.repair.read(root, &bound, keep);
        let mut out = bound
            .into_iter()
            .zip(read.items)
            .map(|(path, item)| {
                let reading = Reading::Bound(item.fingerprint.hash_hex(), item.bytes);
                (path, reading)
            })
            .collect::<Vec<_>>();
        out.extend(other.into_iter().map(|path| (path, Reading::Legacy)));
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    fn current_hashes(&self, root: &Path) -> Vec<(String, Option<String>)> {
        self.current(root, false)
            .into_iter()
            .map(|(path, reading)| {
                let hash = match reading {
                    Reading::Bound(hash, _) => hash,
                    Reading::Legacy => legacy_hash(&self.repair, root, &path),
                };
                (path, hash)
            })
            .collect()
    }

    fn placeholder_paths(&self, root: &Path) -> Vec<String> {
        self.current(root, true)
            .into_iter()
            .filter(|(path, reading)| {
                let content = match reading {
                    Reading::Bound(_, bytes) => bytes
                        .as_ref()
                        .and_then(|bytes| String::from_utf8(bytes.clone()).ok()),
                    Reading::Legacy => match self.repair.legacy_read(root, path) {
                        None => resolve_existing(root, path)
                            .ok()
                            .and_then(|full| std::fs::read_to_string(full).ok()),
                        Some(LegacyRead::Bytes(bytes)) => String::from_utf8(bytes).ok(),
                        Some(_) => None,
                    },
                };
                content.is_some_and(|content| {
                    crate::planner::profiles::nextjs::is_engine_owned_scaffold_page(path, &content)
                })
            })
            .map(|(path, _)| path)
            .collect()
    }

    pub(super) fn merge_changes(&self, root: &Path, paths: &mut Vec<String>) {
        for (path, after) in self.current_hashes(root) {
            if after != self.initial_hashes[&path] && !paths.contains(&path) {
                paths.push(path);
            }
        }
    }

    pub(super) fn changes(&self, root: &Path, paths: &[String]) -> Vec<String> {
        let mut changes = paths.to_vec();
        self.merge_changes(root, &mut changes);
        changes
    }

    pub(super) fn event(
        &self,
        config: &Config,
        options: &RunSessionOptions,
        write_seen: bool,
    ) -> Value {
        let root = &config.workspace_root;
        let changed = self.changes(root, &[]);
        let placeholders = self.placeholder_paths(root);
        let hashes = self
            .current_hashes(root)
            .into_iter()
            .map(|(path, after)| {
                json!({
                    "path": path,
                    "before_sha256": self.initial_hashes[&path],
                    "after_sha256": after,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "step_id": options.step_id.as_deref().unwrap_or(""),
            "write_or_edit_seen": write_seen,
            "scaffold_placeholder_paths": placeholders,
            "expected_path_hashes": hashes,
            "changed_paths": changed,
            "satisfaction_basis": if options.recovery_obligation.as_ref().is_some_and(|o| o.pending()) {
                "recovery_confirmation_pending"
            } else if !placeholders.is_empty() {
                "unsatisfied_scaffold_placeholder"
            } else if changed.is_empty() {
                "verified_existing_artifacts"
            } else {
                "verified_changed_artifacts"
            },
        })
    }

    pub(super) fn feedback(
        &self,
        config: &Config,
        options: &RunSessionOptions,
        write_seen: bool,
    ) -> Option<String> {
        if let Some(obligation) = &options.recovery_obligation
            && let Some(feedback) = obligation.feedback(config, options)
        {
            return Some(feedback);
        }
        let paths = self.placeholder_paths(&config.workspace_root);
        if paths.is_empty() {
            return self.repair_feedback(config, options, write_seen);
        }
        let mut event = self.event(config, options, write_seen);
        event["event"] = json!("step_completion_blocked");
        event["reason"] = json!("implementation_scaffold_placeholder");
        event["step_kind"] = json!("implement");
        event["phase_scope"] = json!(options.phase_scope.as_deref().unwrap_or(""));
        eval_events::emit(config.eval_events_path.as_deref(), event);
        Some(format!(
            "Implementation is incomplete: {} still contains the engine-owned scaffold page. Implement the requested UI in these expected paths. Reading it or writing an unrelated file does not satisfy this step.",
            paths.join(", ")
        ))
    }

    /// The raw-byte obligation of a host-bound final repair (Issue #956).
    fn repair_feedback(
        &self,
        config: &Config,
        options: &RunSessionOptions,
        write_seen: bool,
    ) -> Option<String> {
        let held = self.repair.held(&config.workspace_root, write_seen)?;
        let mut event = self.event(config, options, write_seen);
        event["event"] = json!("step_completion_blocked");
        event["reason"] = json!("final_repair_artifact_unchanged");
        event["step_kind"] = json!("implement");
        event["phase_scope"] = json!(options.phase_scope.as_deref().unwrap_or(""));
        event["repair_binding"] = json!({
            "targets": held.targets,
            "unchanged": held.unchanged,
            "absent": held.absent,
            "unknown": held.unknown,
        });
        eval_events::emit(config.eval_events_path.as_deref(), event);
        Some(held.feedback)
    }
}

/// One step path's current reading.
enum Reading {
    /// A bound target, read by the bounded reader: its hash and, when asked
    /// for, its bytes.
    Bound(Option<String>, Option<Vec<u8>>),
    /// Any other step path, read as before.
    Legacy,
}

/// A step path's hash outside the bound names: as before without a binding;
/// in a bound session through `legacy_read`, which never reads a bound target
/// under another spelling or blocks on a special file.
fn legacy_hash(repair: &RepairArtifactCompletion, root: &Path, path: &str) -> Option<String> {
    match repair.legacy_read(root, path) {
        None => source_hash(root, path),
        Some(LegacyRead::Absent) => None,
        Some(LegacyRead::Unknown(reason)) => Some(format!("unknown:{reason}")),
        Some(LegacyRead::Bytes(bytes)) => Some(format!("{:x}", Sha256::digest(bytes))),
    }
}

fn source_hash(root: &Path, path: &str) -> Option<String> {
    let bytes = std::fs::read(resolve_existing(root, path).ok()?).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)))
}
