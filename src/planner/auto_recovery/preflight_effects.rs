//! Supplemental phase evidence; the existing source hash gate remains mandatory.
use std::{collections::BTreeMap, path::Path};

use super::{Config, json};
use crate::planner::recovery_snapshot::{RecoveryBoundarySnapshot, source_file_sha256};

pub(super) struct Effects<'a> {
    config: &'a Config,
    root: &'a Path,
    allowed: &'a [String],
    observation_id: &'a str,
    previous: Option<BTreeMap<String, String>>,
    pub(super) error: Option<String>,
}

impl<'a> Effects<'a> {
    pub(super) fn new(
        config: &'a Config,
        root: &'a Path,
        allowed: &'a [String],
        checkpoint: &'a RecoveryBoundarySnapshot,
    ) -> Self {
        let mut effects = Self {
            config,
            root,
            allowed,
            observation_id: &checkpoint.workspace_relative_path,
            previous: None,
            error: None,
        };
        effects.record("before_observation");
        effects
    }

    pub(super) fn record(&mut self, stage: &str) {
        let current = source_file_sha256(self.root);
        let mut changes = Vec::new();
        if let (Some(before), Ok(after)) = (&self.previous, &current) {
            let paths: std::collections::BTreeSet<_> = before.keys().chain(after.keys()).collect();
            for path in paths {
                if before.get(path) != after.get(path) {
                    changes.push(json!({
                        "path": path,
                        "before_sha256": before.get(path),
                        "after_sha256": after.get(path),
                        "allowed_generated": self.allowed.contains(path),
                    }));
                }
            }
        }
        if let Err(error) = &current {
            self.error.get_or_insert_with(|| error.to_string());
        }
        crate::eval_events::emit(
            self.config.eval_events_path.as_deref(),
            json!({
                "event": "recovery_preflight_effect_observation",
                "observation_id": self.observation_id,
                "scope": "isolated_workspace",
                "stage": stage,
                "actor": "host_registered_observer",
                "status": if current.is_ok() { "observed" } else { "observation_failed" },
                "comparison_available": self.previous.is_some() && current.is_ok(),
                "changes": changes,
                "allowed_generated_paths": self.allowed,
                "observation_error": current.as_ref().err().map(ToString::to_string),
                // Nested browser/build/start/HTTP timing cannot be inferred from
                // these coarse host boundaries or used to rewrite historical timing.
                "first_write_within_stage": "unknown",
            }),
        );
        self.previous = current.ok();
    }
}
