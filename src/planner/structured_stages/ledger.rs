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
use super::state::{
    AttemptFailure, AttemptIdentity, AttemptRequest, AttemptTicket, BeginDecision, BytesSource,
    CommitDecision, CommitObservation, FailureDecision, LocalDecision, MachineSnapshot,
    ObservedBytes, StageMachine, StageRefusal, StageVerdict,
};

const LEDGER_LEAF: &str = "structured-stages";
const LEDGER_FILE: &str = "ledger.json";
const LEDGER_TEMPORARY: &str = "ledger.json.tmp";
const LOCK_FILE: &str = "lock";
const MAX_LEDGER_BYTES: u64 = 8 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerEntry {
    /// A new process that is not a resume. Only here do the counts start over.
    Fresh,
    /// `--resume` or `runs resume`. The ledger must be there.
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerRefusal {
    Missing,
    Corrupt,
    Held,
    ContractChanged,
    StateDirInsideWorkspace,
    /// The file is not what this process last wrote.
    Diverged,
    Unavailable,
}

fn unavailable(_: std::io::Error) -> LedgerRefusal {
    LedgerRefusal::Unavailable
}

impl From<SavedRefusal> for LedgerRefusal {
    fn from(refusal: SavedRefusal) -> Self {
        match refusal {
            SavedRefusal::ContractChanged => Self::ContractChanged,
            SavedRefusal::WrongSchema
            | SavedRefusal::TargetsDiffer
            | SavedRefusal::CountsInconsistent => Self::Corrupt,
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
    let workspace = fs::canonicalize(workspace).map_err(unavailable)?;
    if resolved_with_missing_tail(state_dir)?.starts_with(&workspace) {
        return Err(LedgerRefusal::StateDirInsideWorkspace);
    }
    Ok(())
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
    last_written_sha256: Option<String>,
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
            .custom_flags(libc::O_NOFOLLOW)
            .open(directory.join(LOCK_FILE))
            .map_err(unavailable)?;
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
            last_written_sha256: None,
        })
    }

    /// `None` only when nothing is there. A symlink, or something too large to be a ledger, is
    /// damage.
    fn read_file(&self) -> Result<Option<Vec<u8>>, LedgerRefusal> {
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
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
    /// temporary name is enough and a crash cannot leave others behind.
    fn write(&mut self, snapshot: &MachineSnapshot) -> Result<(), LedgerRefusal> {
        let bytes = encode(snapshot)?;
        let temporary = self.directory.join(LEDGER_TEMPORARY);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)
            .map_err(unavailable)?;
        file.write_all(&bytes).map_err(unavailable)?;
        file.sync_all().map_err(unavailable)?;
        drop(file);
        fs::rename(&temporary, self.directory.join(LEDGER_FILE)).map_err(unavailable)?;
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(unavailable)?;
        self.last_written_sha256 = Some(sha256_hex(&bytes));
        Ok(())
    }

    fn holds_what_was_written(&self, bytes: &[u8]) -> bool {
        self.last_written_sha256.as_deref() == Some(sha256_hex(bytes).as_str())
    }

    /// Before anything is built on the state: the file is what this process left there.
    fn verify_unchanged(&self) -> Result<(), LedgerRefusal> {
        let bytes = self.read_file()?.ok_or(LedgerRefusal::Missing)?;
        if self.holds_what_was_written(&bytes) {
            Ok(())
        } else {
            Err(LedgerRefusal::Diverged)
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
}

impl StageLedger {
    fn apply<R>(
        &mut self,
        change: impl FnOnce(&mut StageMachine) -> R,
    ) -> Result<R, LedgerRefusal> {
        if let Some(refusal) = self.failed {
            return Err(refusal);
        }
        if let Err(refusal) = self.store.verify_unchanged() {
            self.failed = Some(refusal);
            return Err(refusal);
        }
        let mut next = self.machine.clone();
        let result = change(&mut next);
        if next.snapshot() != self.machine.snapshot() {
            if let Err(refusal) = self.store.write(next.snapshot()) {
                self.failed = Some(refusal);
                return Err(refusal);
            }
            self.machine = next;
        }
        Ok(result)
    }

    /// The D7 table and the limits. On `Proceed`, the attempt is on disk before this returns.
    pub(crate) fn begin(
        &mut self,
        key: &TargetKey,
        request: AttemptRequest<'_>,
    ) -> Result<BeginDecision, LedgerRefusal> {
        self.apply(|machine| machine.begin(key, request))
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

    /// `Ok(Ok(()))` means the intent is durable, and the rename may go ahead.
    pub(crate) fn record_intent(
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

    pub(crate) fn record_commit(
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

    pub(crate) fn contract(&self) -> &Arc<StageContract> {
        self.machine.contract()
    }

    /// Taken again by a later session in this process. A file changed under us loses the
    /// states and gives no count back; a file gone or damaged is a refusal.
    fn reconcile(&mut self, contract: &StageContract) -> Result<(), LedgerRefusal> {
        if let Some(refusal) = self.failed {
            return Err(refusal);
        }
        if self.machine.contract().sha256() != contract.sha256() {
            return Err(LedgerRefusal::ContractChanged);
        }
        let outcome = self.reconcile_with_file();
        if let Err(refusal) = outcome {
            self.failed = Some(refusal);
        }
        outcome
    }

    fn reconcile_with_file(&mut self) -> Result<(), LedgerRefusal> {
        let bytes = self.store.read_file()?.ok_or(LedgerRefusal::Missing)?;
        if self.store.holds_what_was_written(&bytes) {
            return Ok(());
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

fn open_new(directory: PathBuf, request: &LedgerRequest<'_>) -> Result<StageLedger, LedgerRefusal> {
    let mut store = Store::open(directory)?;
    let existing = store.read_file()?;
    let machine = match (request.entry, existing) {
        (LedgerEntry::Resume, None) => return Err(LedgerRefusal::Missing),
        (LedgerEntry::Resume, Some(bytes)) => lower_saved(
            decode(&bytes)?,
            &request.contract,
            &ResumeFacts {
                process_nonce: new_nonce(),
                now_ms: request.now_ms,
                bytes: request.bytes,
                pins: request.pins,
            },
        )?,
        (LedgerEntry::Fresh, existing) => {
            // Starting over is the operator's to choose, on a record that can be read. A
            // damaged one stays until the operator has looked at it.
            if let Some(bytes) = existing {
                decode(&bytes)?;
            }
            StageMachine::new(
                request.contract.clone(),
                new_nonce(),
                new_nonce(),
                request.now_ms,
            )
        }
    };
    store.write(machine.snapshot())?;
    Ok(StageLedger {
        machine,
        store,
        failed: None,
    })
}

type HeldLedger = Arc<Mutex<StageLedger>>;

/// The ledgers one process holds. Tests make their own, to stand for separate processes.
#[derive(Debug, Default)]
pub(crate) struct LedgerRegistry {
    held: Mutex<BTreeMap<(PathBuf, String), HeldLedger>>,
}

impl LedgerRegistry {
    /// Whatever the entry says, a ledger this process already holds is the one returned: only a
    /// new process starts anything over (D10).
    pub(crate) fn open(&self, request: &LedgerRequest<'_>) -> Result<HeldLedger, LedgerRefusal> {
        refuse_state_dir_inside_workspace(request.state_dir, request.workspace)?;
        let workspace = fs::canonicalize(request.workspace).map_err(unavailable)?;
        let mut identity = workspace.as_os_str().as_bytes().to_vec();
        identity.push(0);
        identity.extend_from_slice(request.profile.as_bytes());
        let directory = request
            .state_dir
            .join(LEDGER_LEAF)
            .join(sha256_hex(&identity));
        let mut held = self.held.lock().map_err(|_| LedgerRefusal::Unavailable)?;
        let key = (workspace, request.profile.to_string());
        if let Some(existing) = held.get(&key) {
            existing
                .lock()
                .map_err(|_| LedgerRefusal::Unavailable)?
                .reconcile(&request.contract)?;
            return Ok(existing.clone());
        }
        let ledger = Arc::new(Mutex::new(open_new(directory, request)?));
        held.insert(key, ledger.clone());
        Ok(ledger)
    }
}

pub(crate) fn process_ledgers() -> &'static LedgerRegistry {
    static LEDGERS: OnceLock<LedgerRegistry> = OnceLock::new();
    LEDGERS.get_or_init(LedgerRegistry::default)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::super::contract::{
        NarrowedStageLimits, StageContract, StageSetDeclaration, TargetKey, admit,
    };
    use super::super::receipt::sha256_hex;
    use super::super::resume::AdmittedPins;
    use super::super::state::{
        AttemptTicket, BeginDecision, BytesSource, CommitObservation, LocalDecision,
        MachineSnapshot, Phase, StageRefusal, UnverifiedReason,
    };
    use super::super::test_support::{
        FakeBytes, commit_for, facts, key, one_target, passed_checks, pins_for, request_for,
        schema_for, sha_of, two_stage_contract,
    };
    use super::{
        LEDGER_FILE, LEDGER_LEAF, LEDGER_TEMPORARY, LOCK_FILE, LedgerEntry, LedgerRefusal,
        LedgerRegistry, LedgerRequest, StageLedger, Store, decode, encode, process_ledgers,
        refuse_state_dir_inside_workspace,
    };

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
        assert!(verdict.complete);
        assert!(verdict.ledger_intact);

        let edited = world.bytes.with("out/alpha.json", b"edited");
        let verdict = ledger.verdict(&edited);
        assert!(!verdict.complete);
        assert_eq!(verdict.invalidated, vec![alpha(), beta()]);
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
        assert!(!ledger.verdict(&world.bytes).complete);
        // A result still on its way from the earlier run is another run's.
        assert_eq!(
            ledger.cas_check(&old_ticket).unwrap(),
            Err(StageRefusal::OtherRun)
        );
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
        assert!(!verdict.complete);
        assert!(
            verdict
                .obligations
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
            assert!(!ledger.verdict(&world.bytes).complete, "never sealed");
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
            assert!(!verdict.complete, "every target is sealed, and still no");
            assert!(!verdict.ledger_intact);
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
        assert!(!verdict.complete);
        assert!(!verdict.ledger_intact);
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
        assert!(!verdict.complete);
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
            assert!(verdict.ledger_intact);
            assert!(!verdict.complete);
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
            !verdict.complete,
            "no success once the record cannot be kept"
        );
        assert!(!verdict.ledger_intact);
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
        assert!(std::ptr::eq(process_ledgers(), process_ledgers()));
    }

    /// Not a path a test should reach by accident: the helper above asserts one directory.
    #[test]
    fn a_ledger_lives_below_its_own_leaf_of_the_state_dir() {
        let world = world();
        let held = world.process(LedgerEntry::Fresh).unwrap();
        let directory = world.directory();
        assert!(directory.starts_with(world.state_dir.join(LEDGER_LEAF)));
        assert!(!directory.starts_with(&world.workspace));
        assert!(Path::new(&world.file()).exists());
        drop(held);
    }
}
