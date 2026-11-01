//! The raw-byte obligation of a host-bound final repair (Issue #956).
//!
//! Design record:
//!
//! - Binding. The obligation exists only when the host bound repair targets
//!   to this session: the final-acceptance repair loop binds the names of the
//!   failed artifact-check receipts to its escalation carryover just before
//!   the session, and the session takes them once when it starts. The step
//!   must also be a plan-run Implement step with the host mutation flag and
//!   no Recovery obligation (Issue #465 keeps its own proof). Options alone,
//!   a carryover alone, the prompt, the profile, the intent, the model, or
//!   the adapter never enable it. Without a binding nothing here reads a
//!   file, so every other session keeps its calls and early completions.
//! - What clears it. A completion candidate (after a tool batch, a final
//!   answer without tools, or a contract short-circuit) is held while no
//!   bound target's raw bytes differ from the start and the session made no
//!   successful Write or Edit. A read, a listing, a passing check, a
//!   self-reported change, an mtime-only touch, a change undone, a deletion,
//!   or a change to another file leaves it held; the next turn is an ordinary
//!   turn of the existing loop, so its iteration, stagnation, wall-clock, and
//!   provider limits end a session that keeps reading as a non-success. A
//!   raw-byte change of a bound target, by any tool (a Bash edit included),
//!   hands the candidate back to the existing checks. A Write or Edit hands it
//!   back too, whatever it wrote: the repair loop's raw-byte snapshot already
//!   turns a same-bytes or unrelated write into `no_source_change`, and
//!   holding the session would spend a provider call the bounded repair does
//!   not have. Neither hand-back is a pass: the same host check decides.
//! - Limits and guards. At most [`MAX_TARGETS`] targets; each adopted content
//!   at most [`MAX_FILE_BYTES`], the set at most [`MAX_TOTAL_BYTES`]. One set
//!   read reads at most one probe byte more per file, so at most
//!   1,048,577 bytes per file and 8,388,736 for the set. Only a workspace
//!   regular file is read: an escape, a credential path, any symlink (inside
//!   the workspace too, and on a parent), and a special file are refused
//!   before any open, and the open neither follows a link nor blocks, so a
//!   FIFO returns at once. A file replaced, grown, or changed while it is read
//!   is unknown. An unknown fingerprint never equals anything, so it never
//!   counts as a change; a binding over the target limit is unbindable and is
//!   held. Only digests and fixed reason words are kept; no content leaves
//!   this module except the placeholder check of the same bounded read.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path};

use sha2::{Digest, Sha256};

use crate::tools::sensitive_path::{SkippedPath, sensitive_skip};

use super::super::{RunSessionOptions, RunSessionScope, RunSessionStepKind};

#[cfg(test)]
#[path = "repair_artifact_completion_tests.rs"]
mod tests;

pub(crate) const MAX_TARGETS: usize = 128;
pub(crate) const MAX_FILE_BYTES: u64 = 1_048_576;
pub(crate) const MAX_TOTAL_BYTES: u64 = 8_388_608;

/// The raw-byte identity of one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fingerprint {
    Absent,
    Digest([u8; 32]),
    Unknown(&'static str),
}

impl Fingerprint {
    /// The digest in the form of the existing step hashes; an unknown target
    /// is named by its reason, an absent one has none.
    pub(crate) fn hash_hex(&self) -> Option<String> {
        match self {
            Self::Absent => None,
            Self::Digest(digest) => Some(digest.iter().map(|b| format!("{b:02x}")).collect()),
            Self::Unknown(reason) => Some(format!("unknown:{reason}")),
        }
    }

    /// A confirmed change: both sides known and different.
    pub(crate) fn changed_from(&self, before: &Self) -> bool {
        match (before, self) {
            (Self::Absent, Self::Digest(_)) => true,
            (Self::Digest(before), Self::Digest(after)) => before != after,
            _ => false,
        }
    }
}

pub(crate) struct TargetRead {
    pub(crate) fingerprint: Fingerprint,
    pub(crate) bytes: Option<Vec<u8>>,
}

/// One bounded read of a target set.
#[derive(Default)]
pub(crate) struct SetRead {
    pub(crate) items: Vec<TargetRead>,
    pub(crate) bytes_read: u64,
}

