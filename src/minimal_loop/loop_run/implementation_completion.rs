use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::*;

impl RunSessionOptions {
    pub(crate) fn final_acceptance_repair() -> Self {
        let mut options = Self::plan_step(RunSessionStepKind::Implement);
        options.require_mutation_before_contract_short_circuit = true;
        options
    }
}

/// Step-local source identity, deliberately captured before run-contract paths
/// are merged. A backend artifact cannot discharge a UI step's placeholder.
#[derive(Default)]
pub(super) struct ImplementationCompletion {
    initial_hashes: BTreeMap<String, Option<String>>,
}

impl ImplementationCompletion {
    pub(super) fn capture(config: &Config, options: &RunSessionOptions, paths: &[String]) -> Self {
        if options.step_kind != Some(RunSessionStepKind::Implement) {
            return Self::default();
        }
        Self {
            initial_hashes: paths
                .iter()
                .map(|path| (path.clone(), source_hash(&config.workspace_root, path)))
                .collect(),
        }
    }

    fn placeholder_paths(&self, root: &Path) -> Vec<String> {
        self.initial_hashes
            .keys()
            .filter(|path| {
                resolve_existing(root, path)
                    .ok()
                    .and_then(|full| std::fs::read_to_string(full).ok())
                    .is_some_and(|content| {
                        crate::planner::profiles::nextjs::is_engine_owned_scaffold_page(
                            path, &content,
                        )
                    })
            })
            .cloned()
            .collect()
    }

    pub(super) fn merge_changes(&self, root: &Path, paths: &mut Vec<String>) {
        for (path, before) in &self.initial_hashes {
            if source_hash(root, path) != *before && !paths.contains(path) {
                paths.push(path.clone());
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
            .initial_hashes
            .iter()
            .map(|(path, before)| {
                json!({
                    "path": path,
                    "before_sha256": before,
                    "after_sha256": source_hash(root, path),
                })
            })
            .collect::<Vec<_>>();
        json!({
            "step_id": options.step_id.as_deref().unwrap_or(""),
            "write_or_edit_seen": write_seen,
            "scaffold_placeholder_paths": placeholders,
            "expected_path_hashes": hashes,
            "changed_paths": changed,
            "satisfaction_basis": if !placeholders.is_empty() {
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
        let paths = self.placeholder_paths(&config.workspace_root);
        if paths.is_empty() {
            return None;
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
}

fn source_hash(root: &Path, path: &str) -> Option<String> {
    let bytes = std::fs::read(resolve_existing(root, path).ok()?).ok()?;
    Some(format!("{:x}", Sha256::digest(bytes)))
}
