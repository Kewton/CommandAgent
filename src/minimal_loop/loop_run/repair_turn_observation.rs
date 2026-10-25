//! Keeps a step repair session alive after a batch that changed nothing.
//!
//! A verify step may finish after one tool batch because its first
//! observation only runs commands. A repair session inherits the step's
//! options, so the same rule used to end it after a read-only batch, before
//! the model could act on what it read. Host verification still judges the
//! repair afterwards; only the early completion is deferred here.
//!
//! A batch without Write or Edit may still change files through Bash. Such a
//! batch only counts as unchanged when the fingerprints of the known target
//! files match before and after it. Anything that cannot be fingerprinted
//! ends the session as it did before.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{RunSessionOptions, RunSessionScope};
use crate::state::ToolCall;
use crate::tools::path_guard::{resolve_existing, validate_workspace_relative};
use crate::tools::sensitive_path::sensitive_skip;

const MAX_TARGET_FILES: usize = 128;
const MAX_TARGET_FILE_BYTES: u64 = 1024 * 1024;

/// A step repair session is the only `PlanRunStep` session that carries
/// escalation state; the planner attaches it when it starts a repair turn.
pub(super) fn is_step_repair_turn(options: &RunSessionOptions) -> bool {
    options.scope == RunSessionScope::PlanRunStep && options.escalation_carryover.is_some()
}

/// `None` content means the file did not exist.
type Fingerprints = BTreeMap<String, Option<[u8; 32]>>;

/// What a repair turn knew before one tool batch ran.
pub(super) struct RepairBatchProbe {
    targets: Vec<String>,
    had_bash: bool,
    before: Option<Fingerprints>,
}

impl RepairBatchProbe {
    pub(super) fn before_batch(
        root: &Path,
        options: &RunSessionOptions,
        failure_paths: &[String],
        tool_calls: &[ToolCall],
    ) -> Self {
        let mut probe = Self {
            targets: Vec::new(),
            had_bash: tool_calls.iter().any(|call| call.name == "Bash"),
            before: None,
        };
        if is_step_repair_turn(options) && probe.had_bash {
            for path in failure_paths
                .iter()
                .chain(&options.path_fallback_candidates)
            {
                if !probe.targets.contains(path) {
                    probe.targets.push(path.clone());
                }
            }
            probe.before = fingerprints(root, &probe.targets);
        }
        probe
    }
}

fn fingerprints(root: &Path, targets: &[String]) -> Option<Fingerprints> {
    if targets.is_empty() || targets.len() > MAX_TARGET_FILES {
        return None;
    }
    let mut prints = Fingerprints::new();
    for target in targets {
        validate_workspace_relative(target).ok()?;
        let full = root.join(target);
        if sensitive_skip(root, &full).is_some() {
            return None;
        }
        if std::fs::symlink_metadata(&full)
            .is_err_and(|err| err.kind() == std::io::ErrorKind::NotFound)
        {
            prints.insert(target.clone(), None);
            continue;
        }
        let resolved = resolve_existing(root, target).ok()?;
        let metadata = std::fs::metadata(&resolved).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_TARGET_FILE_BYTES {
            return None;
        }
        let bytes = std::fs::read(&resolved).ok()?;
        prints.insert(target.clone(), Some(Sha256::digest(bytes).into()));
    }
    Some(prints)
}

/// True when a repair turn must continue instead of completing on the batch
/// it just ran: the batch had no Write or Edit, and either ran no Bash or ran
/// Bash without changing any fingerprinted target file.
pub(super) fn defers_tool_observation_completion(
    options: &RunSessionOptions,
    batch_had_edit: bool,
    probe: &RepairBatchProbe,
    root: &Path,
) -> bool {
    if !is_step_repair_turn(options) || batch_had_edit {
        return false;
    }
    if !probe.had_bash {
        return true;
    }
    match &probe.before {
        Some(before) => fingerprints(root, &probe.targets).as_ref() == Some(before),
        None => false,
    }
}

#[cfg(test)]
#[path = "verify_repair_observation_tests.rs"]
mod verify_repair_observation_tests;
