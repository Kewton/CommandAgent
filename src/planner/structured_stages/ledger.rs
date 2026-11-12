//! The durable side of the state machine (Issue #975, G4).
//!
//! The authority is the [`StageLedger`] held in this process, one per workspace and profile, so
//! a new session or a repair meets the same counts. A session ID changes inside one plan and is
//! not part of the key.
//!
//! A file below `state_dir`, outside the workspace, carries it to a later process. Every change
//! reaches the disk before it takes effect here, so an attempt is counted before its process
//! starts and an intent exists before its rename. A ledger that should exist and does not, or
//! that cannot be read, is a refusal: a count is never rebuilt, and never starts again at zero
//! on a resume.
//!
//! A fresh start replaces an earlier run's file only at its first `begin`. Opening alone, as a
//! preflight or a probe does, changes no byte of it, and a resume before that `begin` goes on
//! with the earlier run. A record a secret value ended is replaced only by `--fresh-session`.
//!
//! `flock` on an open descriptor refuses a second holder, in another process or through another
//! descriptor of this one, and goes away with its holder. A stale lock file cannot exist. A
//! process that dies with a child between fork and exec leaves the lock for that moment, and a
//! resume in that moment is refused rather than waiting.
//!
//! [`StageLedger::verdict`] is the only judgment. It compares real bytes, and nothing else
//! answers whether obligations are met.

use std::collections::BTreeMap;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use uuid::Uuid;

use super::contract::{StageContract, TargetKey};
use super::receipt::{CheckResult, sha256_hex};
use super::resume::{
    AdmittedPins, ResumeFacts, SavedRefusal, absorb_diverged, lower_saved, validate_saved,
};
use crate::tools::os_sandbox::SandboxPolicy;

use super::state::{
    AttemptCounts, AttemptFailure, AttemptIdentity, AttemptRequest, AttemptTicket, BeginDecision,
    BytesSource, CommitDecision, CommitObservation, FailureDecision, LocalDecision,
    MachineSnapshot, ObservedBytes, StageMachine, StageRefusal, StageVerdict,
};

const LEDGER_LEAF: &str = "structured-stages";
const LEDGER_FILE: &str = "ledger.json";
const LEDGER_TEMPORARY: &str = "ledger.json.tmp";
const LOCK_FILE: &str = "lock";
const MAX_LEDGER_BYTES: u64 = 8 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerEntry {
    /// A new process that is not a resume. Only here and below do the counts start over.
    Fresh,
    /// `--fresh-session`: the operator asked to start over. Nothing else removes a secret's mark.
    FreshSession,
    /// `--resume`, or `/resume` in a new process. The ledger must be there.
    Resume,
}

impl LedgerEntry {
    /// The same reading as the session store: a fresh session wins over a resume. `/resume`
    /// reaches here too, by setting `resume` on the config it runs the plan with.
    pub(crate) fn of_config(config: &crate::config::Config) -> Self {
        if config.fresh_session {
            Self::FreshSession
        } else if config.resume.is_some() {
            Self::Resume
        } else {
            Self::Fresh
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LedgerRefusal {
    Missing,
    Corrupt,
    Held,
    ContractChanged,
    /// Only a fresh session goes on from here. Names the input, not its content.
    FixedInputChanged {
        path: String,
    },
    StateDirInsideWorkspace,
    /// A dependency cache, which the OS boundary leaves commands free to write.
    StateDirWritableByCommands,
    /// The file is not what this process last wrote.
    Diverged,
    /// The record says a secret value ended its run, and this start did not ask to replace it.
    EndedBySecretValue,
    Unavailable,
}

impl LedgerRefusal {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Corrupt => "corrupt",
            Self::Held => "held",
            Self::ContractChanged => "contract_changed",
            Self::FixedInputChanged { .. } => "fixed_input_changed",
            Self::StateDirInsideWorkspace => "state_dir_inside_workspace",
            Self::StateDirWritableByCommands => "state_dir_writable_by_commands",
            Self::Diverged => "diverged",
            Self::EndedBySecretValue => "ended_by_secret_value",
            Self::Unavailable => "unavailable",
        }
    }

    /// Fixed words, safe for a message.
    pub(crate) const fn recovery(&self) -> &'static str {
        match self {
            Self::Missing => "A resume needs the run's record; `--fresh-session` starts over",
            Self::Corrupt => {
                "It stays in place to be looked at; move it away and start a new session"
            }
            Self::Held => "Another process holds it; wait for that process to end",
            Self::ContractChanged => {
                "The declared stages are not the ones it was written under; `--fresh-session` starts over"
            }
            Self::FixedInputChanged { .. } => {
                "A fixed input is not what the run pinned; `--fresh-session`, or any new process that does not resume, starts over"
            }
            Self::EndedBySecretValue => {
                "A secret value reached an output in the run it holds; only `--fresh-session` starts over"
            }
            Self::StateDirInsideWorkspace | Self::StateDirWritableByCommands => {
                "Commands of this run can write there; choose a state directory they cannot"
            }
            Self::Diverged => {
                "Something other than this process changed it; `--fresh-session` in a new process starts over"
            }
            Self::Unavailable => "It cannot be read or written; check the state directory",
        }
    }
}

fn unavailable(_: std::io::Error) -> LedgerRefusal {
    LedgerRefusal::Unavailable
}

impl From<SavedRefusal> for LedgerRefusal {
    fn from(refusal: SavedRefusal) -> Self {
        match refusal {
            SavedRefusal::ContractChanged => Self::ContractChanged,
            SavedRefusal::FixedInputChanged { path } => Self::FixedInputChanged { path },
            SavedRefusal::WrongSchema
            | SavedRefusal::TargetsDiffer
            | SavedRefusal::CountsInconsistent
            | SavedRefusal::PinOutsideContract => Self::Corrupt,
        }
    }
}

/// The path with what does not exist yet still on it, its existing part resolved. A tail that
/// is not plain names is refused: it could lead anywhere once created.
fn resolved_with_missing_tail(path: &Path) -> Result<PathBuf, LedgerRefusal> {
    let mut tail = Vec::new();
    let mut current = path;
    loop {
        match fs::canonicalize(current) {
            Ok(mut resolved) => {
                for part in tail.iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                // A link that points at nothing is not a name yet to be made: it leads wherever
                // its target is created.
                if fs::symlink_metadata(current).is_ok() {
                    return Err(LedgerRefusal::Unavailable);
                }
                let name = match current.components().next_back() {
                    Some(Component::Normal(name)) => name.to_os_string(),
                    _ => return Err(LedgerRefusal::Unavailable),
                };
                tail.push(name);
                current = current.parent().ok_or(LedgerRefusal::Unavailable)?;
                if current.as_os_str().is_empty() {
                    current = Path::new(".");
                }
            }
            Err(_) => return Err(LedgerRefusal::Unavailable),
        }
    }
}

/// Bash can write anywhere in the workspace, so a ledger there would prove nothing (D5).
pub(crate) fn refuse_state_dir_inside_workspace(
    state_dir: &Path,
    workspace: &Path,
) -> Result<(), LedgerRefusal> {
    // The caches this builds are the ones the installed policy resolved from the same home. The
    // run's own temporary is `<state_dir>/sandbox/run-*`, which holds no ledger, and no accessor
    // gives the installed value.
    refuse_state_dir_under_policy(
        state_dir,
        workspace,
        &SandboxPolicy::new(workspace, workspace),
    )
}

/// Every place the OS boundary lets a command write, by where the path really leads.
fn refuse_state_dir_under_policy(
    state_dir: &Path,
    workspace: &Path,
    policy: &SandboxPolicy,
) -> Result<(), LedgerRefusal> {
    let workspace = fs::canonicalize(workspace).map_err(unavailable)?;
    let resolved = resolved_with_missing_tail(state_dir)?;
    if resolved.starts_with(&workspace) {
        return Err(LedgerRefusal::StateDirInsideWorkspace);
    }
    let caches = &policy.caches;
    let writable = [
        &caches.cargo_registry,
        &caches.cargo_git,
        &caches.cargo_package_cache,
        &caches.npm,
        &caches.pip,
        &caches.playwright,
    ];
    for root in writable.into_iter().flatten() {
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        if resolved.starts_with(&root) {
            return Err(LedgerRefusal::StateDirWritableByCommands);
        }
    }
    Ok(())
}

/// The one directory of a workspace and profile under a state directory.
fn ledger_directory(state_dir: &Path, canonical_workspace: &Path, profile: &str) -> PathBuf {
    let mut identity = canonical_workspace.as_os_str().as_bytes().to_vec();
    identity.push(0);
    identity.extend_from_slice(profile.as_bytes());
    state_dir.join(LEDGER_LEAF).join(sha256_hex(&identity))
}

/// Where a refusal tells the operator to look.
pub(crate) fn ledger_file(state_dir: &Path, workspace: &Path, profile: &str) -> Option<PathBuf> {
    let workspace = fs::canonicalize(workspace).ok()?;
    Some(ledger_directory(state_dir, &workspace, profile).join(LEDGER_FILE))
}

#[derive(Serialize)]
struct EnvelopeOut<'a> {
    sha256: &'a str,
    body: &'a RawValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvelopeIn {
    sha256: String,
    body: Box<RawValue>,
}

fn encode(snapshot: &MachineSnapshot) -> Result<Vec<u8>, LedgerRefusal> {
    let body = serde_json::to_string(snapshot).map_err(|_| LedgerRefusal::Unavailable)?;
    let sha256 = sha256_hex(body.as_bytes());
    let body = RawValue::from_string(body).map_err(|_| LedgerRefusal::Unavailable)?;
    serde_json::to_vec(&EnvelopeOut {
        sha256: &sha256,
        body: &body,
    })
    .map_err(|_| LedgerRefusal::Unavailable)
}

/// Finds damage, not an adversary: an OS boundary is what keeps others off this file (D6).
fn decode(bytes: &[u8]) -> Result<MachineSnapshot, LedgerRefusal> {
    let envelope: EnvelopeIn = serde_json::from_slice(bytes).map_err(|_| LedgerRefusal::Corrupt)?;
    if sha256_hex(envelope.body.get().as_bytes()) != envelope.sha256 {
        return Err(LedgerRefusal::Corrupt);
    }
    serde_json::from_str(envelope.body.get()).map_err(|_| LedgerRefusal::Corrupt)
}

/// One ledger directory, its lock held for as long as this value lives.
#[derive(Debug)]
struct Store {
    directory: PathBuf,
    lock: File,
    /// What the file should hold now: what this process last wrote or, before its first write,
    /// what it found. `None` only when nothing should be there yet.
    expected_sha256: Option<String>,
}

impl Drop for Store {
    /// Closing is not enough: a child forked a moment ago has a copy of the descriptor until it
    /// execs, and the lock would last as long. An explicit unlock holds for every copy.
    fn drop(&mut self) {
        // SAFETY: `lock` is still open here, and flock only uses its descriptor.
        let _ = unsafe { libc::flock(self.lock.as_raw_fd(), libc::LOCK_UN) };
    }
}

