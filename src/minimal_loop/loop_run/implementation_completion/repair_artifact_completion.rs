//! The raw-byte obligation of a host-bound final repair (Issue #956).
//!
//! Design record:
//!
//! - Binding. The obligation exists only when the host bound repair targets
//!   to this session: the final-acceptance repair loop binds the names of the
//!   failed artifact-check receipts to its escalation carryover just before
//!   the session, and the session takes them once when it starts (the
//!   carryover wrapper discards an untaken binding on every exit). The step
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
//!   hands the candidate back to the existing checks. A successful Write or
//!   Edit hands it back too, whatever it wrote, so the bounded repair spends
//!   no provider call it does not have. Neither hand-back is a pass. The
//!   repair loop then compares its own raw-byte snapshot, whose candidates
//!   are not this binding: no change there (a same-bytes rewrite, a change
//!   undone, a write outside it) ends the attempt as `no_source_change`; a
//!   change there reruns the full host final acceptance, and an invalid bound
//!   artifact that is still invalid keeps its receipt failed.
//! - Identity. A target and a step path are the same file when their
//!   lexical components are: repeated separators and `.` components are
//!   dropped, so `contract//terms.json`, `./contract/terms.json`, and
//!   `contract/terms.json` are one target and every reading of it goes
//!   through this reader. Names are never trimmed, `\` is never turned into
//!   `/`, and symlinks are never resolved, so no other file is taken for a
//!   target. A name with a `..`, root, or prefix component is an escape and
//!   matches only its own spelling. The original spelling is what callers
//!   show.
//! - Other spellings. Releasing the obligation needs that lexical identity;
//!   letting the legacy reader read a step path is decided apart from it. In
//!   a bound session every step path the bound names do not spell is
//!   resolved as the legacy reader resolves it (root-prefix salvage and a
//!   case-insensitive filesystem included), refused before any open when it
//!   is not a regular file, and opened by the same no-follow, non-blocking
//!   fd walk. It is read only when every bound name is shown not to be the
//!   opened file: the name's `lstat` now equals its `lstat` at the binding,
//!   is no symlink and no error, and is not the opened `(st_dev, st_ino)`;
//!   that is checked before the read and again after it, and the read stops
//!   at the file bound plus one probe byte. Anything else is unknown
//!   (`alias`, `special_file`, `over_limit`, `replaced`) and no bytes are
//!   kept, so a name turned into a symlink, renamed, or unlinked never lets
//!   an alias be read without bound. Such an alias is never a bound target:
//!   it neither releases nor holds anything.
//! - Limits and guards. At most [`MAX_TARGETS`] targets; each adopted content
//!   at most [`MAX_FILE_BYTES`], the set at most [`MAX_TOTAL_BYTES`]. One set
//!   read reads at most one probe byte more per file, so at most
//!   1,048,577 bytes per file and 8,388,736 for the set. Only a workspace
//!   regular file is read: an escape, a credential path, any symlink (inside
//!   the workspace too, and on a parent), and a special file are refused
//!   before any open. The open walks every parent from a directory fd with
//!   `O_DIRECTORY | O_NOFOLLOW` and opens the leaf from its parent fd with
//!   `O_NOFOLLOW | O_NONBLOCK` (`tools::dir_fd::open_read_file`), so a parent
//!   swapped for a symlink is refused and a FIFO returns at once.
//! - Replacement. The identity of a read is the `(st_dev, st_ino)` of the
//!   opened fd. It must equal the `lstat` of the leaf taken before the open,
//!   and a second fd walk after the read must reach the same identity; the
//!   fd's length must equal the bytes read and its length and mtime must not
//!   change during the read. Otherwise the target is unknown (`replaced`,
//!   `changed_during_read`). A platform without that fd walk reads nothing
//!   and is unknown (`unsupported_platform`).
//! - Unknown. An unknown fingerprint never equals anything, so it never
//!   counts as a change; a binding over the target limit is unbindable and is
//!   held. Only digests and fixed reason words are kept; no content leaves
//!   this module except the placeholder check of the same bounded read.

use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::tools::sensitive_path::{SkippedPath, sensitive_skip};

use super::super::{RunSessionOptions, RunSessionScope, RunSessionStepKind};

#[cfg(test)]
#[path = "repair_artifact_completion_tests.rs"]
mod tests;