#[derive(Debug)]
enum State {
    Inactive,
    /// The binding is refused as a whole; its names are never read.
    Unbindable(&'static str, Vec<String>),
    Bound(Vec<(String, Fingerprint)>),
}

const TOO_MANY_TARGETS: Fingerprint = Fingerprint::Unknown("too_many_targets");

#[derive(Debug)]
pub(crate) struct RepairArtifactCompletion {
    state: State,
}

impl Default for RepairArtifactCompletion {
    fn default() -> Self {
        Self {
            state: State::Inactive,
        }
    }
}

/// A held completion: the feedback for the next turn and the counts for the
/// blocking event.
pub(crate) struct Held {
    pub(crate) feedback: String,
    pub(crate) targets: usize,
    pub(crate) unchanged: usize,
    pub(crate) absent: usize,
    pub(crate) unknown: usize,
}

/// The step conditions besides the binding itself.
fn eligible(options: &RunSessionOptions) -> bool {
    options.scope == RunSessionScope::PlanRunStep
        && options.step_kind == Some(RunSessionStepKind::Implement)
        && options.require_mutation_before_contract_short_circuit
        && options.recovery_obligation.is_none()
}

impl RepairArtifactCompletion {
    /// Takes the host binding of this session, if any, and records the
    /// initial fingerprints of its targets. Reads nothing without a binding.
    pub(crate) fn capture(root: &Path, options: &RunSessionOptions) -> Self {
        let Some(bound) = options
            .escalation_carryover
            .as_ref()
            .and_then(|carryover| carryover.take_repair_targets())
        else {
            return Self::default();
        };
        if !eligible(options) {
            return Self::default();
        }
        let mut targets: Vec<String> = Vec::new();
        for path in bound {
            if !targets.contains(&path) {
                targets.push(path);
            }
        }
        if targets.len() > MAX_TARGETS {
            return Self {
                state: State::Unbindable("too_many_targets", targets),
            };
        }
        let read = read_set(root, &targets, false);
        Self {
            state: State::Bound(
                targets
                    .into_iter()
                    .zip(read.items)
                    .map(|(path, item)| (path, item.fingerprint))
                    .collect(),
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn active(&self) -> bool {
        !matches!(self.state, State::Inactive)
    }

    /// The initial fingerprint of a bound target; `None` when `path` is not
    /// bound (the caller keeps its own reading for it).
    pub(crate) fn initial(&self, path: &str) -> Option<&Fingerprint> {
        match &self.state {
            State::Bound(targets) => targets
                .iter()
                .find(|(target, _)| target == path)
                .map(|(_, fingerprint)| fingerprint),
            State::Unbindable(_, names) => names
                .iter()
                .any(|name| name == path)
                .then_some(&TOO_MANY_TARGETS),
            State::Inactive => None,
        }
    }

    /// The current reading of bound `paths`; a refused binding reads nothing.
    pub(crate) fn read(&self, root: &Path, paths: &[String], keep_bytes: bool) -> SetRead {
        match &self.state {
            State::Unbindable(reason, _) => SetRead {
                items: paths.iter().map(|_| unknown(reason)).collect(),
                bytes_read: 0,
            },
            _ => read_set(root, paths, keep_bytes),
        }
    }

    /// `None` when the completion may go on to the existing checks.
    pub(crate) fn held(&self, root: &Path, write_or_edit_seen: bool) -> Option<Held> {
        if write_or_edit_seen {
            return None;
        }
        match &self.state {
            State::Inactive => None,
            State::Unbindable(reason, _) => Some(Held {
                feedback: format!(
                    "Final repair is incomplete: the host repair binding is unavailable ({reason}), so no change to the failing artifact can be confirmed. Reading or re-running a check does not repair it."
                ),
                targets: 0,
                unchanged: 0,
                absent: 0,
                unknown: 0,
            }),
            State::Bound(targets) => {
                let paths = targets.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>();
                let now = read_set(root, &paths, false);
                let mut held = Held {
                    feedback: String::new(),
                    targets: targets.len(),
                    unchanged: 0,
                    absent: 0,
                    unknown: 0,
                };
                let mut named = Vec::new();
                for ((path, before), after) in targets.iter().zip(&now.items) {
                    if after.fingerprint.changed_from(before) {
                        return None;
                    }
                    match &after.fingerprint {
                        Fingerprint::Absent => held.absent += 1,
                        Fingerprint::Unknown(_) => held.unknown += 1,
                        Fingerprint::Digest(_) => held.unchanged += 1,
                    }
                    if after.fingerprint != Fingerprint::Unknown("sensitive") {
                        named.push(path.as_str());
                    }
                }
                held.feedback = format!(
                    "Final repair is incomplete: the failing artifact ({}) is byte-for-byte unchanged since this repair started. Reading it, listing it, or re-running a check does not repair it. Change the failing artifact; the same host check then runs again.",
                    named.join(", ")
                );
                Some(held)
            }
        }
    }
}

/// Reads `paths` within the set bounds. More than [`MAX_TARGETS`] paths read
/// nothing and are all unknown.
pub(crate) fn read_set(root: &Path, paths: &[String], keep_bytes: bool) -> SetRead {
    let mut out = SetRead::default();
    if paths.len() > MAX_TARGETS {
        out.items = paths.iter().map(|_| unknown("too_many_targets")).collect();
        return out;
    }
    let mut remaining = MAX_TOTAL_BYTES;
    #[cfg(test)]
    tests::TARGET_VISITS.with(|visits| visits.set(visits.get() + paths.len()));
    for path in paths {
        let cap = MAX_FILE_BYTES.min(remaining);
        let (item, read) = read_one(root, path, cap, keep_bytes);
        remaining = remaining.saturating_sub(read.min(cap));
        out.bytes_read += read;
        out.items.push(item);
    }
    out
}

fn unknown(reason: &'static str) -> TargetRead {
    TargetRead {
        fingerprint: Fingerprint::Unknown(reason),
        bytes: None,
    }
}

/// One target, reading at most `cap + 1` bytes; returns the bytes read.
fn read_one(root: &Path, rel: &str, cap: u64, keep: bool) -> (TargetRead, u64) {
    let file = match open_regular(root, rel) {
        Ok(Some(file)) => file,
        Ok(None) => {
            let absent = TargetRead {
                fingerprint: Fingerprint::Absent,
                bytes: None,
            };
            return (absent, 0);
        }
        Err(reason) => return (unknown(reason), 0),
    };
    let Ok(before) = file.metadata() else {
        return (unknown("unreadable"), 0);
    };
    let mut bytes = Vec::new();
    if (&file).take(cap + 1).read_to_end(&mut bytes).is_err() {
        let read = bytes.len() as u64;
        return (unknown("unreadable"), read);
    }
    let read = bytes.len() as u64;
    if read > cap {
        return (unknown("over_limit"), read);
    }
    let stable = file
        .metadata()
        .is_ok_and(|after| after.len() == read && after.modified().ok() == before.modified().ok());
    if !stable {
        return (unknown("changed_during_read"), read);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let item = TargetRead {
        fingerprint: Fingerprint::Digest(digest),
        bytes: keep.then_some(bytes),
    };
    (item, read)
}

/// `Ok(None)` is an absent target. Every escape, credential path, symlink,
/// and special file is refused before any open.
fn open_regular(root: &Path, rel: &str) -> Result<Option<File>, &'static str> {
    let relative = Path::new(rel);
    if rel.is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("escape");
    }
    match sensitive_skip(root, &root.join(relative)) {
        Some(SkippedPath::Credential) => return Err("sensitive"),
        Some(SkippedPath::Outside) => return Err("escape"),
        None => {}
    }
    let mut cursor = root.to_path_buf();
    let count = relative.components().count();
    let mut leaf = None;
    for (index, component) in relative.components().enumerate() {
        cursor.push(component);
        let meta = match std::fs::symlink_metadata(&cursor) {
            Ok(meta) => meta,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("unreadable"),
        };
        if meta.file_type().is_symlink() {
            return Err("symlink");
        }
        if index + 1 < count {
            if !meta.is_dir() {
                return Ok(None);
            }
        } else if meta.file_type().is_file() {
            leaf = Some(meta);
        } else {
            return Err("special_file");
        }
    }
    let leaf = leaf.ok_or("escape")?;
    let file = open_no_follow(&cursor).map_err(|_| "unreadable")?;
    let opened = file.metadata().map_err(|_| "unreadable")?;
    if !opened.file_type().is_file() || !same_file(&leaf, &opened) {
        return Err("replaced");
    }
    Ok(Some(file))
}

#[cfg(unix)]
fn open_no_follow(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(unix)]
fn same_file(before: &std::fs::Metadata, opened: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    before.dev() == opened.dev() && before.ino() == opened.ino()
}

#[cfg(not(unix))]
fn same_file(_: &std::fs::Metadata, _: &std::fs::Metadata) -> bool {
    true
}