impl Store {
    fn open(directory: PathBuf) -> Result<Self, LedgerRefusal> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&directory)
            .map_err(unavailable)?;
        let metadata = fs::symlink_metadata(&directory).map_err(unavailable)?;
        if !metadata.is_dir() {
            return Err(LedgerRefusal::Unavailable);
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(directory.join(LOCK_FILE))
            .map_err(unavailable)?;
        // A lock on anything else, such as a FIFO, is not one every holder agrees on.
        if !lock.metadata().map_err(unavailable)?.is_file() {
            return Err(LedgerRefusal::Unavailable);
        }
        // SAFETY: `lock` is an open descriptor this function owns, and flock only uses it.
        // LOCK_NB makes it return at once instead of waiting for the other holder.
        let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            return Err(match error.raw_os_error() {
                Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => {
                    LedgerRefusal::Held
                }
                _ => LedgerRefusal::Unavailable,
            });
        }
        Ok(Self {
            directory,
            lock,
            expected_sha256: None,
        })
    }

    /// `None` only when nothing is there. A symlink, or something too large to be a ledger, is
    /// damage. `O_NONBLOCK` is for a FIFO in its place: the open returns, and the file-type check
    /// below refuses it, instead of waiting with every other entry behind this one.
    fn read_file(&self) -> Result<Option<Vec<u8>>, LedgerRefusal> {
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.directory.join(LEDGER_FILE));
        let file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                return Err(LedgerRefusal::Corrupt);
            }
            Err(error) => return Err(unavailable(error)),
        };
        let metadata = file.metadata().map_err(unavailable)?;
        if !metadata.is_file() {
            return Err(LedgerRefusal::Corrupt);
        }
        let mut bytes = Vec::new();
        file.take(MAX_LEDGER_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(unavailable)?;
        if bytes.len() as u64 > MAX_LEDGER_BYTES {
            return Err(LedgerRefusal::Corrupt);
        }
        Ok(Some(bytes))
    }

    /// Temporary, fsync, rename, fsync of the directory: on return it survives a crash, and a
    /// crash before that leaves the previous version whole. Only the lock holder writes, so one
    /// temporary name is enough and a crash cannot leave others behind. What the next read would
    /// call damage is not written at all.
    fn write(&mut self, snapshot: &MachineSnapshot) -> Result<(), LedgerRefusal> {
        let bytes = encode(snapshot)?;
        if bytes.len() as u64 > MAX_LEDGER_BYTES {
            return Err(LedgerRefusal::Unavailable);
        }
        let temporary = self.directory.join(LEDGER_TEMPORARY);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&temporary)
            .map_err(unavailable)?;
        file.write_all(&bytes).map_err(unavailable)?;
        file.sync_all().map_err(unavailable)?;
        drop(file);
        fs::rename(&temporary, self.directory.join(LEDGER_FILE)).map_err(unavailable)?;
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(unavailable)?;
        self.expected_sha256 = Some(sha256_hex(&bytes));
        Ok(())
    }

    fn holds_what_is_expected(&self, bytes: &[u8]) -> bool {
        self.expected_sha256.as_deref() == Some(sha256_hex(bytes).as_str())
    }

    /// Before anything is built on the state: the file is what this process left there or, on a
    /// fresh start not yet begun, what it found there.
    fn verify_unchanged(&self) -> Result<(), LedgerRefusal> {
        match self.read_file()? {
            None if self.expected_sha256.is_none() => Ok(()),
            None => Err(LedgerRefusal::Missing),
            Some(bytes) if self.holds_what_is_expected(&bytes) => Ok(()),
            Some(_) => Err(LedgerRefusal::Diverged),
        }
    }
}

/// The attempts and states of one workspace and profile. After any refusal of its file it
/// refuses everything: there is no way back to a state this process cannot vouch for.
#[derive(Debug)]
pub(crate) struct StageLedger {
    machine: StageMachine,
    store: Store,
    failed: Option<LedgerRefusal>,
    /// A fresh start that has not reached its first `begin`. The machine exists only here; the
    /// file still holds whatever the previous run left.
    fresh_not_begun: bool,
}

impl StageLedger {
    fn apply<R>(
        &mut self,
        change: impl FnOnce(&mut StageMachine) -> R,
    ) -> Result<R, LedgerRefusal> {
        self.apply_writing(false, change)
    }

    fn apply_writing<R>(
        &mut self,
        write_regardless: bool,
        change: impl FnOnce(&mut StageMachine) -> R,
    ) -> Result<R, LedgerRefusal> {
        if let Some(refusal) = &self.failed {
            return Err(refusal.clone());
        }
        if let Err(refusal) = self.store.verify_unchanged() {
            self.failed = Some(refusal.clone());
            return Err(refusal);
        }
        let mut next = self.machine.clone();
        let result = change(&mut next);
        if write_regardless || next.snapshot() != self.machine.snapshot() {
            if let Err(refusal) = self.store.write(next.snapshot()) {
                self.failed = Some(refusal.clone());
                return Err(refusal);
            }
            self.machine = next;
        }
        Ok(result)
    }

    /// The D7 table and the limits. On `Proceed`, the attempt is on disk before this returns. The
    /// first call of a fresh start is what replaces the previous run's file, whatever it answers.
    pub(crate) fn begin(
        &mut self,
        key: &TargetKey,
        request: AttemptRequest<'_>,
    ) -> Result<BeginDecision, LedgerRefusal> {
        let fresh = self.fresh_not_begun;
        let decision = self.apply_writing(fresh, |machine| {
            // The run's clock starts here, once. Opening was not a stage call.
            if fresh {
                machine.snapshot_mut().started_at_ms = request.now_ms;
            }
            machine.begin(key, request)
        })?;
        self.fresh_not_begun = false;
        Ok(decision)
    }

    pub(crate) fn record_local_result(
        &mut self,
        ticket: &AttemptTicket,
        candidate_sha256: &str,
        checks: Vec<CheckResult>,
        current_output: &ObservedBytes,
    ) -> Result<LocalDecision, LedgerRefusal> {
        self.apply(|machine| {
            machine.record_local_result(ticket, candidate_sha256, checks, current_output)
        })
    }

    pub(crate) fn record_failure(
        &mut self,
        ticket: &AttemptTicket,
        failure: AttemptFailure,
    ) -> Result<FailureDecision, LedgerRefusal> {
        self.apply(|machine| machine.record_failure(ticket, failure))
    }

    /// `Ok(Ok(()))` means the intent is durable. Reached through [`record_intent`], which is what
    /// hands out the way to answer it.
    fn record_intent(
        &mut self,
        ticket: &AttemptTicket,
        previous_sha256: Option<String>,
    ) -> Result<Result<(), StageRefusal>, LedgerRefusal> {
        self.apply(|machine| machine.record_intent(ticket, previous_sha256))
    }

    /// The compare again, right before the rename. It writes nothing, and a ledger that can no
    /// longer be vouched for stops the rename too.
    pub(crate) fn cas_check(
        &mut self,
        ticket: &AttemptTicket,
    ) -> Result<Result<(), StageRefusal>, LedgerRefusal> {
        self.apply(|machine| machine.cas_check(ticket))
    }

    /// Reached through a [`CommitPermit`], so no intent goes unanswered.
    fn record_commit(
        &mut self,
        ticket: &AttemptTicket,
        observation: CommitObservation,
    ) -> Result<Result<CommitDecision, StageRefusal>, LedgerRefusal> {
        self.apply(|machine| machine.record_commit(ticket, observation))
    }

    /// Whether the obligations are met, by the bytes there now (S1, S5). This is the only
    /// judgment: no other reader of this module can say a stage is complete. A ledger that
    /// cannot be vouched for answers no, and says so.
    pub(crate) fn verdict(&mut self, current_bytes: &dyn BytesSource) -> StageVerdict {
        match self.apply(|machine| machine.judge(current_bytes)) {
            Ok(verdict) => verdict,
            Err(_) => self.machine.verdict_without_ledger(),
        }
    }

    /// What a later budget attaches a cost to. It reads and changes nothing.
    pub(crate) fn attempt_identity(&self, key: &TargetKey) -> Option<AttemptIdentity> {
        self.machine.attempt_identity(key)
    }

    pub(crate) fn revision_of(&self, key: &TargetKey) -> Option<String> {
        self.machine.revision_of(key)
    }

    /// What a model is told of G4's counts (Issue #1004). It reads and changes nothing.
    pub(crate) fn attempt_counts(&self, key: &TargetKey) -> Option<AttemptCounts> {
        self.machine.attempt_counts(key)
    }

    #[cfg(test)]
    pub(crate) fn contract(&self) -> &Arc<StageContract> {
        self.machine.contract()
    }

    #[cfg(test)]
    pub(crate) fn started_at_ms(&self) -> u64 {
        self.machine.snapshot().started_at_ms
    }

    /// Taken again by a later session in this process. A file changed under us loses the
    /// states and gives no count back; a file gone or damaged is a refusal.
    fn reconcile(&mut self, request: &LedgerRequest<'_>) -> Result<(), LedgerRefusal> {
        if let Some(refusal) = &self.failed {
            return Err(refusal.clone());
        }
        if self.machine.contract().sha256() != request.contract.sha256() {
            return Err(LedgerRefusal::ContractChanged);
        }
        if self.fresh_not_begun && request.entry == LedgerEntry::Resume {
            return self.take_up_resume(request);
        }
        let outcome = self.reconcile_with_file();
        if let Err(refusal) = &outcome {
            self.failed = Some(refusal.clone());
        }
        outcome
    }

    /// A resume reaching a fresh start that has not begun: nothing of that start is on disk, so
    /// the previous run is still there to go on with, through the same checks as in a new process.
    /// What those checks refuse changes nothing here, and the operator can put it right and ask
    /// again. A file that is no longer the one found stops the ledger, as in any later session.
    fn take_up_resume(&mut self, request: &LedgerRequest<'_>) -> Result<(), LedgerRefusal> {
        let bytes = match self.store.read_file() {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Err(LedgerRefusal::Missing),
            Err(refusal) => return Err(self.stop(refusal)),
        };
        if !self.store.holds_what_is_expected(&bytes) {
            return Err(self.stop(LedgerRefusal::Diverged));
        }
        let machine = lowered_for_resume(&bytes, request)?;
        if let Err(refusal) = self.store.write(machine.snapshot()) {
            return Err(self.stop(refusal));
        }
        self.machine = machine;
        self.fresh_not_begun = false;
        Ok(())
    }

    fn stop(&mut self, refusal: LedgerRefusal) -> LedgerRefusal {
        self.failed = Some(refusal.clone());
        refusal
    }

    fn reconcile_with_file(&mut self) -> Result<(), LedgerRefusal> {
        let Some(bytes) = self.store.read_file()? else {
            return if self.store.expected_sha256.is_none() {
                Ok(())
            } else {
                Err(LedgerRefusal::Missing)
            };
        };
        if self.store.holds_what_is_expected(&bytes) {
            return Ok(());
        }
        // Not begun, there is no run of this process to take counts into.
        if self.fresh_not_begun {
            return Err(LedgerRefusal::Diverged);
        }
        let disk = decode(&bytes)?;
        validate_saved(&disk, self.machine.contract())?;
        if disk.run_nonce != self.machine.snapshot().run_nonce {
            return Err(LedgerRefusal::Corrupt);
        }
        let mut next = self.machine.clone();
        absorb_diverged(next.snapshot_mut(), &disk);
        self.store.write(next.snapshot())?;
        self.machine = next;
        Ok(())
    }
}

/// What it takes to get the ledger of one workspace and profile.
pub(crate) struct LedgerRequest<'a> {
    pub(crate) state_dir: &'a Path,
    pub(crate) workspace: &'a Path,
    pub(crate) profile: &'a str,
    pub(crate) entry: LedgerEntry,
    pub(crate) contract: Arc<StageContract>,
    pub(crate) now_ms: u64,
    /// Read only when another process's record is being lowered.
    pub(crate) bytes: &'a dyn BytesSource,
    pub(crate) pins: &'a BTreeMap<String, AdmittedPins>,
}