/// The fixed feedback of a held completion: every bound target is known and
/// unchanged, or at least one cannot be verified (over a bound, refused, or
/// replaced). Only bound names fill the parentheses; no content or size.
const UNCHANGED_PREFIX: &str = "Final repair is incomplete: the failing artifact";
const UNCHANGED_REST: &str = " is byte-for-byte unchanged since this repair started. Reading it, listing it, or re-running a check does not repair it. Change the failing artifact; the same host check then runs again.";
const UNVERIFIABLE_PREFIX: &str = "Final repair is incomplete: the changed target";
const UNVERIFIABLE_REST: &str = " could not be verified because it exceeds the verification size limit or cannot be read safely. A change to it is not confirmed, so this repair is not complete; reading it or re-running a check does not confirm it either.";

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

/// The lexical identity of a workspace-relative name: its normal components,
/// with repeated separators and `.` components dropped. `None` for an empty
/// name or one with a `..`, root, or prefix component.
pub(crate) fn identity(name: &str) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for component in Path::new(name).components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

/// What a name's `lstat` showed: a regular file's `(st_dev, st_ino)`, no
/// regular file (absent, a directory, a special file), or nothing safe to
/// compare (`Err`: a symlink, an error, or no such metadata on this
/// platform).
type FileId = Result<Option<(u64, u64)>, ()>;