fn new_nonce() -> String {
    Uuid::now_v7().to_string()
}

/// Every resume's checks, whichever way it comes. It writes nothing.
fn lowered_for_resume(
    bytes: &[u8],
    request: &LedgerRequest<'_>,
) -> Result<StageMachine, LedgerRefusal> {
    Ok(lower_saved(
        decode(bytes)?,
        &request.contract,
        &ResumeFacts {
            process_nonce: new_nonce(),
            now_ms: request.now_ms,
            bytes: request.bytes,
            pins: request.pins,
        },
    )?)
}

fn open_new(directory: PathBuf, request: &LedgerRequest<'_>) -> Result<StageLedger, LedgerRefusal> {
    let mut store = Store::open(directory)?;
    let existing = store.read_file()?;
    match (request.entry, existing) {
        (LedgerEntry::Resume, None) => Err(LedgerRefusal::Missing),
        (LedgerEntry::Resume, Some(bytes)) => {
            let machine = lowered_for_resume(&bytes, request)?;
            store.write(machine.snapshot())?;
            Ok(StageLedger {
                machine,
                store,
                failed: None,
                fresh_not_begun: false,
            })
        }
        (entry, existing) => {
            // Starting over is the operator's to choose, on a record that can be read. A
            // damaged one stays until the operator has looked at it. A run a secret ended stays
            // until they say so: only being started again would let the value be compared again.
            if let Some(bytes) = existing {
                let saved = decode(&bytes)?;
                if saved.ended_by_secret_value && entry != LedgerEntry::FreshSession {
                    return Err(LedgerRefusal::EndedBySecretValue);
                }
                store.expected_sha256 = Some(sha256_hex(&bytes));
            }
            Ok(StageLedger {
                machine: StageMachine::new(
                    request.contract.clone(),
                    new_nonce(),
                    new_nonce(),
                    request.now_ms,
                ),
                store,
                failed: None,
                fresh_not_begun: true,
            })
        }
    }
}

pub(crate) type HeldLedger = Arc<Mutex<StageLedger>>;

/// The one thing that lets a commit go ahead, and the one way to report how it went. Dropped
/// unanswered, by an early return or by a panic on any path past the intent, it records
/// [`CommitObservation::OutcomeUnknown`]: a rename may have happened and nothing said otherwise.
#[must_use = "dropping this records the commit's outcome as unknown"]
#[derive(Debug)]
pub(crate) struct CommitPermit {
    ledger: HeldLedger,
    ticket: AttemptTicket,
    answered: bool,
}

/// The intent, durable before this returns. The rename may go ahead only on a permit.
pub(crate) fn record_intent(
    ledger: &HeldLedger,
    ticket: &AttemptTicket,
    previous_sha256: Option<String>,
) -> Result<Result<CommitPermit, StageRefusal>, LedgerRefusal> {
    let recorded = ledger
        .lock()
        .map_err(|_| LedgerRefusal::Unavailable)?
        .record_intent(ticket, previous_sha256)?;
    Ok(recorded.map(|()| CommitPermit {
        ledger: ledger.clone(),
        ticket: ticket.clone(),
        answered: false,
    }))
}

impl CommitPermit {
    pub(crate) fn answer(
        mut self,
        observation: CommitObservation,
    ) -> Result<Result<CommitDecision, StageRefusal>, LedgerRefusal> {
        self.answered = true;
        self.record(observation)
    }

    /// A poisoned lock means something panicked while holding the state, so this live ledger
    /// can vouch for nothing from here on. The intent is on disk, and a new process judges it by
    /// the bytes.
    fn record(
        &self,
        observation: CommitObservation,
    ) -> Result<Result<CommitDecision, StageRefusal>, LedgerRefusal> {
        match self.ledger.lock() {
            Ok(mut ledger) => ledger.record_commit(&self.ticket, observation),
            Err(poisoned) => {
                poisoned
                    .into_inner()
                    .failed
                    .get_or_insert(LedgerRefusal::Unavailable);
                Err(LedgerRefusal::Unavailable)
            }
        }
    }
}

impl Drop for CommitPermit {
    /// Never panics. A write that fails here has already set the ledger's `failed`, and from then
    /// on it refuses everything.
    fn drop(&mut self) {
        if !self.answered {
            let _ = self.record(CommitObservation::OutcomeUnknown);
        }
    }
}

/// What is at a workspace-relative path now, reached with no symlink.
#[derive(Debug, Clone, Copy)]
pub(crate) struct WorkspaceBytes<'a> {
    root: &'a Path,
}

impl<'a> WorkspaceBytes<'a> {
    pub(crate) const fn new(root: &'a Path) -> Self {
        Self { root }
    }
}

impl BytesSource for WorkspaceBytes<'_> {
    fn observe(&self, path: &str) -> ObservedBytes {
        let file = match crate::tools::dir_fd::open_read_file(self.root, Path::new(path)) {
            Ok(file) => file,
            Err(error) => {
                let nothing_there = error
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|error| error.kind() == ErrorKind::NotFound);
                return if nothing_there {
                    ObservedBytes::Absent
                } else {
                    ObservedBytes::Unreadable
                };
            }
        };
        if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
            return ObservedBytes::Unreadable;
        }
        let mut bytes = Vec::new();
        match file.take(MAX_LEDGER_BYTES + 1).read_to_end(&mut bytes) {
            Ok(_) if bytes.len() as u64 <= MAX_LEDGER_BYTES => ObservedBytes::Present(bytes),
            _ => ObservedBytes::Unreadable,
        }
    }
}

/// The ledgers one process holds. Tests make their own, to stand for separate processes.
#[derive(Debug, Default)]
pub(crate) struct LedgerRegistry {
    held: Mutex<BTreeMap<(PathBuf, String), HeldLedger>>,
}

impl LedgerRegistry {
    /// Whatever the entry says, a ledger this process already holds is the one returned: only a
    /// new process starts anything over (D10). A resume does move a fresh start that has not
    /// begun onto the previous run, since going on with that run is what it asks for.
    pub(crate) fn open(&self, request: &LedgerRequest<'_>) -> Result<HeldLedger, LedgerRefusal> {
        let workspace = fs::canonicalize(request.workspace).map_err(unavailable)?;
        let directory = ledger_directory(request.state_dir, &workspace, request.profile);
        // The directory itself, past any link on the way down to it.
        refuse_state_dir_inside_workspace(&directory, request.workspace)?;
        let mut held = self.held.lock().map_err(|_| LedgerRefusal::Unavailable)?;
        let key = (workspace, request.profile.to_string());
        if let Some(existing) = held.get(&key) {
            existing
                .lock()
                .map_err(|_| LedgerRefusal::Unavailable)?
                .reconcile(request)?;
            return Ok(existing.clone());
        }
        let ledger = Arc::new(Mutex::new(open_new(directory, request)?));
        held.insert(key, ledger.clone());
        Ok(ledger)
    }
}