#[cfg(unix)]
fn file_id(path: &Path) -> FileId {
    use std::os::unix::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(()),
        Ok(meta) if meta.file_type().is_file() => Ok(Some((meta.dev(), meta.ino()))),
        Ok(_) => Ok(None),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

#[cfg(not(unix))]
fn file_id(_path: &Path) -> FileId {
    Err(())
}

/// A bound name, its identity, and what its `lstat` showed at the binding.
#[derive(Debug)]
struct Name {
    spelling: String,
    identity: Option<PathBuf>,
    captured: FileId,
}

impl Name {
    fn new(root: &Path, spelling: String) -> Self {
        let identity = identity(&spelling);
        let captured = match &identity {
            Some(relative) => file_id(&root.join(relative)),
            None => Ok(None),
        };
        Self {
            spelling,
            identity,
            captured,
        }
    }

    /// Whether this name is shown, now as at the binding, not to be the
    /// regular file `opened`. A name that is or was a symlink, whose
    /// `lstat` fails, or whose file changed since the binding cannot be
    /// told apart from `opened`, so it is not.
    fn excludes(&self, root: &Path, opened: (u64, u64)) -> bool {
        let Some(relative) = &self.identity else {
            // An escape never resolves inside the workspace.
            return true;
        };
        let current = file_id(&root.join(relative));
        current.is_ok() && current == self.captured && current != Ok(Some(opened))
    }

    /// Whether `path` names the same target.
    fn names(&self, path: &str) -> bool {
        match (&self.identity, identity(path)) {
            (Some(own), Some(other)) => *own == other,
            _ => self.spelling == path,
        }
    }
}

#[derive(Debug)]
enum State {
    Inactive,
    /// The binding is refused as a whole; its names are never read.
    Unbindable(&'static str, Vec<Name>),
    Bound(Vec<(Name, Fingerprint)>),
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
    /// initial fingerprints of its targets. Reads nothing without a binding
    /// or when the session is not eligible; the binding is taken either way.
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
        let mut targets: Vec<Name> = Vec::new();
        for spelling in bound {
            let name = Name::new(root, spelling);
            if !targets.iter().any(|target| target.names(&name.spelling)) {
                targets.push(name);
            }
        }
        if targets.len() > MAX_TARGETS {
            return Self {
                state: State::Unbindable("too_many_targets", targets),
            };
        }
        let spellings = targets
            .iter()
            .map(|target| target.spelling.clone())
            .collect::<Vec<_>>();
        let read = read_set(root, &spellings, false);
        Self {
            state: State::Bound(
                targets
                    .into_iter()
                    .zip(read.items.into_iter().map(|item| item.fingerprint))
                    .collect(),
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn active(&self) -> bool {
        !matches!(self.state, State::Inactive)
    }

    /// The initial fingerprint of the bound target `path` names; `None` when
    /// it names none (the caller keeps its own reading for it).
    pub(crate) fn initial(&self, path: &str) -> Option<&Fingerprint> {
        match &self.state {
            State::Bound(targets) => targets
                .iter()
                .find(|(target, _)| target.names(path))
                .map(|(_, fingerprint)| fingerprint),
            State::Unbindable(_, names) => names
                .iter()
                .any(|name| name.names(path))
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

    /// The legacy reading of a step path the bound names do not spell; `None`
    /// when no binding is active (the caller keeps its own reading).
    pub(crate) fn legacy_read(&self, root: &Path, path: &str) -> Option<LegacyRead> {
        let names = match &self.state {
            State::Inactive => return None,
            State::Unbindable(_, names) => names.iter().collect::<Vec<_>>(),
            State::Bound(targets) => targets.iter().map(|(name, _)| name).collect(),
        };
        let Ok(resolved) = crate::tools::path_guard::resolve_existing(root, path) else {
            return Some(LegacyRead::Absent);
        };
        Some(read_unless_bound(root, &resolved, &names))
    }

    /// `None` when the completion may go on to the existing checks.
    pub(crate) fn held(&self, root: &Path, write_or_edit_seen: bool) -> Option<Held> {
        if write_or_edit_seen {
            return None;
        }
        match &self.state {
            State::Inactive => None,
            State::Unbindable(reason, names) => Some(Held {
                feedback: format!(
                    "Final repair is incomplete: the host repair binding is unavailable ({reason}), so no change to the failing artifact can be confirmed. Reading or re-running a check does not repair it."
                ),
                targets: names.len(),
                unchanged: 0,
                absent: 0,
                unknown: names.len(),
            }),
            State::Bound(targets) => {
                let paths = targets
                    .iter()
                    .map(|(name, _)| name.spelling.clone())
                    .collect::<Vec<_>>();
                let now = read_set(root, &paths, false);
                let mut held = Held {
                    feedback: String::new(),
                    targets: targets.len(),
                    unchanged: 0,
                    absent: 0,
                    unknown: 0,
                };
                let mut named = Vec::new();
                let mut unverifiable = Vec::new();
                let mut any_unknown = false;
                for ((name, before), after) in targets.iter().zip(&now.items) {
                    if after.fingerprint.changed_from(before) {
                        return None;
                    }
                    let shown = after.fingerprint != Fingerprint::Unknown("sensitive")
                        && before != &Fingerprint::Unknown("sensitive");
                    match &after.fingerprint {
                        Fingerprint::Absent => held.absent += 1,
                        Fingerprint::Unknown(_) => held.unknown += 1,
                        Fingerprint::Digest(_) => held.unchanged += 1,
                    }
                    let unknown = matches!(after.fingerprint, Fingerprint::Unknown(_))
                        || matches!(before, Fingerprint::Unknown(_));
                    any_unknown |= unknown;
                    if shown {
                        named.push(name.spelling.as_str());
                        if unknown {
                            unverifiable.push(name.spelling.as_str());
                        }
                    }
                }
                held.feedback = if any_unknown {
                    format!(
                        "{UNVERIFIABLE_PREFIX} ({}){UNVERIFIABLE_REST}",
                        unverifiable.join(", ")
                    )
                } else {
                    format!("{UNCHANGED_PREFIX} ({}){UNCHANGED_REST}", named.join(", "))
                };
                Some(held)
            }
        }
    }
}

/// The legacy reading of a step path in a bound session.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LegacyRead {
    /// The legacy resolver does not reach a file.
    Absent,
    /// Not read: a special file, a bound target under another spelling, or
    /// nothing safe to read on this platform.
    Unknown(&'static str),
    /// The bytes of a regular file that is no bound target.
    Bytes(Vec<u8>),
}

/// The step path the legacy resolver reached, read in a bound session: an
/// `lstat` refuses a special file before any open; the open walks from the
/// workspace root without following a link and does not block
/// (`tools::dir_fd::open_read_file`); every bound name must be shown not to
/// be the opened file both before and after the read; and the read stops at
/// the file bound plus one probe byte (over it, unknown).
#[cfg(unix)]
fn read_unless_bound(root: &Path, resolved: &Path, names: &[&Name]) -> LegacyRead {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt;
    let Ok(canonical_root) = root.canonicalize() else {
        return LegacyRead::Unknown("unreadable");
    };
    let Ok(relative) = resolved.strip_prefix(&canonical_root) else {
        return LegacyRead::Unknown("escape");
    };
    match std::fs::symlink_metadata(resolved) {
        Ok(meta) if meta.file_type().is_file() => {}
        Ok(_) => return LegacyRead::Unknown("special_file"),
        Err(err) if err.kind() == ErrorKind::NotFound => return LegacyRead::Absent,
        Err(_) => return LegacyRead::Unknown("unreadable"),
    }
    let Ok(file) = crate::tools::dir_fd::open_read_file(&canonical_root, relative) else {
        return LegacyRead::Unknown("replaced");
    };
    let Ok(opened) = file.metadata() else {
        return LegacyRead::Unknown("unreadable");
    };
    if !opened.is_file() {
        return LegacyRead::Unknown("special_file");
    }
    let opened = (opened.dev(), opened.ino());
    #[cfg(test)]
    tests::alias_seam(resolved);
    if !names.iter().all(|name| name.excludes(root, opened)) {
        return LegacyRead::Unknown("alias");
    }
    let mut bytes = Vec::new();
    let read = (&file).take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes);
    #[cfg(test)]
    tests::LEGACY_BYTES.with(|total| total.set(total.get() + bytes.len() as u64));
    if read.is_err() {
        return LegacyRead::Unknown("unreadable");
    }
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return LegacyRead::Unknown("over_limit");
    }
    if !names.iter().all(|name| name.excludes(root, opened)) {
        return LegacyRead::Unknown("alias");
    }
    LegacyRead::Bytes(bytes)
}

#[cfg(not(unix))]
fn read_unless_bound(_root: &Path, _resolved: &Path, _names: &[&Name]) -> LegacyRead {
    LegacyRead::Unknown("unsupported_platform")
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
fn read_one(root: &Path, name: &str, cap: u64, keep: bool) -> (TargetRead, u64) {
    let Some(relative) = identity(name) else {
        return (unknown("escape"), 0);
    };
    match classify(root, &relative) {
        Ok(Some(leaf)) => read_regular(root, &relative, &leaf, cap, keep),
        Ok(None) => {
            let absent = TargetRead {
                fingerprint: Fingerprint::Absent,
                bytes: None,
            };
            (absent, 0)
        }
        Err(reason) => (unknown(reason), 0),
    }
}

/// The `lstat` of the leaf, before any open: `Ok(None)` is an absent target,
/// and every credential path, escape, symlink, and special file is refused.
fn classify(root: &Path, relative: &Path) -> Result<Option<std::fs::Metadata>, &'static str> {
    match sensitive_skip(root, &root.join(relative)) {
        Some(SkippedPath::Credential) => return Err("sensitive"),
        Some(SkippedPath::Outside) => return Err("escape"),
        None => {}
    }
    let mut cursor = root.to_path_buf();
    let count = relative.components().count();
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
            return Ok(Some(meta));
        } else {
            return Err("special_file");
        }
    }
    Err("escape")
}

#[cfg(unix)]
fn read_regular(
    root: &Path,
    relative: &Path,
    leaf: &std::fs::Metadata,
    cap: u64,
    keep: bool,
) -> (TargetRead, u64) {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt;
    let same =
        |a: &std::fs::Metadata, b: &std::fs::Metadata| a.dev() == b.dev() && a.ino() == b.ino();
    let Ok(file) = crate::tools::dir_fd::open_read_file(root, relative) else {
        return (unknown("replaced"), 0);
    };
    let Ok(before) = file.metadata() else {
        return (unknown("unreadable"), 0);
    };
    if !before.is_file() || !same(leaf, &before) {
        return (unknown("replaced"), 0);
    }
    #[cfg(test)]
    tests::read_seam(relative);
    let mut bytes = Vec::new();
    if (&file).take(cap + 1).read_to_end(&mut bytes).is_err() {
        let read = bytes.len() as u64;
        return (unknown("unreadable"), read);
    }
    let read = bytes.len() as u64;
    if read > cap {
        return (unknown("over_limit"), read);
    }
    let stable = file.metadata().is_ok_and(|after| {
        before.len() == read
            && after.len() == read
            && after.modified().ok() == before.modified().ok()
    });
    if !stable {
        return (unknown("changed_during_read"), read);
    }
    let still = crate::tools::dir_fd::open_read_file(root, relative)
        .and_then(|again| Ok(again.metadata()?))
        .is_ok_and(|again| same(&before, &again));
    if !still {
        return (unknown("replaced"), read);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let item = TargetRead {
        fingerprint: Fingerprint::Digest(digest),
        bytes: keep.then_some(bytes),
    };
    (item, read)
}

/// Without the Unix fd walk no read is safe enough to adopt.
#[cfg(not(unix))]
fn read_regular(
    _root: &Path,
    _relative: &Path,
    _leaf: &std::fs::Metadata,
    _cap: u64,
    _keep: bool,
) -> (TargetRead, u64) {
    (unknown("unsupported_platform"), 0)
}