pub(crate) fn process_ledgers() -> Arc<LedgerRegistry> {
    static LEDGERS: OnceLock<Arc<LedgerRegistry>> = OnceLock::new();
    LEDGERS.get_or_init(Arc::default).clone()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex, PoisonError};

    use super::super::contract::{
        NarrowedStageLimits, StageContract, StageSetDeclaration, TargetKey,
    };
    use super::super::receipt::{InputDigest, sha256_hex};
    use super::super::resume::AdmittedPins;
    use super::super::state::{
        AttemptTicket, BeginDecision, BytesSource, CommitDecision, CommitObservation,
        LocalDecision, MachineSnapshot, ObservedBytes, Phase, StageMachine, StageRefusal,
        UnverifiedReason,
    };
    use super::super::test_support::{
        FakeBytes, admit_enforced as admit, commit_for, facts, key, one_target, passed_checks,
        pins_for, request_for, schema_for, sha_of, two_stage_contract,
    };
    use super::{
        HeldLedger, LEDGER_FILE, LEDGER_LEAF, LEDGER_TEMPORARY, LOCK_FILE, LedgerEntry,
        LedgerRefusal, LedgerRegistry, LedgerRequest, StageLedger, Store, WorkspaceBytes, decode,
        encode, process_ledgers, record_intent, refuse_state_dir_inside_workspace,
        refuse_state_dir_under_policy,
    };
    use crate::tools::os_sandbox::SandboxPolicy;

    const NOW: u64 = 9_000;

    struct World {
        // Held so the directories outlive the test.
        _root: tempfile::TempDir,
        workspace: PathBuf,
        state_dir: PathBuf,
        contract: Arc<StageContract>,
        pins: BTreeMap<String, AdmittedPins>,
        bytes: FakeBytes,
    }

    fn pins_of(contract: &StageContract) -> BTreeMap<String, AdmittedPins> {
        contract
            .targets()
            .map(|target| {
                (
                    target.tool_id.clone(),
                    AdmittedPins {
                        pins: pins_for(&target.tool_id),
                        schema_sha256: schema_for(&target.tool_id),
                    },
                )
            })
            .collect()
    }

    fn world() -> World {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let state_dir = root.path().join("state");
        let contract = Arc::new(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        World {
            workspace,
            state_dir,
            pins: pins_of(&contract),
            contract,
            bytes,
            _root: root,
        }
    }

    impl World {
        fn request(&self, entry: LedgerEntry) -> LedgerRequest<'_> {
            LedgerRequest {
                state_dir: &self.state_dir,
                workspace: &self.workspace,
                profile: "synthetic",
                entry,
                contract: self.contract.clone(),
                now_ms: NOW,
                bytes: &self.bytes,
                pins: &self.pins,
            }
        }

        fn directory(&self) -> PathBuf {
            let parent = self.state_dir.join(LEDGER_LEAF);
            let mut entries = fs::read_dir(parent)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            assert_eq!(entries.len(), 1);
            entries.pop().unwrap()
        }

        fn file(&self) -> PathBuf {
            self.directory().join(LEDGER_FILE)
        }

        fn saved(&self) -> MachineSnapshot {
            decode(&fs::read(self.file()).unwrap()).unwrap()
        }

        fn save(&self, snapshot: &MachineSnapshot) {
            fs::write(self.file(), encode(snapshot).unwrap()).unwrap();
        }

        /// Stands for a new process: its own registry, and its own lock when it opens one.
        fn process(&self, entry: LedgerEntry) -> Result<Arc<Mutex<StageLedger>>, LedgerRefusal> {
            LedgerRegistry::default().open(&self.request(entry))
        }
    }

    fn alpha() -> TargetKey {
        key("alpha", "main")
    }

    fn beta() -> TargetKey {
        key("beta", "main")
    }

    fn start(
        ledger: &mut StageLedger,
        target: &TargetKey,
        bytes: &FakeBytes,
        turn: u64,
        arguments: &str,
    ) -> AttemptTicket {
        let contract = ledger.contract().clone();
        let path = contract.target(target).unwrap().output_path.clone();
        let revision = ledger.revision_of(target).unwrap();
        let output = bytes.observe(&path);
        let request = request_for(
            &contract, target, &revision, bytes, turn, NOW, arguments, &output,
        );
        match ledger.begin(target, request).unwrap() {
            BeginDecision::Proceed(ticket) => ticket,
            other => panic!("expected to proceed, got {other:?}"),
        }
    }

    /// Up to the durable intent, for a candidate with these bytes.
    fn to_intent(
        ledger: &mut StageLedger,
        target: &TargetKey,
        content: &[u8],
        bytes: &FakeBytes,
        turn: u64,
        arguments: &str,
    ) -> AttemptTicket {
        let contract = ledger.contract().clone();
        let path = contract.target(target).unwrap().output_path.clone();
        let ticket = start(ledger, target, bytes, turn, arguments);
        let output = bytes.observe(&path);
        assert_eq!(
            ledger
                .record_local_result(
                    &ticket,
                    &sha_of(content),
                    passed_checks(&contract, target),
                    &output
                )
                .unwrap(),
            LocalDecision::ReadyToCommit
        );
        ledger
            .record_intent(&ticket, output.sha256())
            .unwrap()
            .unwrap();
        ticket
    }

    fn seal(
        ledger: &mut StageLedger,
        target: &TargetKey,
        content: &[u8],
        bytes: &mut FakeBytes,
        turn: u64,
        arguments: &str,
    ) {
        let contract = ledger.contract().clone();
        let admitted = contract.target(target).unwrap().clone();
        let request_inputs = request_for(
            &contract,
            target,
            "x",
            bytes,
            turn,
            NOW,
            arguments,
            &super::super::state::ObservedBytes::Absent,
        );
        let ticket = to_intent(ledger, target, content, bytes, turn, arguments);
        bytes.put(&admitted.output_path, content);
        let commit = commit_for(
            &contract,
            target,
            &request_inputs.pins,
            &request_inputs.schema_sha256,
            &request_inputs.input_revision,
            &sha_of(content),
        );
        let decision = ledger
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
            .unwrap()
            .unwrap();
        assert!(decision.is_progress(), "{decision:?}");
    }

    fn both_sealed(world: &mut World) -> Arc<Mutex<StageLedger>> {
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        {
            let mut ledger = held.lock().unwrap();
            let mut bytes = world.bytes.clone();
            seal(&mut ledger, &alpha(), b"alpha v1", &mut bytes, 1, "a");
            seal(&mut ledger, &beta(), b"beta v1", &mut bytes, 2, "b");
            world.bytes = bytes;
        }
        held
    }

    #[test]
    fn the_verdict_says_complete_only_for_sealed_obligations_on_the_current_bytes() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let mut ledger = held.lock().unwrap();
        let verdict = ledger.verdict(&world.bytes);
        assert!(verdict.complete());
        assert!(verdict.ledger_intact());

        let edited = world.bytes.with("out/alpha.json", b"edited");
        let verdict = ledger.verdict(&edited);
        assert!(!verdict.complete());
        assert_eq!(verdict.invalidated(), [alpha(), beta()]);
        // What the judgment lowered is on disk.
        let saved = world.saved();
        assert_eq!(saved.targets[0].phase, Phase::Unverified);
        assert_eq!(saved.targets[1].phase, Phase::NotReady);
    }

    #[test]
    fn an_attempt_is_on_disk_before_its_process_can_start() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        let mut ledger = held.lock().unwrap();
        start(&mut ledger, &alpha(), &world.bytes, 1, "a");
        let saved = world.saved();
        assert_eq!(saved.attempts_used, 1);
        assert_eq!(saved.targets[0].attempts_used, 1);
        assert!(saved.targets[0].in_flight.is_some());
    }

    #[test]
    fn a_later_session_in_this_process_gets_the_same_counts_whatever_it_says() {
        let world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            let again = registry.open(&world.request(entry)).unwrap();
            assert!(Arc::ptr_eq(&held, &again));
            assert_eq!(again.lock().unwrap().machine.snapshot().attempts_used, 1);
        }
    }

    #[test]
    fn a_session_id_is_no_part_of_the_key_but_workspace_and_profile_are() {
        let world = world();
        let registry = LedgerRegistry::default();
        let first = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        let mut other = world.request(LedgerEntry::Fresh);
        other.profile = "another";
        let second = registry.open(&other).unwrap();
        assert!(!Arc::ptr_eq(&first, &second));

        let elsewhere = world.workspace.parent().unwrap().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        let mut other = world.request(LedgerEntry::Fresh);
        other.workspace = &elsewhere;
        assert!(!Arc::ptr_eq(&first, &registry.open(&other).unwrap()));

        // Another spelling of the same workspace is the same ledger.
        let spelled = world.workspace.join(".");
        let mut same = world.request(LedgerEntry::Fresh);
        same.workspace = &spelled;
        assert!(Arc::ptr_eq(&first, &registry.open(&same).unwrap()));
    }

    #[test]
    fn a_resume_without_its_ledger_is_refused_and_writes_no_ledger() {
        let world = world();
        assert_eq!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Missing
        );
        let directory = world.directory();
        assert!(!directory.join(LEDGER_FILE).exists());
    }

    /// A child forked while the lock is held has a copy of the descriptor until it execs, and
    /// a lock lasts until every copy closes. `try_clone` makes such a copy without a child.
    #[test]
    fn a_copy_of_the_descriptor_in_a_forked_child_does_not_keep_the_lock_after_its_holder() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("ledger");
        let store = Store::open(directory.clone()).unwrap();
        let copy_in_a_child = store.lock.try_clone().unwrap();
        assert_eq!(
            Store::open(directory.clone()).unwrap_err(),
            LedgerRefusal::Held
        );
        drop(store);
        assert!(Store::open(directory).is_ok());
        drop(copy_in_a_child);
    }

    #[test]
    fn a_second_holder_is_refused_and_the_lock_goes_with_its_holder() {
        let world = world();
        let first = LedgerRegistry::default();
        let held = first.open(&world.request(LedgerEntry::Fresh)).unwrap();
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            assert_eq!(world.process(entry).unwrap_err(), LedgerRefusal::Held);
        }
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        drop(held);
        drop(first);
        assert!(world.directory().join(LOCK_FILE).exists());
        assert!(
            world.process(LedgerEntry::Resume).is_ok(),
            "a holder that is gone leaves nothing to clear by hand"
        );
    }

    #[test]
    fn a_new_process_that_is_not_a_resume_starts_over_under_a_new_run() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let old_run = held.lock().unwrap().machine.snapshot().run_nonce.clone();
        let old_ticket = {
            let mut ledger = held.lock().unwrap();
            let bytes = world.bytes.clone();
            start(&mut ledger, &alpha(), &bytes, 3, "a2")
        };
        drop(held);

        let held = world.process(LedgerEntry::Fresh).unwrap();
        let mut ledger = held.lock().unwrap();
        let snapshot = ledger.machine.snapshot().clone();
        assert_ne!(snapshot.run_nonce, old_run);
        assert_eq!(snapshot.attempts_used, 0);
        assert!(!ledger.verdict(&world.bytes).complete());
        // A result still on its way from the earlier run is another run's.
        assert_eq!(
            ledger.cas_check(&old_ticket).unwrap(),
            Err(StageRefusal::OtherRun)
        );
    }

    #[test]
    fn opening_fresh_leaves_the_previous_ledger_alone_until_the_first_begin() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let counts = held.lock().unwrap().machine.snapshot().attempts_used;
        drop(held);
        let previous = fs::read(world.file()).unwrap();

        // A process that only opens it, such as a preflight, and then ends.
        let held = world.process(LedgerEntry::Fresh).unwrap();
        assert!(fs::read(world.file()).unwrap() == previous, "after opening");
        drop(held);
        assert!(fs::read(world.file()).unwrap() == previous, "after ending");

        let resumed = world.process(LedgerEntry::Resume).unwrap();
        assert_eq!(
            resumed.lock().unwrap().machine.snapshot().attempts_used,
            counts,
            "the counts are still there for a resume"
        );
    }

    #[test]
    fn a_fresh_start_not_yet_begun_is_not_taken_for_a_changed_file() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let old_run = held.lock().unwrap().machine.snapshot().run_nonce.clone();
        drop(held);

        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        {
            let mut ledger = held.lock().unwrap();
            let verdict = ledger.verdict(&world.bytes);
            assert!(verdict.ledger_intact());
            assert!(!verdict.complete());
            assert_eq!(ledger.failed, None);
        }
        // A later session in this process that also starts fresh, before anything began.
        assert!(registry.open(&world.request(LedgerEntry::Fresh)).is_ok());

        let mut ledger = held.lock().unwrap();
        start(&mut ledger, &alpha(), &world.bytes, 1, "a");
        assert_eq!(ledger.failed, None);
        let saved = world.saved();
        assert_ne!(saved.run_nonce, old_run, "the first begin replaced it");
        assert_eq!(saved.attempts_used, 1);
        assert!(ledger.verdict(&world.bytes).ledger_intact());
    }

    #[test]
    fn a_resume_on_a_fresh_open_not_yet_begun_takes_up_the_previous_run() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let before = held.lock().unwrap().machine.snapshot().clone();
        drop(held);

        let registry = LedgerRegistry::default();
        let opened = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        assert_ne!(
            opened.lock().unwrap().machine.snapshot().run_nonce,
            before.run_nonce
        );

        let resumed = registry.open(&world.request(LedgerEntry::Resume)).unwrap();
        assert!(Arc::ptr_eq(&opened, &resumed));
        let after = resumed.lock().unwrap().machine.snapshot().clone();
        assert_eq!(after.run_nonce, before.run_nonce);
        assert_ne!(after.process_nonce, before.process_nonce);
        assert_eq!(after.attempts_used, before.attempts_used);
        assert_eq!(after.started_at_ms, before.started_at_ms);
        for (record, earlier) in after.targets.iter().zip(&before.targets) {
            assert_eq!(record.phase, Phase::Unverified, "{:?}", record.key);
            assert_eq!(record.unverified_reason, Some(UnverifiedReason::Resumed));
            assert_eq!(record.attempts_used, earlier.attempts_used);
            assert_eq!(record.generation, earlier.generation);
            assert!(record.current_receipt.is_none());
        }
        assert_eq!(world.saved(), after, "the lowering is durable");
        assert!(!resumed.lock().unwrap().fresh_not_begun);

        // From here it is a resume, whatever a later session in this process says (D10).
        let again = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        assert!(Arc::ptr_eq(&opened, &again));
        let mut ledger = again.lock().unwrap();
        assert_eq!(ledger.machine.snapshot(), &after);
        assert!(!ledger.verdict(&world.bytes).complete());
        start(&mut ledger, &alpha(), &world.bytes, 3, "a2");
        let saved = world.saved();
        assert_eq!(saved.run_nonce, before.run_nonce);
        assert_eq!(saved.attempts_used, before.attempts_used + 1);
    }

    #[test]
    fn a_refused_resume_on_a_fresh_open_changes_nothing_and_can_be_tried_again() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let before = held.lock().unwrap().machine.snapshot().clone();
        drop(held);
        let previous = fs::read(world.file()).unwrap();

        let registry = LedgerRegistry::default();
        let opened = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        let fresh = opened.lock().unwrap().machine.snapshot().clone();

        world
            .bytes
            .put("fixed/source.json", b"changed while the fresh open waited");
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::FixedInputChanged {
                path: "fixed/source.json".to_string()
            }
        );
        {
            let ledger = opened.lock().unwrap();
            assert_eq!(ledger.machine.snapshot(), &fresh);
            assert!(ledger.fresh_not_begun);
            assert_eq!(ledger.failed, None);
        }
        assert!(fs::read(world.file()).unwrap() == previous, "after refusal");

        // Going on as fresh, short of a begin, leaves the previous run as it was too.
        assert!(registry.open(&world.request(LedgerEntry::Fresh)).is_ok());
        assert!(opened.lock().unwrap().verdict(&world.bytes).ledger_intact());
        assert!(fs::read(world.file()).unwrap() == previous, "as fresh");

        world.bytes.put("fixed/source.json", b"fixed source");
        let resumed = registry.open(&world.request(LedgerEntry::Resume)).unwrap();
        let after = resumed.lock().unwrap().machine.snapshot().clone();
        assert_eq!(after.run_nonce, before.run_nonce);
        assert_eq!(after.attempts_used, before.attempts_used);
    }

    #[test]
    fn a_refused_resume_on_a_fresh_open_leaves_the_fresh_start_able_to_begin() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let old_run = held.lock().unwrap().machine.snapshot().run_nonce.clone();
        drop(held);

        let registry = LedgerRegistry::default();
        let opened = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        world.bytes.put(
            "fixed/source.json",
            b"only a fresh session goes on from here",
        );
        assert!(matches!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::FixedInputChanged { .. }
        ));
        let mut ledger = opened.lock().unwrap();
        start(&mut ledger, &alpha(), &world.bytes, 1, "a");
        assert_eq!(ledger.failed, None);
        let saved = world.saved();
        assert_ne!(saved.run_nonce, old_run);
        assert_eq!(saved.attempts_used, 1);
    }

    #[test]
    fn a_resume_on_a_fresh_open_with_no_file_is_missing_and_changes_nothing() {
        // Nothing was ever there.
        let world_empty = world();
        let registry = LedgerRegistry::default();
        let opened = registry
            .open(&world_empty.request(LedgerEntry::Fresh))
            .unwrap();
        assert_eq!(
            registry
                .open(&world_empty.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Missing
        );
        {
            let mut ledger = opened.lock().unwrap();
            assert!(ledger.fresh_not_begun);
            assert_eq!(ledger.failed, None);
            assert!(!world_empty.file().exists());
            start(&mut ledger, &alpha(), &world_empty.bytes, 1, "a");
            assert_eq!(world_empty.saved().attempts_used, 1);
        }

        // It was there when the fresh open found it, and is back in time for another try.
        let mut world = world();
        let held = both_sealed(&mut world);
        let before = held.lock().unwrap().machine.snapshot().clone();
        drop(held);
        let previous = fs::read(world.file()).unwrap();
        let registry = LedgerRegistry::default();
        let opened = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        fs::remove_file(world.file()).unwrap();
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Missing
        );
        assert_eq!(opened.lock().unwrap().failed, None);
        assert!(opened.lock().unwrap().fresh_not_begun);
        fs::write(world.file(), &previous).unwrap();
        let resumed = registry.open(&world.request(LedgerEntry::Resume)).unwrap();
        assert_eq!(
            resumed.lock().unwrap().machine.snapshot().run_nonce,
            before.run_nonce
        );
    }

    #[test]
    fn a_file_that_appears_or_changes_under_a_fresh_open_stops_a_resume_for_good() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let previous = fs::read(world.file()).unwrap();

        let registry = LedgerRegistry::default();
        let opened = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        let mut saved = world.saved();
        saved.attempts_used += 1;
        world.save(&saved);
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Diverged
        );
        assert_eq!(opened.lock().unwrap().failed, Some(LedgerRefusal::Diverged));
        fs::write(world.file(), &previous).unwrap();
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Diverged,
            "putting it back does not vouch for it"
        );

        // Nothing was there when the fresh open looked.
        let elsewhere = self::world();
        let registry = LedgerRegistry::default();
        registry
            .open(&elsewhere.request(LedgerEntry::Fresh))
            .unwrap();
        fs::write(elsewhere.file(), &previous).unwrap();
        assert_eq!(
            registry
                .open(&elsewhere.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Diverged
        );
    }

    #[test]
    fn a_resume_on_a_fresh_open_that_cannot_write_stops_the_ledger() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let previous = fs::read(world.file()).unwrap();

        let registry = LedgerRegistry::default();
        let opened = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        fs::create_dir(world.directory().join(LEDGER_TEMPORARY)).unwrap();
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Unavailable
        );
        assert!(fs::read(world.file()).unwrap() == previous);
        let mut ledger = opened.lock().unwrap();
        assert_eq!(ledger.failed, Some(LedgerRefusal::Unavailable));
        assert!(!ledger.verdict(&world.bytes).ledger_intact());
    }

    #[test]
    fn a_ledger_that_refused_its_file_stays_refused_when_the_file_comes_back() {
        let world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        let good = fs::read(world.file()).unwrap();

        fs::remove_file(world.file()).unwrap();
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Missing
        );
        fs::write(world.file(), &good).unwrap();
        for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
            assert_eq!(
                registry.open(&world.request(entry)).unwrap_err(),
                LedgerRefusal::Missing,
                "{entry:?}"
            );
        }
        assert!(!held.lock().unwrap().verdict(&world.bytes).ledger_intact());
    }

    /// A live ticket on `alpha`, up to the point its intent would be written.
    fn before_intent(world: &World, held: &HeldLedger) -> AttemptTicket {
        let mut ledger = held.lock().unwrap();
        let contract = ledger.contract().clone();
        let bytes = world.bytes.clone();
        let ticket = start(&mut ledger, &alpha(), &bytes, 3, "a2");
        let output = bytes.observe("out/alpha.json");
        assert_eq!(
            ledger
                .record_local_result(
                    &ticket,
                    &sha_of(b"alpha v2"),
                    passed_checks(&contract, &alpha()),
                    &output,
                )
                .unwrap(),
            LocalDecision::ReadyToCommit
        );
        ticket
    }

    fn generation_of_alpha(held: &HeldLedger) -> u64 {
        let ledger = held.lock().unwrap();
        ledger.machine.record(&alpha()).unwrap().generation
    }

    #[test]
    fn an_intent_nobody_answered_is_recorded_as_an_unknown_outcome() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let generation = generation_of_alpha(&held);
        let ticket = before_intent(&world, &held);
        let permit = record_intent(&held, &ticket, Some(sha_of(b"alpha v1")))
            .unwrap()
            .unwrap();
        // Durable before the permit exists, and only the permit can end it.
        assert!(world.saved().targets[0].has_recorded_intent());
        assert_eq!(generation_of_alpha(&held), generation);
        drop(permit);

        let ledger = held.lock().unwrap();
        let record = ledger.machine.record(&alpha()).unwrap();
        assert!(record.in_flight.is_none(), "the intent has been answered");
        assert_eq!(record.generation, generation + 1);
        assert_eq!(record.phase, Phase::Unverified);
        assert_eq!(
            record.unverified_reason,
            Some(UnverifiedReason::CommitOutcomeUnknown)
        );
        assert_eq!(world.saved().targets[0].generation, generation + 1);
        assert_eq!(ledger.failed, None);
    }

    #[test]
    fn an_answered_permit_records_once_and_its_drop_adds_nothing() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let generation = generation_of_alpha(&held);
        let ticket = before_intent(&world, &held);
        let permit = record_intent(&held, &ticket, Some(sha_of(b"alpha v1")))
            .unwrap()
            .unwrap();
        assert_eq!(
            permit.answer(CommitObservation::NotAdopted).unwrap(),
            Ok(CommitDecision::NotAdopted)
        );
        let ledger = held.lock().unwrap();
        let record = ledger.machine.record(&alpha()).unwrap();
        assert_eq!(
            record.generation, generation,
            "the old bytes are still there"
        );
        assert_eq!(record.unverified_reason, None);
        assert!(record.in_flight.is_none());
    }

    #[test]
    fn an_intent_refused_hands_out_no_permit() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let ticket = before_intent(&world, &held);
        let permit = record_intent(&held, &ticket, None).unwrap().unwrap();
        // The second intent on the same attempt is out of order, and the first is still open.
        assert_eq!(
            record_intent(&held, &ticket, None).unwrap().err(),
            Some(StageRefusal::OutOfOrder)
        );
        assert!(
            held.lock()
                .unwrap()
                .machine
                .record(&alpha())
                .unwrap()
                .has_recorded_intent()
        );
        drop(permit);
    }

    #[test]
    fn a_permit_dropped_when_the_write_fails_stops_the_ledger_and_does_not_panic() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let ticket = before_intent(&world, &held);
        let permit = record_intent(&held, &ticket, None).unwrap().unwrap();
        let on_disk = fs::read(world.file()).unwrap();
        fs::create_dir(world.directory().join(LEDGER_TEMPORARY)).unwrap();
        drop(permit);

        let mut ledger = held.lock().unwrap();
        assert_eq!(ledger.failed, Some(LedgerRefusal::Unavailable));
        assert!(
            fs::read(world.file()).unwrap() == on_disk,
            "the intent stays"
        );
        assert!(!ledger.verdict(&world.bytes).ledger_intact());
    }

    #[test]
    fn a_permit_dropped_on_a_poisoned_lock_stops_the_ledger_and_does_not_panic() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let ticket = before_intent(&world, &held);
        let permit = record_intent(&held, &ticket, None).unwrap().unwrap();
        let on_disk = fs::read(world.file()).unwrap();

        let poisoner = held.clone();
        let outcome = std::thread::spawn(move || {
            let _guard = poisoner.lock().unwrap();
            panic!("a panic standing for one in the middle of a ledger call");
        })
        .join();
        assert!(outcome.is_err());
        assert!(held.is_poisoned());

        drop(permit);
        let ledger = held.lock().unwrap_or_else(PoisonError::into_inner);
        assert_eq!(ledger.failed, Some(LedgerRefusal::Unavailable));
        assert!(fs::read(world.file()).unwrap() == on_disk, "no write");
    }

    #[test]
    fn what_is_at_a_path_is_read_by_its_bytes_and_a_link_is_not_followed() {
        let world = world();
        let bytes = WorkspaceBytes::new(&world.workspace);
        assert_eq!(bytes.observe("out/nothing.json"), ObservedBytes::Absent);
        assert_eq!(bytes.observe("nothing.json"), ObservedBytes::Absent);

        fs::create_dir(world.workspace.join("out")).unwrap();
        assert_eq!(bytes.observe("out/nothing.json"), ObservedBytes::Absent);
        fs::write(world.workspace.join("out/there.json"), b"held").unwrap();
        assert_eq!(
            bytes.observe("out/there.json"),
            ObservedBytes::Present(b"held".to_vec())
        );
        assert_eq!(bytes.observe("out"), ObservedBytes::Unreadable);
        std::os::unix::fs::symlink(
            world.workspace.join("out/there.json"),
            world.workspace.join("out/link.json"),
        )
        .unwrap();
        assert_eq!(bytes.observe("out/link.json"), ObservedBytes::Unreadable);
    }

    fn begin_alpha_at(ledger: &mut StageLedger, bytes: &FakeBytes, now: u64) -> BeginDecision {
        let contract = ledger.contract().clone();
        let revision = ledger.revision_of(&alpha()).unwrap();
        let output = bytes.observe("out/alpha.json");
        ledger
            .begin(
                &alpha(),
                request_for(&contract, &alpha(), &revision, bytes, 1, now, "a", &output),
            )
            .unwrap()
    }

    /// T7: the clock starts at the run's first stage call, and no resume starts it again.
    #[test]
    fn the_deadline_starts_once_and_a_resume_gives_none_of_it_back() {
        let world = world();
        const T0: u64 = 50_000;
        let deadline_ms = world.contract.limits().deadline_seconds * 1000;
        let at = |entry, now_ms| {
            let mut request = world.request(entry);
            request.now_ms = now_ms;
            LedgerRegistry::default().open(&request).unwrap()
        };

        let held = at(LedgerEntry::Fresh, T0);
        assert!(matches!(
            begin_alpha_at(&mut held.lock().unwrap(), &world.bytes, T0),
            BeginDecision::Proceed(_)
        ));
        assert_eq!(world.saved().started_at_ms, T0);
        drop(held);

        // A later process, ten seconds on.
        let held = at(LedgerEntry::Resume, T0 + 10_000);
        assert_eq!(held.lock().unwrap().machine.snapshot().started_at_ms, T0);
        assert!(matches!(
            begin_alpha_at(
                &mut held.lock().unwrap(),
                &world.bytes,
                T0 + deadline_ms - 1
            ),
            BeginDecision::Proceed(_)
        ));
        assert_eq!(
            begin_alpha_at(&mut held.lock().unwrap(), &world.bytes, T0 + deadline_ms),
            BeginDecision::DeadlineExceeded
        );
        drop(held);

        // A fresh open that a resume takes up in the same process: the saved start, not its own.
        let registry = LedgerRegistry::default();
        let mut fresh = world.request(LedgerEntry::Fresh);
        fresh.now_ms = T0 + 90_000;
        let opened = registry.open(&fresh).unwrap();
        assert_eq!(
            opened.lock().unwrap().machine.snapshot().started_at_ms,
            T0 + 90_000
        );
        let mut resume = world.request(LedgerEntry::Resume);
        resume.now_ms = T0 + 90_000;
        let resumed = registry.open(&resume).unwrap();
        assert_eq!(resumed.lock().unwrap().machine.snapshot().started_at_ms, T0);
    }

    /// N1: opening is not a stage call. A judgment, or a Read in front of the first call, opens the
    /// ledger, and the time until that first call is not taken out of the run's deadline.
    #[test]
    fn the_deadline_starts_at_the_first_stage_call_and_not_at_the_open() {
        let world = world();
        const OPENED: u64 = 50_000;
        const FIRST_CALL: u64 = OPENED + 7_000;
        let deadline_ms = world.contract.limits().deadline_seconds * 1000;
        let mut request = world.request(LedgerEntry::Fresh);
        request.now_ms = OPENED;
        let held = LedgerRegistry::default().open(&request).unwrap();
        assert!(matches!(
            begin_alpha_at(&mut held.lock().unwrap(), &world.bytes, FIRST_CALL),
            BeginDecision::Proceed(_)
        ));
        assert_eq!(world.saved().started_at_ms, FIRST_CALL);

        // Later calls move it no further: the window is as long as declared, from that call.
        assert_eq!(
            begin_alpha_at(
                &mut held.lock().unwrap(),
                &world.bytes,
                FIRST_CALL + deadline_ms
            ),
            BeginDecision::DeadlineExceeded
        );
        assert_eq!(world.saved().started_at_ms, FIRST_CALL);
    }

    #[test]
    fn the_deadline_cannot_be_declared_past_the_hosts_limit() {
        use super::super::contract::{ContractRefusalKind, HOST_MAX_DEADLINE_SECONDS};
        let at_limit = StageSetDeclaration {
            stages: vec![one_target("only", "t")],
            limits: NarrowedStageLimits {
                deadline_seconds: Some(HOST_MAX_DEADLINE_SECONDS),
                ..NarrowedStageLimits::default()
            },
        };
        let tool = facts("t", "out/t", &[]);
        assert!(admit(&at_limit, std::slice::from_ref(&tool)).is_ok());
        let over = StageSetDeclaration {
            limits: NarrowedStageLimits {
                deadline_seconds: Some(HOST_MAX_DEADLINE_SECONDS + 1),
                ..NarrowedStageLimits::default()
            },
            ..at_limit
        };
        assert_eq!(
            admit(&over, std::slice::from_ref(&tool)).unwrap_err().kind,
            ContractRefusalKind::WidenedLimit
        );
        let mut floor_over = tool;
        floor_over.limits.deadline_seconds = HOST_MAX_DEADLINE_SECONDS + 1;
        let plain = StageSetDeclaration {
            limits: NarrowedStageLimits::default(),
            ..over
        };
        assert_eq!(
            admit(&plain, &[floor_over]).unwrap_err().kind,
            ContractRefusalKind::InvalidLimit
        );
    }

    #[test]
    fn fresh_wins_over_resume_and_neither_means_fresh() {
        let mut config =
            crate::tools::profile_tools::effects::config_for_test(Path::new("/nonexistent"));
        assert_eq!(LedgerEntry::of_config(&config), LedgerEntry::Fresh);
        config.resume = Some("a-saved-session".to_string());
        assert_eq!(LedgerEntry::of_config(&config), LedgerEntry::Resume);
        config.fresh_session = true;
        assert_eq!(LedgerEntry::of_config(&config), LedgerEntry::FreshSession);
        config.resume = None;
        assert_eq!(LedgerEntry::of_config(&config), LedgerEntry::FreshSession);
    }

    #[test]
    fn a_ledger_directory_linked_into_the_workspace_is_refused_and_nothing_is_written_there() {
        let world = world();
        let linked = world.workspace.join("linked");
        fs::create_dir(&linked).unwrap();
        fs::create_dir_all(&world.state_dir).unwrap();
        std::os::unix::fs::symlink(&linked, world.state_dir.join(LEDGER_LEAF)).unwrap();
        assert_eq!(
            world.process(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::StateDirInsideWorkspace
        );
        assert_eq!(fs::read_dir(&linked).unwrap().count(), 0);
    }

    #[test]
    fn a_state_dir_commands_may_write_as_a_dependency_cache_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(root.path()).unwrap();
        let (workspace, home) = (root.join("workspace"), root.join("home"));
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(home.join(".npm/below")).unwrap();
        let policy = SandboxPolicy::with_home(&workspace, &root.join("tmp"), home.clone());
        assert!(policy.caches.npm.is_some(), "{:?}", policy.caches);
        for inside in [home.join(".npm"), home.join(".npm/below/state")] {
            assert_eq!(
                refuse_state_dir_under_policy(&inside, &workspace, &policy),
                Err(LedgerRefusal::StateDirWritableByCommands),
                "{}",
                inside.display()
            );
        }
        for outside in [home.join("state"), home.join(".npm-other")] {
            assert_eq!(
                refuse_state_dir_under_policy(&outside, &workspace, &policy),
                Ok(()),
                "{}",
                outside.display()
            );
        }
    }

    #[test]
    fn a_pin_outside_the_contract_in_a_saved_record_is_damage() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        world.bytes.put("elsewhere/outside.json", b"outside bytes");
        let mut saved = world.saved();
        saved.targets[0].fixed_inputs = Some(vec![InputDigest {
            path: "elsewhere/outside.json".to_string(),
            sha256: sha_of(b"outside bytes"),
        }]);
        world.save(&saved);
        assert_eq!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_pin_outside_the_contract_in_a_file_changed_under_a_live_ledger_is_damage() {
        let world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        let mut saved = world.saved();
        saved.targets[0].fixed_inputs = Some(vec![InputDigest {
            path: "elsewhere/outside.json".to_string(),
            sha256: sha_of(b"outside bytes"),
        }]);
        world.save(&saved);
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_fifo_in_place_of_the_lock_is_no_lock() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("ledger");
        fs::create_dir(&directory).unwrap();
        make_fifo(&directory.join(LOCK_FILE));
        let outcome = within_a_bound(move || Store::open(directory).map(|_| ()));
        assert_eq!(outcome, Err(LedgerRefusal::Unavailable));
    }

    #[test]
    fn a_fresh_start_with_nothing_before_it_writes_nothing_until_the_first_begin() {
        let world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        assert!(!world.file().exists());
        assert!(held.lock().unwrap().verdict(&world.bytes).ledger_intact());
        assert!(registry.open(&world.request(LedgerEntry::Fresh)).is_ok());
        assert!(!world.file().exists());

        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        assert!(world.file().exists());
    }

    #[test]
    fn a_first_begin_that_starts_nothing_still_replaces_the_previous_run() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let old_run = held.lock().unwrap().machine.snapshot().run_nonce.clone();
        drop(held);

        let held = world.process(LedgerEntry::Fresh).unwrap();
        let mut ledger = held.lock().unwrap();
        let contract = ledger.contract().clone();
        let output = world.bytes.observe("out/beta.json");
        let decision = ledger
            .begin(
                &beta(),
                request_for(
                    &contract,
                    &beta(),
                    "initial",
                    &world.bytes,
                    1,
                    NOW,
                    "b",
                    &output,
                ),
            )
            .unwrap();
        assert!(matches!(decision, BeginDecision::NotReady { .. }));
        let saved = world.saved();
        assert_ne!(saved.run_nonce, old_run);
        assert_eq!(saved.attempts_used, 0);
    }

    /// `beta` reads `alpha`'s output, and neither has a fixed input.
    fn world_with_no_fixed_input() -> World {
        let mut world = world();
        let declaration = StageSetDeclaration {
            stages: vec![one_target("alpha", "main"), one_target("beta", "beta-tool")],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [
            facts("main", "out/alpha.json", &[]),
            facts("beta-tool", "out/beta.json", &["out/alpha.json"]),
        ];
        world.contract = Arc::new(admit(&declaration, &tools).unwrap());
        world.pins = pins_of(&world.contract);
        world
    }

    fn previous_run_of(world: &World) -> String {
        let held = world.process(LedgerEntry::Fresh).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        let run = world.saved().run_nonce;
        assert_eq!(world.saved().attempts_used, 1);
        run
    }

    #[test]
    fn with_no_fixed_input_a_first_begin_that_starts_nothing_still_replaces_the_previous_run() {
        let world = world_with_no_fixed_input();
        let old_run = previous_run_of(&world);

        let held = world.process(LedgerEntry::Fresh).unwrap();
        let target = key("beta", "beta-tool");
        let mut ledger = held.lock().unwrap();
        let contract = ledger.contract().clone();
        let output = world.bytes.observe("out/beta.json");
        let decision = ledger
            .begin(
                &target,
                request_for(
                    &contract,
                    &target,
                    "initial",
                    &world.bytes,
                    1,
                    NOW,
                    "b",
                    &output,
                ),
            )
            .unwrap();
        assert!(
            matches!(decision, BeginDecision::NotReady { .. }),
            "{decision:?}"
        );
        let saved = world.saved();
        assert_ne!(saved.run_nonce, old_run);
        assert_eq!(saved.attempts_used, 0);
    }

    /// The one first `begin` that changes nothing in the machine, so only `write_regardless`
    /// makes it replace the file. Every other answer has by then pinned the fixed inputs, which
    /// changes the record even when there are none to pin. A production call always names a
    /// target of the contract, so this is the guarantee stated, not a path a run takes.
    #[test]
    fn a_first_begin_that_changes_nothing_in_the_machine_still_replaces_the_previous_run() {
        let world = world_with_no_fixed_input();
        let old_run = previous_run_of(&world);

        let held = world.process(LedgerEntry::Fresh).unwrap();
        let unknown = key("no-such-stage", "main");
        let mut ledger = held.lock().unwrap();
        let contract = ledger.contract().clone();
        let before = ledger.machine.snapshot().clone();
        let output = world.bytes.observe("out/alpha.json");
        let decision = ledger
            .begin(
                &unknown,
                request_for(
                    &contract,
                    &unknown,
                    "initial",
                    &world.bytes,
                    1,
                    NOW,
                    "a",
                    &output,
                ),
            )
            .unwrap();
        assert_eq!(decision, BeginDecision::UnknownTarget);
        assert_eq!(ledger.machine.snapshot(), &before);
        let saved = world.saved();
        assert_ne!(saved.run_nonce, old_run);
        assert_eq!(saved.run_nonce, before.run_nonce);
    }

    #[test]
    fn a_first_begin_that_cannot_write_leaves_the_previous_run_and_starts_nothing() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let previous = fs::read(world.file()).unwrap();

        let held = world.process(LedgerEntry::Fresh).unwrap();
        fs::create_dir(world.directory().join(LEDGER_TEMPORARY)).unwrap();
        let mut ledger = held.lock().unwrap();
        let contract = ledger.contract().clone();
        let output = world.bytes.observe("out/alpha.json");
        let attempted = ledger.begin(
            &alpha(),
            request_for(
                &contract,
                &alpha(),
                "initial",
                &world.bytes,
                1,
                NOW,
                "a",
                &output,
            ),
        );
        assert_eq!(attempted.unwrap_err(), LedgerRefusal::Unavailable);
        assert!(fs::read(world.file()).unwrap() == previous);
        assert!(!ledger.verdict(&world.bytes).ledger_intact());
    }

    #[test]
    fn a_file_that_changes_under_a_fresh_start_not_yet_begun_stops_it() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        let mut saved = world.saved();
        saved.attempts_used += 1;
        world.save(&saved);
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Diverged
        );
        assert!(!held.lock().unwrap().verdict(&world.bytes).complete());
    }

    #[test]
    fn a_resume_after_a_fixed_input_changed_is_refused_by_name_and_only_a_fresh_start_goes_on() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let previous = fs::read(world.file()).unwrap();

        world.bytes.put("fixed/source.json", b"quietly rewritten");
        assert_eq!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::FixedInputChanged {
                path: "fixed/source.json".to_string()
            }
        );
        assert!(fs::read(world.file()).unwrap() == previous, "nothing moved");

        // The refusal holds for as long as the input differs, and leaves no lock behind.
        assert!(matches!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::FixedInputChanged { .. }
        ));
        let held = world.process(LedgerEntry::Fresh).unwrap();
        let mut ledger = held.lock().unwrap();
        start(&mut ledger, &alpha(), &world.bytes, 1, "a");
        assert_eq!(world.saved().attempts_used, 1);
    }

    /// Stands for the call hanging: a blocking open would never send.
    fn within_a_bound<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> T {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(call());
        });
        receiver
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("the call did not return")
    }

    fn make_fifo(path: &Path) {
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: `path` is a valid C string that outlives the call, which keeps no reference.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    }

    #[test]
    fn a_fifo_in_place_of_the_ledger_is_damage_and_does_not_hang_the_open() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let file = world.file();
        fs::remove_file(&file).unwrap();
        make_fifo(&file);
        for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
            let world = World {
                _root: tempfile::tempdir().unwrap(),
                workspace: world.workspace.clone(),
                state_dir: world.state_dir.clone(),
                contract: world.contract.clone(),
                pins: world.pins.clone(),
                bytes: world.bytes.clone(),
            };
            let outcome = within_a_bound(move || world.process(entry).map(|_| ()));
            assert_eq!(outcome, Err(LedgerRefusal::Corrupt), "{entry:?}");
        }
    }

    #[test]
    fn a_fifo_in_place_of_the_temporary_is_a_failed_write_and_does_not_hang_it() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        make_fifo(&world.directory().join(LEDGER_TEMPORARY));
        let bytes = world.bytes.clone();
        let outcome = within_a_bound(move || {
            let mut ledger = held.lock().unwrap();
            let contract = ledger.contract().clone();
            let output = bytes.observe("out/alpha.json");
            ledger
                .begin(
                    &alpha(),
                    request_for(&contract, &alpha(), "initial", &bytes, 1, NOW, "a", &output),
                )
                .map(|_| ())
        });
        assert_eq!(outcome, Err(LedgerRefusal::Unavailable));
    }

    #[test]
    fn a_record_too_large_to_read_back_is_never_written() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(root.path().join("ledger")).unwrap();
        let contract = Arc::new(two_stage_contract());
        let sized = |bytes: usize| {
            let base = StageMachine::new(contract.clone(), String::new(), "p".to_string(), NOW);
            let mut snapshot = base.snapshot().clone();
            let empty = encode(&snapshot).unwrap().len();
            snapshot.run_nonce = "n".repeat(bytes - empty);
            assert_eq!(encode(&snapshot).unwrap().len(), bytes);
            snapshot
        };
        let limit = super::MAX_LEDGER_BYTES as usize;

        store.write(&sized(limit - 1)).unwrap();
        store.write(&sized(limit)).unwrap();
        let at_the_limit = store.read_file().unwrap().unwrap();
        assert_eq!(decode(&at_the_limit).unwrap(), sized(limit));

        assert_eq!(
            store.write(&sized(limit + 1)).unwrap_err(),
            LedgerRefusal::Unavailable
        );
        assert!(
            store.read_file().unwrap().unwrap() == at_the_limit,
            "the record before it is what stays"
        );
        assert!(store.verify_unchanged().is_ok());
    }

    #[test]
    fn a_resume_keeps_the_counts_and_the_run_and_seals_nothing() {
        let mut world = world();
        let held = both_sealed(&mut world);
        let before = held.lock().unwrap().machine.snapshot().clone();
        drop(held);

        let held = world.process(LedgerEntry::Resume).unwrap();
        let mut ledger = held.lock().unwrap();
        let after = ledger.machine.snapshot().clone();
        assert_eq!(after.run_nonce, before.run_nonce);
        assert_ne!(after.process_nonce, before.process_nonce);
        assert_eq!(after.attempts_used, before.attempts_used);
        assert_eq!(after.started_at_ms, before.started_at_ms);
        let verdict = ledger.verdict(&world.bytes);
        assert!(!verdict.complete());
        assert!(
            verdict
                .obligations()
                .iter()
                .all(|status| status.phase == Phase::Unverified)
        );
        // The lowering itself is durable.
        assert_eq!(world.saved().targets[0].phase, Phase::Unverified);
    }

    #[test]
    fn a_resume_after_a_crash_at_each_commit_point_follows_the_table() {
        // (bytes on disk, reason, generation moved, downstream taken)
        let cases: [(Option<&[u8]>, UnverifiedReason, u64, bool); 3] = [
            (
                Some(b"alpha v2"),
                UnverifiedReason::InterruptedCommit,
                1,
                true,
            ),
            (Some(b"alpha v1"), UnverifiedReason::Resumed, 0, false),
            (Some(b"neither"), UnverifiedReason::BytesChanged, 1, true),
        ];
        for (on_disk, reason, moved, downstream) in cases {
            let mut world = world();
            let held = both_sealed(&mut world);
            let generation = {
                let mut ledger = held.lock().unwrap();
                let bytes = world.bytes.clone();
                to_intent(&mut ledger, &alpha(), b"alpha v2", &bytes, 3, "a2");
                ledger.machine.record(&alpha()).unwrap().generation
            };
            // The intent got out; the process did not.
            assert!(world.saved().targets[0].in_flight.is_some());
            drop(held);

            world.bytes.put("out/alpha.json", on_disk.unwrap());
            let held = world.process(LedgerEntry::Resume).unwrap();
            let mut ledger = held.lock().unwrap();
            let record = ledger.machine.record(&alpha()).unwrap().clone();
            assert_eq!(record.unverified_reason, Some(reason));
            assert_eq!(record.generation, generation + moved);
            assert_eq!(record.attempts_used, 2, "the attempt stays spent");
            assert!(record.in_flight.is_none());
            let phase = ledger.machine.record(&beta()).unwrap().phase;
            assert_eq!(phase == Phase::NotReady, downstream);
            assert!(!ledger.verdict(&world.bytes).complete(), "never sealed");
        }
    }

    #[test]
    fn damage_of_every_kind_is_refused_on_a_resume_and_on_a_fresh_start_and_left_in_place() {
        let mut base = world();
        let held = both_sealed(&mut base);
        drop(held);
        let good = fs::read(base.file()).unwrap();
        let mut flipped = good.clone();
        let position = flipped.len() / 2;
        flipped[position] ^= 0x01;

        // A correct digest over a body that says something the schema does not.
        let with_claim = {
            let mut body: serde_json::Value = serde_json::to_value(base.saved()).unwrap();
            body["accepted"] = serde_json::Value::Bool(true);
            let text = serde_json::to_string(&body).unwrap();
            format!(
                r#"{{"sha256":"{}","body":{text}}}"#,
                sha256_hex(text.as_bytes())
            )
            .into_bytes()
        };
        // A body that fits, under a stored digest that is not its own.
        let wrong_digest = {
            let text = serde_json::to_string(&base.saved()).unwrap();
            format!(r#"{{"sha256":"{}","body":{text}}}"#, sha_of(b"other")).into_bytes()
        };
        let cases: [(&str, Vec<u8>); 7] = [
            ("empty", Vec::new()),
            ("truncated", good[..good.len() / 2].to_vec()),
            ("flipped", flipped),
            ("not json", b"not json".to_vec()),
            ("claim", with_claim),
            ("digest", wrong_digest),
            (
                "too large",
                vec![b' '; (super::MAX_LEDGER_BYTES + 1) as usize],
            ),
        ];
        for (name, content) in cases {
            fs::write(base.file(), &content).unwrap();
            for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
                assert_eq!(
                    base.process(entry).unwrap_err(),
                    LedgerRefusal::Corrupt,
                    "{name} {entry:?}"
                );
                assert_eq!(
                    fs::read(base.file()).unwrap(),
                    content,
                    "{name}: left as it was"
                );
            }
        }
    }

    #[test]
    fn a_digest_that_holds_on_counts_that_do_not_is_refused() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let mut saved = world.saved();
        saved.attempts_used = 0;
        world.save(&saved);
        assert_eq!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_symlink_in_place_of_the_ledger_is_damage() {
        let mut world = world();
        let held = both_sealed(&mut world);
        drop(held);
        let real = world.workspace.join("elsewhere.json");
        fs::rename(world.file(), &real).unwrap();
        std::os::unix::fs::symlink(&real, world.file()).unwrap();
        assert_eq!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_changed_contract_is_refused_on_a_resume_and_in_a_live_process() {
        let mut world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        let other = Arc::new(
            admit(
                &StageSetDeclaration {
                    stages: vec![one_target("other", "t")],
                    limits: NarrowedStageLimits::default(),
                },
                &[facts("t", "out/other", &[])],
            )
            .unwrap(),
        );
        let original = std::mem::replace(&mut world.contract, other);
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::ContractChanged
        );
        // That refusal did not cost the ledger anything.
        world.contract = original;
        assert!(registry.open(&world.request(LedgerEntry::Resume)).is_ok());
        drop(held);
        drop(registry);

        world.contract = Arc::new(
            admit(
                &StageSetDeclaration {
                    stages: vec![one_target("other", "t")],
                    limits: NarrowedStageLimits::default(),
                },
                &[facts("t", "out/other", &[])],
            )
            .unwrap(),
        );
        assert_eq!(
            world.process(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::ContractChanged
        );
    }

    #[test]
    fn a_ledger_that_goes_missing_in_a_run_refuses_and_the_counts_do_not_start_over() {
        let mut world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        {
            let mut ledger = held.lock().unwrap();
            let mut bytes = world.bytes.clone();
            seal(&mut ledger, &alpha(), b"alpha v1", &mut bytes, 1, "a");
            seal(&mut ledger, &beta(), b"beta v1", &mut bytes, 2, "b");
            world.bytes = bytes;
        }
        fs::remove_file(world.file()).unwrap();

        {
            let mut ledger = held.lock().unwrap();
            let verdict = ledger.verdict(&world.bytes);
            assert!(!verdict.complete(), "every target is sealed, and still no");
            assert!(!verdict.ledger_intact());
            let contract = ledger.contract().clone();
            let revision = ledger.revision_of(&alpha()).unwrap();
            let output = world.bytes.observe("out/alpha.json");
            assert_eq!(
                ledger
                    .begin(
                        &alpha(),
                        request_for(
                            &contract,
                            &alpha(),
                            &revision,
                            &world.bytes,
                            3,
                            NOW,
                            "x",
                            &output
                        ),
                    )
                    .unwrap_err(),
                LedgerRefusal::Missing
            );
        }
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Fresh))
                .unwrap_err(),
            LedgerRefusal::Missing,
            "a later session in this process does not start over either"
        );
        assert!(!world.file().exists(), "nothing wrote a new one at zero");
    }

    #[test]
    fn a_file_damaged_under_a_live_process_refuses_from_then_on() {
        let mut world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        {
            let mut ledger = held.lock().unwrap();
            let mut bytes = world.bytes.clone();
            seal(&mut ledger, &alpha(), b"alpha v1", &mut bytes, 1, "a");
            world.bytes = bytes;
        }
        fs::write(world.file(), b"garbage").unwrap();
        assert_eq!(
            registry
                .open(&world.request(LedgerEntry::Resume))
                .unwrap_err(),
            LedgerRefusal::Corrupt
        );
        // Putting something readable back does not lift it.
        let mut ledger = held.lock().unwrap();
        let verdict = ledger.verdict(&world.bytes);
        assert!(!verdict.complete());
        assert!(!verdict.ledger_intact());
    }

    #[test]
    fn a_readable_file_changed_under_a_live_process_stops_the_next_step() {
        let mut world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        {
            let mut ledger = held.lock().unwrap();
            let mut bytes = world.bytes.clone();
            seal(&mut ledger, &alpha(), b"alpha v1", &mut bytes, 1, "a");
            world.bytes = bytes;
        }
        let mut saved = world.saved();
        saved.attempts_used = 0;
        saved.targets[0].attempts_used = 0;
        world.save(&saved);
        let mut ledger = held.lock().unwrap();
        let verdict = ledger.verdict(&world.bytes);
        assert!(!verdict.complete());
        assert_eq!(ledger.failed, Some(LedgerRefusal::Diverged));
    }

    #[test]
    fn a_later_session_takes_the_higher_counts_and_believes_no_state() {
        for (file_attempts, file_target, kept_attempts, kept_target) in
            [(0, 0, 1, 1), (40, 7, 40, 7)]
        {
            let mut world = world();
            let registry = LedgerRegistry::default();
            let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
            {
                let mut ledger = held.lock().unwrap();
                let mut bytes = world.bytes.clone();
                seal(&mut ledger, &alpha(), b"alpha v1", &mut bytes, 1, "a");
                world.bytes = bytes;
            }
            let mut saved = world.saved();
            saved.attempts_used = file_attempts;
            saved.targets[0].attempts_used = file_target;
            if file_attempts == 0 {
                // Keep it a record that fits, so what is tested is the higher count.
                saved.targets[1].attempts_used = 0;
            }
            world.save(&saved);

            let again = registry.open(&world.request(LedgerEntry::Resume)).unwrap();
            assert!(Arc::ptr_eq(&held, &again));
            let mut ledger = again.lock().unwrap();
            let snapshot = ledger.machine.snapshot().clone();
            assert_eq!(snapshot.attempts_used, kept_attempts);
            assert_eq!(snapshot.targets[0].attempts_used, kept_target);
            assert_eq!(snapshot.targets[0].phase, Phase::Unverified);
            assert_eq!(
                snapshot.targets[0].unverified_reason,
                Some(UnverifiedReason::LedgerDiverged)
            );
            let verdict = ledger.verdict(&world.bytes);
            assert!(verdict.ledger_intact());
            assert!(!verdict.complete());
        }
    }

    #[test]
    fn a_ledger_that_cannot_be_written_stops_the_attempt_before_it_starts() {
        let mut world = world();
        let registry = LedgerRegistry::default();
        let held = registry.open(&world.request(LedgerEntry::Fresh)).unwrap();
        {
            let mut ledger = held.lock().unwrap();
            let mut bytes = world.bytes.clone();
            seal(&mut ledger, &alpha(), b"alpha v1", &mut bytes, 1, "a");
            seal(&mut ledger, &beta(), b"beta v1", &mut bytes, 2, "b");
            world.bytes = bytes;
        }
        // A directory where the temporary goes fails for every user, root included.
        fs::create_dir(world.directory().join(LEDGER_TEMPORARY)).unwrap();

        let mut ledger = held.lock().unwrap();
        let before = ledger.machine.snapshot().attempts_used;
        let contract = ledger.contract().clone();
        let revision = ledger.revision_of(&alpha()).unwrap();
        let output = world.bytes.observe("out/alpha.json");
        let attempted = ledger.begin(
            &alpha(),
            request_for(
                &contract,
                &alpha(),
                &revision,
                &world.bytes,
                3,
                NOW,
                "new",
                &output,
            ),
        );
        assert_eq!(attempted.unwrap_err(), LedgerRefusal::Unavailable);
        assert_eq!(ledger.machine.snapshot().attempts_used, before);
        let verdict = ledger.verdict(&world.bytes);
        assert!(
            !verdict.complete(),
            "no success once the record cannot be kept"
        );
        assert!(!verdict.ledger_intact());
    }

    #[test]
    fn a_failed_intent_write_means_no_rename_may_follow() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        let mut ledger = held.lock().unwrap();
        let contract = ledger.contract().clone();
        let ticket = start(&mut ledger, &alpha(), &world.bytes, 1, "a");
        let absent = world.bytes.observe("out/alpha.json");
        ledger
            .record_local_result(
                &ticket,
                &sha_of(b"x"),
                passed_checks(&contract, &alpha()),
                &absent,
            )
            .unwrap();
        fs::create_dir(world.directory().join(LEDGER_TEMPORARY)).unwrap();
        assert_eq!(
            ledger.record_intent(&ticket, None).unwrap_err(),
            LedgerRefusal::Unavailable
        );
        assert_eq!(
            ledger.cas_check(&ticket).unwrap_err(),
            LedgerRefusal::Unavailable,
            "the compare right before a rename stops too"
        );
    }

    #[test]
    fn the_files_are_for_the_owner_only() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        for path in [world.file(), world.directory().join(LOCK_FILE)] {
            let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let mode = fs::metadata(world.directory())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
        drop(held);
    }

    #[test]
    fn a_state_dir_in_the_workspace_is_refused_by_any_spelling() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let outside = root.path().join("state");
        let alias = root.path().join("alias");
        std::os::unix::fs::symlink(&workspace, &alias).unwrap();

        let inside: [PathBuf; 4] = [
            workspace.clone(),
            workspace.join("deep/not/yet/made"),
            workspace.join("./x"),
            alias.join("through-a-link"),
        ];
        for state_dir in &inside {
            assert_eq!(
                refuse_state_dir_inside_workspace(state_dir, &workspace),
                Err(LedgerRefusal::StateDirInsideWorkspace),
                "{}",
                state_dir.display()
            );
        }
        assert_eq!(
            refuse_state_dir_inside_workspace(&outside, &workspace),
            Ok(())
        );
        // A name that only starts the same way is outside.
        let beside = root.path().join("workspace-state");
        assert_eq!(
            refuse_state_dir_inside_workspace(&beside, &workspace),
            Ok(())
        );
        // A tail that could lead anywhere is no answer.
        let tail = root.path().join("not-made/../workspace/x");
        assert_eq!(
            refuse_state_dir_inside_workspace(&tail, &workspace),
            Err(LedgerRefusal::Unavailable)
        );
    }

    #[test]
    fn a_dotted_path_that_exists_is_judged_by_where_it_leads() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let outside = root.path().join("state");
        fs::create_dir(&workspace).unwrap();
        fs::create_dir(&outside).unwrap();

        let into_the_workspace = outside.join("..").join("workspace").join("x");
        assert_eq!(
            refuse_state_dir_inside_workspace(&into_the_workspace, &workspace),
            Err(LedgerRefusal::StateDirInsideWorkspace)
        );
        let out_of_the_workspace = workspace.join("..").join("state").join("x");
        assert_eq!(
            refuse_state_dir_inside_workspace(&out_of_the_workspace, &workspace),
            Ok(())
        );
    }

    #[test]
    fn a_link_that_points_at_nothing_is_no_state_dir() {
        let world = world();
        let link = world.workspace.parent().unwrap().join("link");
        let target = world.workspace.join(".later");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert_eq!(
            refuse_state_dir_inside_workspace(&link, &world.workspace),
            Err(LedgerRefusal::Unavailable)
        );
        // Further down the link, the same.
        assert_eq!(
            refuse_state_dir_inside_workspace(&link.join("below"), &world.workspace),
            Err(LedgerRefusal::Unavailable)
        );
        let mut request = world.request(LedgerEntry::Fresh);
        request.state_dir = &link;
        assert_eq!(
            LedgerRegistry::default().open(&request).unwrap_err(),
            LedgerRefusal::Unavailable
        );
        assert!(!target.exists(), "nothing was made behind the link");
    }

    /// Only where the volume folds case: on another, the two spellings are two directories.
    #[cfg(target_os = "macos")]
    #[test]
    fn another_case_of_the_workspace_name_is_still_the_workspace() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let other_case = root.path().join("WORKSPACE");
        if !other_case.exists() {
            return;
        }
        assert_eq!(
            refuse_state_dir_inside_workspace(&other_case.join("x"), &workspace),
            Err(LedgerRefusal::StateDirInsideWorkspace)
        );
    }

    #[test]
    fn opening_with_a_state_dir_in_the_workspace_creates_nothing() {
        let world = world();
        let inside = world.workspace.join(".state");
        let mut request = world.request(LedgerEntry::Fresh);
        request.state_dir = &inside;
        assert_eq!(
            LedgerRegistry::default().open(&request).unwrap_err(),
            LedgerRefusal::StateDirInsideWorkspace
        );
        assert!(!inside.exists());
    }

    #[test]
    fn the_identity_for_a_budget_is_readable_and_writes_nothing() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        let mut ledger = held.lock().unwrap();
        assert!(ledger.attempt_identity(&alpha()).is_none());
        start(&mut ledger, &alpha(), &world.bytes, 1, "a");
        let before = fs::read(world.file()).unwrap();
        let identity = ledger.attempt_identity(&alpha()).unwrap();
        assert_eq!(identity.run_nonce, ledger.machine.snapshot().run_nonce);
        assert_eq!(
            (identity.stage.as_str(), identity.target.as_str()),
            ("alpha", "main")
        );
        assert_eq!((identity.attempt, identity.generation), (1, 0));
        assert_eq!(fs::read(world.file()).unwrap(), before);
    }

    #[test]
    fn the_process_wide_registry_is_one_value() {
        assert!(Arc::ptr_eq(&process_ledgers(), &process_ledgers()));
    }

    /// Not a path a test should reach by accident: the helper above asserts one directory.
    #[test]
    fn a_ledger_lives_below_its_own_leaf_of_the_state_dir() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        start(&mut held.lock().unwrap(), &alpha(), &world.bytes, 1, "a");
        let directory = world.directory();
        assert!(directory.starts_with(world.state_dir.join(LEDGER_LEAF)));
        assert!(!directory.starts_with(&world.workspace));
        assert!(Path::new(&world.file()).exists());
        drop(held);
    }
}
