//! A wallet on disk.
//!
//! Every change reaches the disk before it takes effect: a temporary, fsync, rename, fsync of the
//! directory. `flock` on a held descriptor refuses a second holder, in another process or in this
//! one, and goes away with its holder, so there is no stale lock. Before every change the file has
//! to be what this handle last wrote and the lock file the one it holds.
//!
//! Whichever way a process comes here, fresh or resuming, a ledger that exists is continued: what
//! it spent does not come back. A new wallet is a new ID from the operator and nothing else. The
//! one directory is the one the policy names: [`LedgerOpen`] takes no other, so a state directory
//! or a caller cannot point it elsewhere. What it cannot vouch for it refuses, and after that it
//! refuses everything.
//!
//! The directory that is judged is the directory that is used: both take a [`WalletDirectory`],
//! and only the policy makes one. No name on the way to it is somewhere a command of this run can
//! change, links and the text of links included, as far as a path can tell: only the path is
//! judged. A link out of a command's reach is allowed.
//!
//! Known limits: someone with the same user ID, outside the sandbox and before a run, can delete
//! the file or put an older one back. A fresh start then cannot tell; a resume is refused when the
//! file is gone. The same goes for an operator who rewrites `ledger_dir` in a policy: there is no
//! ledger at the new directory, so the same ID is a full wallet there. The digest of the file finds
//! damage and has no key, so it does not stop a change made on purpose.
//!
//! A profile the operator named is pinned, in its canonical name, by the first run that names one.
//! One that was only inferred is not: it can change as a run writes files, and it names no wallet.
//! So a run with no explicit profile continues the same wallet, whatever it infers. The name is
//! read by `canonical_profile_name`, the function a stage contract's check compares with, so the
//! two see one identity. Under it `cli` is not `python-cli`.
//!
//! The file holds counts, digests, amounts, fixed words, the profile's name, and G4's stage and
//! target names and run nonce: no prompt, response body, key, or header, and the workspace only as
//! a digest.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::clock::{BudgetClock, RunClock};
use super::identity::{AttemptBinding, LogicalId, LogicalKind, PhysicalId, SubmissionSource};
use super::policy::RunBudgetPolicy;
use super::reservation::{BudgetRefusal, Reservation, ReserveRequest, WalletState};
use super::settlement::{KnownBilling, ReportedUsage, SettleOutcome, SettleReport, UnknownReason};
use crate::planner::profile::canonical_profile_name;
use crate::planner::structured_stages::ledger::{
    LedgerRefusal as StageLedgerRefusal, refuse_state_dir_inside_workspace,
};
use crate::planner::structured_stages::receipt::sha256_hex;

const LEDGER_FILE: &str = "ledger.json";
const LEDGER_TEMPORARY: &str = "ledger.json.tmp";
const LOCK_FILE: &str = "lock";
const MAX_LEDGER_BYTES: u64 = 8 << 20;
/// As many as Linux follows, and more than macOS does. A ring of links stops here.
const MOST_LINKS_ON_THE_WAY: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerRefusal {
    /// A resume needs the record of the wallet it continues.
    Missing,
    Corrupt,
    Held,
    /// The operator's declaration is not the one the wallet was opened under. Another declaration
    /// is another ID.
    PolicyChanged,
    WorkspaceChanged,
    ProfileChanged,
    /// An explicit profile with nothing in its name.
    ProfileBlank,
    DirectoryInsideWorkspace,
    DirectoryWritableByCommands,
    /// Gone from under this handle during the run.
    Vanished,
    /// Not what this handle last wrote.
    Diverged,
    Unavailable,
}

/// Whether the entry asked to continue. Neither one makes or refills a wallet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerEntry {
    Fresh,
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LedgerFailure {
    Budget(BudgetRefusal),
    Ledger(LedgerRefusal),
}

impl From<BudgetRefusal> for LedgerFailure {
    fn from(refusal: BudgetRefusal) -> Self {
        Self::Budget(refusal)
    }
}

impl From<LedgerRefusal> for LedgerFailure {
    fn from(refusal: LedgerRefusal) -> Self {
        Self::Ledger(refusal)
    }
}

pub(crate) struct LedgerOpen<'a> {
    /// Also says where the wallet lives. Nothing else can.
    pub(crate) policy: Arc<RunBudgetPolicy>,
    pub(crate) workspace: &'a Path,
    /// Only a profile the operator named. An inferred one is `None`.
    pub(crate) explicit_profile: Option<&'a str>,
    pub(crate) entry: LedgerEntry,
    pub(crate) clock: Arc<dyn BudgetClock>,
}

/// One wallet, its lock held for as long as this value lives.
#[derive(Debug)]
pub(crate) struct BudgetLedger {
    policy: Arc<RunBudgetPolicy>,
    wallet: WalletState,
    store: Store,
    clock: RunClock,
    failed: Option<LedgerRefusal>,
}

/// What a POST needs: the bytes that were reserved, from a start that is on disk. Only
/// [`BudgetLedger::start`] makes one, and its fields keep anything outside this file from it. It
/// gives its bytes once, by value, so one permit is one POST.
#[must_use = "a permit is one POST, and a start with no POST leaves its send unresolved"]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StartedPermit<'a> {
    physical: PhysicalId,
    body: &'a [u8],
}

impl<'a> StartedPermit<'a> {
    pub(crate) fn physical(&self) -> PhysicalId {
        self.physical
    }

    pub(crate) fn into_body(self) -> &'a [u8] {
        self.body
    }
}

mod wallet_directory {
    use std::path::{Path, PathBuf};

    use super::super::policy::RunBudgetPolicy;

    /// The one directory of a wallet. Its field keeps every other path from being one, so what is
    /// checked cannot be other than what is opened.
    #[derive(Debug)]
    pub(super) struct WalletDirectory(PathBuf);

    impl WalletDirectory {
        pub(super) fn of(policy: &RunBudgetPolicy) -> Self {
            Self(policy.ledger_dir.join(&policy.budget_id))
        }

        pub(super) fn path(&self) -> &Path {
            &self.0
        }
    }
}

use wallet_directory::WalletDirectory;

#[derive(Debug)]
struct Store {
    directory: WalletDirectory,
    lock: File,
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

fn unavailable(_: std::io::Error) -> LedgerRefusal {
    LedgerRefusal::Unavailable
}

/// What a failed `flock` with `LOCK_NB` says: only "someone holds it" is a holder.
fn lock_refusal(code: Option<i32>) -> LedgerRefusal {
    match code {
        Some(code) if code == libc::EWOULDBLOCK || code == libc::EAGAIN => LedgerRefusal::Held,
        _ => LedgerRefusal::Unavailable,
    }
}

/// The same bound for what is read and what is written, so what is written can be read.
const fn fits_a_ledger(bytes: u64) -> bool {
    bytes <= MAX_LEDGER_BYTES
}

/// The one name of a profile, as a stage contract's check reads it, or the refusal of no name.
fn canonical_explicit_profile(name: &str) -> Result<String, LedgerRefusal> {
    let canonical = canonical_profile_name(name);
    if canonical.is_empty() {
        return Err(LedgerRefusal::ProfileBlank);
    }
    Ok(canonical)
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

fn encode(wallet: &WalletState) -> Result<Vec<u8>, LedgerRefusal> {
    let body = serde_json::to_string(wallet).map_err(|_| LedgerRefusal::Unavailable)?;
    let sha256 = sha256_hex(body.as_bytes());
    let body = RawValue::from_string(body).map_err(|_| LedgerRefusal::Unavailable)?;
    serde_json::to_vec(&EnvelopeOut {
        sha256: &sha256,
        body: &body,
    })
    .map_err(|_| LedgerRefusal::Unavailable)
}

fn decode(bytes: &[u8]) -> Result<WalletState, LedgerRefusal> {
    let envelope: EnvelopeIn = serde_json::from_slice(bytes).map_err(|_| LedgerRefusal::Corrupt)?;
    if sha256_hex(envelope.body.get().as_bytes()) != envelope.sha256 {
        return Err(LedgerRefusal::Corrupt);
    }
    serde_json::from_str(envelope.body.get()).map_err(|_| LedgerRefusal::Corrupt)
}

impl Store {
    fn open(directory: WalletDirectory) -> Result<Self, LedgerRefusal> {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory.path())
            .map_err(unavailable)?;
        let metadata = fs::symlink_metadata(directory.path()).map_err(unavailable)?;
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
            .open(directory.path().join(LOCK_FILE))
            .map_err(unavailable)?;
        if !lock.metadata().map_err(unavailable)?.is_file() {
            return Err(LedgerRefusal::Unavailable);
        }
        // SAFETY: `lock` is an open descriptor this function owns, and flock only uses it.
        // LOCK_NB makes it return at once instead of waiting for the other holder.
        let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            return Err(lock_refusal(std::io::Error::last_os_error().raw_os_error()));
        }
        Ok(Self {
            directory,
            lock,
            expected_sha256: None,
        })
    }

    /// `None` only when nothing is there. A symlink, or something too large to be a ledger, is
    /// damage. `O_NONBLOCK` is for a FIFO in its place: the open returns, and the file-type check
    /// refuses it.
    fn read_file(&self) -> Result<Option<Vec<u8>>, LedgerRefusal> {
        let opened = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(self.directory.path().join(LEDGER_FILE));
        let file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                return Err(LedgerRefusal::Corrupt);
            }
            Err(error) => return Err(unavailable(error)),
        };
        if !file.metadata().map_err(unavailable)?.is_file() {
            return Err(LedgerRefusal::Corrupt);
        }
        let mut bytes = Vec::new();
        file.take(MAX_LEDGER_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(unavailable)?;
        if !fits_a_ledger(bytes.len() as u64) {
            return Err(LedgerRefusal::Corrupt);
        }
        Ok(Some(bytes))
    }

    /// Temporary, fsync, rename, fsync of the directory: on return it survives a crash, and a
    /// crash before that leaves the previous version whole.
    fn write(&mut self, wallet: &WalletState) -> Result<(), LedgerRefusal> {
        let bytes = encode(wallet)?;
        if !fits_a_ledger(bytes.len() as u64) {
            return Err(LedgerRefusal::Unavailable);
        }
        let temporary = self.directory.path().join(LEDGER_TEMPORARY);
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
        fs::rename(&temporary, self.directory.path().join(LEDGER_FILE)).map_err(unavailable)?;
        File::open(self.directory.path())
            .and_then(|directory| directory.sync_all())
            .map_err(unavailable)?;
        self.expected_sha256 = Some(sha256_hex(&bytes));
        Ok(())
    }

    /// The lock at the path is the one held, and the file is what this handle last wrote. A lock
    /// file replaced under a holder would let the next process hold another one.
    fn verify_unchanged(&self) -> Result<(), LedgerRefusal> {
        let held = self.lock.metadata().map_err(unavailable)?;
        let same_lock = fs::symlink_metadata(self.directory.path().join(LOCK_FILE))
            .is_ok_and(|found| found.dev() == held.dev() && found.ino() == held.ino());
        if !same_lock {
            return Err(LedgerRefusal::Vanished);
        }
        match self.read_file()? {
            None => Err(LedgerRefusal::Vanished),
            Some(bytes) if self.expected_sha256.as_deref() == Some(&sha256_hex(&bytes)) => Ok(()),
            Some(_) => Err(LedgerRefusal::Diverged),
        }
    }
}

/// The check G4 made for its own ledger, of one place: where it really leads.
fn refuse_place(place: &Path, workspace: &Path) -> Result<(), LedgerRefusal> {
    refuse_state_dir_inside_workspace(place, workspace).map_err(|refusal| match refusal {
        StageLedgerRefusal::StateDirInsideWorkspace => LedgerRefusal::DirectoryInsideWorkspace,
        StageLedgerRefusal::StateDirWritableByCommands => {
            LedgerRefusal::DirectoryWritableByCommands
        }
        _ => LedgerRefusal::Unavailable,
    })
}

/// Walks the path as the system does, one name at a time and with no link followed unseen, and
/// judges each real directory it enters. So a directory holding a name of the way, even a name in
/// the text of a link, has been judged before that name is read. What is not there yet ends the
/// walk: nothing below it can be a link, and [`refuse_place`] has already judged the plain tail.
fn refuse_every_name_on_the_way(
    directory: &WalletDirectory,
    workspace: &Path,
) -> Result<(), LedgerRefusal> {
    let mut place = PathBuf::new();
    let mut rest = directory.path().to_path_buf();
    let mut followed = 0;
    loop {
        let mut names = rest.components();
        let Some(name) = names.next() else {
            return Ok(());
        };
        let after = names.as_path().to_path_buf();
        rest = match name {
            Component::RootDir => {
                place = PathBuf::from(Component::RootDir.as_os_str());
                after
            }
            Component::CurDir => after,
            // `place` holds no link, so its parent is the directory the system climbs to, and has
            // been judged on the way down.
            Component::ParentDir => {
                place.pop();
                after
            }
            Component::Prefix(_) => return Err(LedgerRefusal::Unavailable),
            Component::Normal(name) => {
                let next = place.join(name);
                match fs::symlink_metadata(&next) {
                    Ok(found) if found.is_symlink() => {
                        followed += 1;
                        if followed > MOST_LINKS_ON_THE_WAY {
                            return Err(LedgerRefusal::Unavailable);
                        }
                        fs::read_link(&next).map_err(unavailable)?.join(after)
                    }
                    Ok(_) => {
                        refuse_place(&next, workspace)?;
                        place = next;
                        after
                    }
                    Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                    Err(error) => return Err(unavailable(error)),
                }
            }
        };
    }
}

/// A directory commands of the run can write proves nothing, and neither does a path that runs
/// through one: the place is judged by where it leads, and every use walks the path again. So both
/// are judged. Where it leads comes first, so its refusals stay as G4 words them.
fn refuse_directory(directory: &WalletDirectory, workspace: &Path) -> Result<(), LedgerRefusal> {
    refuse_place(directory.path(), workspace)?;
    refuse_every_name_on_the_way(directory, workspace)
}

/// The wallet already on disk, held to the declaration and the run that opened it.
fn continue_wallet(
    bytes: &[u8],
    policy: &RunBudgetPolicy,
    workspace_sha256: &str,
    explicit_profile: Option<&str>,
) -> Result<WalletState, LedgerRefusal> {
    let wallet = decode(bytes)?;
    if wallet.budget_id != policy.budget_id {
        return Err(LedgerRefusal::Corrupt);
    }
    if wallet.policy_sha256 != policy.sha256 {
        return Err(LedgerRefusal::PolicyChanged);
    }
    if wallet.workspace_sha256 != workspace_sha256 {
        return Err(LedgerRefusal::WorkspaceChanged);
    }
    if let (Some(pinned), Some(named)) = (wallet.profile.as_deref(), explicit_profile)
        && pinned != named
    {
        return Err(LedgerRefusal::ProfileChanged);
    }
    if !wallet.is_consistent(policy) {
        return Err(LedgerRefusal::Corrupt);
    }
    Ok(wallet)
}

impl BudgetLedger {
    pub(crate) fn open(request: LedgerOpen<'_>) -> Result<Self, LedgerRefusal> {
        let policy = request.policy;
        let directory = WalletDirectory::of(&policy);
        refuse_directory(&directory, request.workspace)?;
        let workspace = fs::canonicalize(request.workspace).map_err(unavailable)?;
        let workspace_sha256 = sha256_hex(workspace.as_os_str().as_bytes());
        let explicit_profile = request
            .explicit_profile
            .map(canonical_explicit_profile)
            .transpose()?;
        let mut store = Store::open(directory)?;
        let wallet = match store.read_file()? {
            Some(bytes) => {
                let found = continue_wallet(
                    &bytes,
                    &policy,
                    &workspace_sha256,
                    explicit_profile.as_deref(),
                )?;
                store.expected_sha256 = Some(sha256_hex(&bytes));
                let mut wallet = found.clone();
                wallet.recover_after_process_end();
                if wallet.profile.is_none() {
                    wallet.profile = explicit_profile;
                }
                if wallet != found {
                    store.write(&wallet)?;
                }
                wallet
            }
            None if request.entry == LedgerEntry::Resume => return Err(LedgerRefusal::Missing),
            None => {
                let wallet = WalletState::new(&policy, workspace_sha256, explicit_profile);
                store.write(&wallet)?;
                wallet
            }
        };
        Ok(Self {
            policy,
            wallet,
            store,
            clock: RunClock::new(request.clock),
            failed: None,
        })
    }

    pub(crate) fn wallet(&self) -> &WalletState {
        &self.wallet
    }

    /// A refusal of the file is for good: there is no way back to a state this handle cannot
    /// vouch for.
    fn verified(&mut self) -> Result<(), LedgerRefusal> {
        if let Some(refusal) = self.failed {
            return Err(refusal);
        }
        self.store.verify_unchanged().inspect_err(|refusal| {
            self.failed = Some(*refusal);
        })
    }

    /// The change on a copy, the copy on disk, and only then the change here.
    fn apply<R>(
        &mut self,
        change: impl FnOnce(
            &mut WalletState,
            &RunBudgetPolicy,
            &mut RunClock,
        ) -> Result<R, BudgetRefusal>,
    ) -> Result<R, LedgerFailure> {
        self.verified()?;
        let mut next = self.wallet.clone();
        self.clock.observe(&mut next.clock);
        let result = change(&mut next, &self.policy, &mut self.clock)?;
        if let Err(refusal) = self.store.write(&next) {
            self.failed = Some(refusal);
            return Err(refusal.into());
        }
        self.wallet = next;
        Ok(result)
    }

    pub(crate) fn reserve(
        &mut self,
        request: &ReserveRequest<'_>,
    ) -> Result<Reservation, LedgerFailure> {
        self.apply(|wallet, policy, clock| wallet.reserve(policy, request, clock))
    }

    /// The start is on disk when this returns, and the permit is what a POST takes its bytes from.
    pub(crate) fn start<'a>(
        &mut self,
        physical: PhysicalId,
        body: &'a [u8],
    ) -> Result<StartedPermit<'a>, LedgerFailure> {
        self.apply(|wallet, policy, _| wallet.start(policy, physical, body))?;
        Ok(StartedPermit { physical, body })
    }

    pub(crate) fn release(&mut self, physical: PhysicalId) -> Result<(), LedgerFailure> {
        self.apply(|wallet, _, _| wallet.release(physical))
    }

    pub(crate) fn settle(
        &mut self,
        physical: PhysicalId,
        billing: &KnownBilling,
    ) -> Result<SettleOutcome, LedgerFailure> {
        self.apply(|wallet, policy, _| wallet.settle(policy, physical, billing))
    }

    /// One response's report, which leaves the send settled or unknown on disk.
    pub(crate) fn settle_reported(
        &mut self,
        physical: PhysicalId,
        reported: &ReportedUsage<'_>,
    ) -> Result<SettleReport, LedgerFailure> {
        self.apply(|wallet, policy, _| wallet.settle_reported(policy, physical, reported))
    }

    pub(crate) fn mark_unknown(
        &mut self,
        physical: PhysicalId,
        reason: UnknownReason,
    ) -> Result<(), LedgerFailure> {
        self.apply(|wallet, _, _| wallet.mark_unknown(physical, reason))
    }

    pub(crate) fn record_submission(
        &mut self,
        kind: LogicalKind,
        source: Option<SubmissionSource>,
    ) -> Result<LogicalId, LedgerFailure> {
        self.apply(|wallet, policy, clock| wallet.record_submission(policy, kind, source, clock))
    }

    pub(crate) fn bind_attempt(
        &mut self,
        logical: LogicalId,
        binding: AttemptBinding,
    ) -> Result<(), LedgerFailure> {
        self.apply(|wallet, _, _| wallet.bind_attempt(logical, binding))
    }

    /// Whether the run may be called a success: the file still is what this handle wrote, and the
    /// wallet has nothing unresolved and no latch.
    pub(crate) fn success_gate(&mut self) -> Result<(), LedgerFailure> {
        self.verified()?;
        self.wallet.success_gate().map_err(LedgerFailure::from)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::super::identity::{
        FinalRepairClaim, FinalRepairId, LogicalKind, PhysicalId, ResponseIdentity, SendPurpose,
    };
    use super::super::policy::RunBudgetPolicy;
    use super::super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money, WalletState};
    use super::super::settlement::{KnownBilling, ReportedUsage, SettleReport, UnknownReason};
    use super::super::test_support::{
        BODY, FakeClock, Rng, WALLET, policy_value, request, stage_kind,
    };
    use super::{
        BudgetLedger, LEDGER_FILE, LEDGER_TEMPORARY, LOCK_FILE, LedgerEntry, LedgerFailure,
        LedgerOpen, LedgerRefusal, MAX_LEDGER_BYTES, MOST_LINKS_ON_THE_WAY, Store, WalletDirectory,
        fits_a_ledger, lock_refusal, refuse_directory, refuse_every_name_on_the_way,
    };
    use crate::planner::profile::canonical_profile_name;
    use crate::planner::structured_stages::receipt::sha256_hex;
    use crate::providers::BilledCostUsd;

    struct World {
        // Held so the directories outlive the test.
        _root: tempfile::TempDir,
        ledgers: PathBuf,
        workspace: PathBuf,
        policy: Arc<RunBudgetPolicy>,
        clock: Arc<FakeClock>,
    }

    impl World {
        fn new() -> Self {
            Self::with(|_| {})
        }

        /// A policy whose `ledger_dir` is this test's own directory, after the test's changes.
        fn with(change: impl FnOnce(&mut Value)) -> Self {
            let root = tempfile::tempdir().unwrap();
            let base = fs::canonicalize(root.path()).unwrap();
            let workspace = base.join("workspace");
            fs::create_dir_all(&workspace).unwrap();
            let ledgers = base.join("operator-ledgers");
            let mut value = policy_value();
            value["ledger_dir"] = json!(ledgers.to_str().unwrap());
            change(&mut value);
            let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
            Self {
                ledgers,
                workspace,
                _root: root,
                policy: Arc::new(policy),
                clock: FakeClock::at(7_000),
            }
        }

        /// Another declaration, in this world's directory unless it says otherwise.
        fn declaration(&self, id: &str, change: impl FnOnce(&mut Value)) -> Arc<RunBudgetPolicy> {
            let mut value = policy_value();
            value["budget_id"] = json!(id);
            value["ledger_dir"] = json!(self.ledgers.to_str().unwrap());
            change(&mut value);
            Arc::new(RunBudgetPolicy::decode(value.to_string().as_bytes(), id).unwrap())
        }

        fn open_under(
            &self,
            policy: Arc<RunBudgetPolicy>,
            profile: Option<&str>,
        ) -> Result<BudgetLedger, LedgerRefusal> {
            BudgetLedger::open(LedgerOpen {
                policy,
                workspace: &self.workspace,
                explicit_profile: profile,
                entry: LedgerEntry::Fresh,
                clock: self.clock.clone(),
            })
        }

        fn open_as(
            &self,
            entry: LedgerEntry,
            workspace: &Path,
            profile: Option<&str>,
        ) -> Result<BudgetLedger, LedgerRefusal> {
            BudgetLedger::open(LedgerOpen {
                policy: self.policy.clone(),
                workspace,
                explicit_profile: profile,
                entry,
                clock: self.clock.clone(),
            })
        }

        fn open(&self, entry: LedgerEntry) -> Result<BudgetLedger, LedgerRefusal> {
            self.open_as(entry, &self.workspace, Some("profile-a"))
        }

        fn open_naming(&self, profile: Option<&str>) -> Result<BudgetLedger, LedgerRefusal> {
            self.open_as(LedgerEntry::Fresh, &self.workspace, profile)
        }

        fn wallet_dir(&self) -> PathBuf {
            self.ledgers.join(WALLET)
        }

        fn file(&self) -> PathBuf {
            self.wallet_dir().join(LEDGER_FILE)
        }

        /// The file with a change to its body, under a digest that holds for what it now says.
        fn rewrite(&self, change: impl FnOnce(&mut Value)) {
            let mut body = on_disk(self);
            change(&mut body);
            let body = body.to_string();
            let sha256 = sha256_hex(body.as_bytes());
            let envelope = format!("{{\"sha256\":\"{sha256}\",\"body\":{body}}}");
            fs::write(self.file(), envelope).unwrap();
        }
    }

    fn known(prompt: u64, completion: u64) -> KnownBilling {
        let cost = BilledCostUsd::from_json_token("0").unwrap();
        KnownBilling::from_reported(&ReportedUsage {
            cost: Some(&cost),
            prompt_tokens: Some(prompt),
            completion_tokens: Some(completion),
            ..ReportedUsage::default()
        })
        .unwrap()
    }

    fn reserve(ledger: &mut BudgetLedger) -> Result<PhysicalId, LedgerFailure> {
        ledger
            .reserve(&request())
            .map(|reservation| reservation.physical)
    }

    fn send(ledger: &mut BudgetLedger) -> PhysicalId {
        let id = reserve(ledger).unwrap();
        let _permit = ledger.start(id, BODY).unwrap();
        ledger.settle(id, &known(100, 40)).unwrap();
        id
    }

    fn reserve_final_repair(
        ledger: &mut BudgetLedger,
        claim: FinalRepairClaim,
    ) -> Result<super::Reservation, LedgerFailure> {
        let mut body = request();
        body.purpose = SendPurpose::FinalRepair;
        body.final_repair = Some(claim);
        ledger.reserve(&body)
    }

    fn new_final_repair(ledger: &mut BudgetLedger) -> PhysicalId {
        reserve_final_repair(ledger, FinalRepairClaim::New)
            .unwrap()
            .physical
    }

    fn on_disk(world: &World) -> serde_json::Value {
        let bytes = fs::read(world.file()).unwrap();
        let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        envelope["body"].clone()
    }

    fn money(text: &str) -> Money {
        Money::parse_decimal_up(text).unwrap()
    }

    // ---- fresh, resume, and the wallet's identity ----

    #[test]
    fn a_fresh_open_of_an_existing_ledger_gets_no_budget_back() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut first);
        drop(first);
        let second = world.open(LedgerEntry::Fresh).unwrap();
        assert_eq!(second.wallet().spent().unwrap(), money("0.0006"));
        assert_eq!(second.wallet().physical_used(), 1);
    }

    #[test]
    fn a_resume_continues_the_same_balance_and_counts() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        let id = send(&mut first);
        drop(first);
        let mut second = world.open(LedgerEntry::Resume).unwrap();
        assert_eq!(second.wallet().spent().unwrap(), money("0.0006"));
        let next = send(&mut second);
        assert!(next > id);
        assert_eq!(second.wallet().spent().unwrap(), money("0.0012"));
    }

    #[test]
    fn a_fresh_open_and_a_resume_reach_the_same_wallet_with_the_limits_spent() {
        let world = World::with(|v| v["limits"]["physical_sends"] = json!(1));
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut first);
        drop(first);
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            let mut again = world.open(entry).unwrap();
            assert_eq!(
                reserve(&mut again),
                Err(LedgerFailure::Budget(BudgetRefusal::PhysicalExceeded)),
                "{entry:?}"
            );
        }
    }

    #[test]
    fn a_resume_with_no_ledger_is_refused_and_writes_nothing() {
        let world = World::new();
        assert_eq!(
            world.open(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Missing
        );
        assert!(!world.file().exists());
    }

    #[test]
    fn a_fresh_open_makes_the_record_at_once_so_a_resume_finds_it() {
        let world = World::new();
        let first = world.open(LedgerEntry::Fresh).unwrap();
        assert!(world.file().exists());
        drop(first);
        assert!(world.open(LedgerEntry::Resume).is_ok());
    }

    /// The pattern names every field and has no `..`: a directory, or any other input, added to
    /// what opens a wallet stops this from compiling. What is left to say where it lives is the
    /// policy, and the file is there.
    #[test]
    fn the_handle_has_no_state_directory_to_make_another_wallet_with() {
        let world = World::new();
        let request = LedgerOpen {
            policy: world.policy.clone(),
            workspace: &world.workspace,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: world.clock.clone(),
        };
        let LedgerOpen {
            policy,
            workspace,
            explicit_profile,
            entry,
            clock,
        } = request;
        let mut first = BudgetLedger::open(LedgerOpen {
            policy,
            workspace,
            explicit_profile,
            entry,
            clock,
        })
        .unwrap();
        send(&mut first);
        drop(first);
        assert!(
            world
                .policy
                .ledger_dir
                .join(WALLET)
                .join(LEDGER_FILE)
                .exists()
        );
        let second = world.open_naming(None).unwrap();
        assert_eq!(second.wallet().spent().unwrap(), money("0.0006"));
        assert_eq!(
            second.store.directory.path(),
            world.policy.ledger_dir.join(WALLET)
        );
    }

    /// Known limit: the file's digest and the policy's cannot reach a directory with nothing in
    /// it, so an operator who rewrites `ledger_dir` has a full wallet there under the same ID.
    #[test]
    fn a_policy_rewritten_to_another_directory_is_a_full_wallet_there() {
        let world = World::with(|v| v["limits"]["physical_sends"] = json!(1));
        let mut first = world.open_naming(None).unwrap();
        send(&mut first);
        drop(first);
        let elsewhere = world.ledgers.parent().unwrap().join("elsewhere");
        let moved = world.declaration(WALLET, |v| {
            v["limits"]["physical_sends"] = json!(1);
            v["ledger_dir"] = json!(elsewhere.to_str().unwrap());
        });
        let second = world.open_under(moved, None).unwrap();
        assert_eq!(second.wallet().physical_used(), 0);
        assert!(elsewhere.join(WALLET).join(LEDGER_FILE).exists());
    }

    #[test]
    fn the_first_workspace_is_pinned() {
        let world = World::new();
        drop(world.open(LedgerEntry::Fresh).unwrap());
        let other = world.ledgers.parent().unwrap().join("other-workspace");
        fs::create_dir_all(&other).unwrap();
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            assert_eq!(
                world.open_as(entry, &other, Some("profile-a")).unwrap_err(),
                LedgerRefusal::WorkspaceChanged,
                "{entry:?}"
            );
        }
    }

    #[test]
    fn two_explicit_profiles_that_differ_are_refused() {
        let world = World::new();
        drop(world.open_naming(Some("profile-a")).unwrap());
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            assert_eq!(
                world
                    .open_as(entry, &world.workspace, Some("profile-b"))
                    .unwrap_err(),
                LedgerRefusal::ProfileChanged,
                "{entry:?}"
            );
        }
        // A refused open pinned nothing.
        assert_eq!(on_disk(&world)["profile"], json!("profile-a"));
    }

    #[test]
    fn the_names_of_one_profile_are_one_profile() {
        let names = ["python-cli", "py-cli", "python", "py", " PY-CLI "];
        for first in names {
            let world = World::new();
            drop(world.open_naming(Some(first)).unwrap());
            assert_eq!(on_disk(&world)["profile"], json!("python-cli"), "{first}");
            for again in names {
                assert!(world.open_naming(Some(again)).is_ok(), "{first} {again}");
            }
            assert_eq!(
                world.open_naming(Some("generic")).unwrap_err(),
                LedgerRefusal::ProfileChanged,
                "{first}"
            );
        }
    }

    /// `cli` is another name for `python-cli` in the descriptor's table, and not to the check of a
    /// stage contract, which is the identity a pin has to share.
    #[test]
    fn cli_and_python_cli_are_two_profiles_as_a_stage_contract_reads_them() {
        let world = World::new();
        drop(world.open_naming(Some("python-cli")).unwrap());
        assert_eq!(
            world.open_naming(Some("cli")).unwrap_err(),
            LedgerRefusal::ProfileChanged
        );

        let world = World::new();
        drop(world.open_naming(Some(" CLI ")).unwrap());
        assert_eq!(on_disk(&world)["profile"], json!("cli"));
        assert!(world.open_naming(Some("cli")).is_ok());
        for other in ["python-cli", "py-cli", "py"] {
            assert_eq!(
                world.open_naming(Some(other)).unwrap_err(),
                LedgerRefusal::ProfileChanged,
                "{other}"
            );
        }
    }

    /// `names_the_same_profile` is `canonical_profile_name` of each side, and it is private to the
    /// module it decides for. This holds a pin to that same comparison, both ways, over every pair.
    #[test]
    fn a_pin_agrees_with_the_comparison_a_stage_contract_makes() {
        let names = [
            "python-cli",
            "py-cli",
            "py",
            "python",
            "cli",
            "nextjs",
            "next-js",
            " Next.JS ",
            "generic",
        ];
        for first in names {
            for second in names {
                let world = World::new();
                drop(world.open_naming(Some(first)).unwrap());
                let pinned = on_disk(&world)["profile"].as_str().unwrap().to_string();
                assert_eq!(pinned, canonical_profile_name(first), "{first}");
                assert_eq!(canonical_profile_name(&pinned), pinned, "{first}");
                let same = canonical_profile_name(first) == canonical_profile_name(second);
                let opened = world.open_naming(Some(second)).map(|_| ());
                let expected = if same {
                    Ok(())
                } else {
                    Err(LedgerRefusal::ProfileChanged)
                };
                assert_eq!(opened, expected, "{first} {second}");
            }
        }
    }

    #[test]
    fn a_blank_explicit_profile_is_refused_and_pins_nothing() {
        for blank in ["", "   ", "\t\n"] {
            let world = World::new();
            assert_eq!(
                world.open_naming(Some(blank)).unwrap_err(),
                LedgerRefusal::ProfileBlank,
                "{blank:?}"
            );
            assert!(!world.wallet_dir().exists(), "{blank:?}");

            drop(world.open_naming(None).unwrap());
            assert_eq!(
                world.open_naming(Some(blank)).unwrap_err(),
                LedgerRefusal::ProfileBlank,
                "{blank:?}"
            );
            assert_eq!(on_disk(&world)["profile"], Value::Null, "{blank:?}");
            // A name is still free to pin it.
            assert!(world.open_naming(Some("generic")).is_ok(), "{blank:?}");
        }
    }

    /// An inferred profile is `None` here. It changes as a run writes files, and it does not make
    /// or refill a wallet: the balance goes on through every way of naming or not naming one.
    #[test]
    fn no_explicit_profile_is_the_same_wallet_with_the_same_balance() {
        let world = World::with(|v| v["limits"]["physical_sends"] = json!(2));
        let mut first = world.open_naming(Some("python-cli")).unwrap();
        send(&mut first);
        drop(first);
        let mut second = world.open_naming(None).unwrap();
        assert_eq!(second.wallet().physical_used(), 1);
        assert_eq!(second.wallet().spent().unwrap(), money("0.0006"));
        send(&mut second);
        drop(second);
        // It kept the pin it had, and the limit still counts both.
        assert_eq!(on_disk(&world)["profile"], json!("python-cli"));
        let mut third = world.open_naming(Some("py")).unwrap();
        assert_eq!(third.wallet().physical_used(), 2);
        assert_eq!(
            reserve(&mut third),
            Err(LedgerFailure::Budget(BudgetRefusal::PhysicalExceeded))
        );
    }

    #[test]
    fn the_first_explicit_profile_pins_a_wallet_that_had_none_and_gets_nothing_back() {
        let world = World::new();
        let mut first = world.open_naming(None).unwrap();
        send(&mut first);
        drop(first);
        assert_eq!(on_disk(&world)["profile"], Value::Null);
        let second = world.open_naming(Some("py-cli")).unwrap();
        assert_eq!(second.wallet().physical_used(), 1);
        drop(second);
        assert_eq!(on_disk(&world)["profile"], json!("python-cli"));
        assert_eq!(
            world.open_naming(Some("generic")).unwrap_err(),
            LedgerRefusal::ProfileChanged
        );
    }

    #[test]
    fn opening_with_nothing_to_change_writes_nothing() {
        let world = World::new();
        drop(world.open_naming(Some("python-cli")).unwrap());
        let before = fs::read(world.file()).unwrap();
        drop(world.open_naming(Some("py")).unwrap());
        drop(world.open_naming(None).unwrap());
        assert_eq!(fs::read(world.file()).unwrap(), before);
    }

    #[test]
    fn another_name_for_the_workspace_is_the_same_wallet() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut first);
        drop(first);
        let alias = world.ledgers.parent().unwrap().join("workspace-alias");
        std::os::unix::fs::symlink(&world.workspace, &alias).unwrap();
        let second = world
            .open_as(LedgerEntry::Fresh, &alias, Some("profile-a"))
            .unwrap();
        assert_eq!(second.wallet().physical_used(), 1);
    }

    #[test]
    fn a_changed_declaration_is_refused_and_a_new_id_is_a_new_wallet() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut first);
        drop(first);

        // The operator widens the same ID.
        let widened = world.declaration(WALLET, |v| v["limits"]["cost_usd"] = json!("9999"));
        let refused = world.open_under(widened, Some("profile-a"));
        assert_eq!(refused.unwrap_err(), LedgerRefusal::PolicyChanged);

        // The operator issues a new ID, with the same directory.
        let issued = world.declaration("wallet-beta", |_| {});
        let wallet = world.open_under(issued, Some("profile-a")).unwrap();
        assert_eq!(wallet.wallet().physical_used(), 0);
        assert!(world.file().exists());
        let beta = world.ledgers.join("wallet-beta").join(LEDGER_FILE);
        assert!(beta.exists());
    }

    // ---- crash ----

    #[test]
    fn a_process_that_ended_with_a_reservation_releases_it_on_reopen() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut first).unwrap();
        drop(first);
        for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
            let mut second = world.open(entry).unwrap();
            let attempt = second.wallet().attempt(id).unwrap();
            assert_eq!(attempt.state, AttemptState::Released, "{entry:?}");
            assert_eq!(second.wallet().held().unwrap(), Money::ZERO);
            assert!(second.wallet().latch().is_none());
            assert_eq!(second.success_gate(), Ok(()));
        }
        // The release itself is on disk.
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("released"));
    }

    #[test]
    fn a_process_that_ended_with_a_started_send_stops_the_wallet_on_reopen() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut first).unwrap();
        let _permit = first.start(id, BODY).unwrap();
        drop(first);
        for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
            let mut second = world.open(entry).unwrap();
            let attempt = second.wallet().attempt(id).unwrap();
            assert_eq!(attempt.state, AttemptState::Unknown, "{entry:?}");
            assert_eq!(
                attempt.unknown_reason,
                Some(UnknownReason::InterruptedAfterStart)
            );
            assert_eq!(second.wallet().held().unwrap(), money("0.00072"));
            assert_eq!(
                second.wallet().latch().unwrap().reason,
                LatchReason::InterruptedAfterStart
            );
            assert_eq!(
                reserve(&mut second),
                Err(LedgerFailure::Budget(BudgetRefusal::Latched))
            );
            assert_eq!(
                second.success_gate(),
                Err(LedgerFailure::Budget(BudgetRefusal::Latched))
            );
        }
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("unknown"));
    }

    #[test]
    fn a_latch_stays_through_every_later_process() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut first).unwrap();
        let _permit = first.start(id, BODY).unwrap();
        first.mark_unknown(id, UnknownReason::Timeout).unwrap();
        first.settle(id, &known(100, 40)).unwrap();
        drop(first);
        let mut second = world.open(LedgerEntry::Fresh).unwrap();
        assert_eq!(
            second.wallet().latch().unwrap().reason,
            LatchReason::UnknownBilling
        );
        assert_eq!(
            reserve(&mut second),
            Err(LedgerFailure::Budget(BudgetRefusal::Latched))
        );
    }

    // ---- durable before it takes effect ----

    #[test]
    fn every_change_is_on_disk_when_it_returns() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("reserved"));
        let _permit = ledger.start(id, BODY).unwrap();
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("started"));
        ledger.mark_unknown(id, UnknownReason::Cancelled).unwrap();
        let disk = on_disk(&world);
        assert_eq!(disk["attempts"][0]["state"], json!("unknown"));
        assert_eq!(disk["latch"]["reason"], json!("unknown_billing"));
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("settled"));

        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        ledger.release(id).unwrap();
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("released"));
    }

    #[test]
    fn a_change_that_cannot_reach_the_disk_takes_no_effect_and_refuses_everything_after() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let before = ledger.wallet().clone();
        // A directory where the temporary goes: the write fails, whoever runs the test.
        fs::create_dir(world.ledgers.join(WALLET).join(LEDGER_TEMPORARY)).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(ledger.wallet(), &before);
        fs::remove_dir(world.ledgers.join(WALLET).join(LEDGER_TEMPORARY)).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(
            ledger.success_gate(),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
    }

    #[test]
    fn a_refused_reservation_writes_nothing() {
        let world = World::with(|v| v["limits"]["tokens"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let before = fs::read(world.file()).unwrap();
        world.clock.advance(1_000);
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Budget(BudgetRefusal::TokensExceeded))
        );
        assert_eq!(fs::read(world.file()).unwrap(), before);
    }

    #[test]
    fn the_file_holds_no_request_bytes_or_host_path() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let canary = r#"{"model":"m-1","max_tokens":50,"reasoning_effort":"low","provider":{"only":["u"],"allow_fallbacks":false},"messages":[{"role":"user","content":"canary-prompt-body-text"}]}"#;
        let mut body = request();
        body.body = canary.as_bytes();
        let reservation = ledger.reserve(&body).unwrap();
        let _permit = ledger
            .start(reservation.physical, canary.as_bytes())
            .unwrap();
        ledger.settle(reservation.physical, &known(1, 1)).unwrap();
        let text = String::from_utf8(fs::read(world.file()).unwrap()).unwrap();
        assert!(!text.contains("canary-prompt-body-text"));
        assert!(!text.contains(world.workspace.to_str().unwrap()));
    }

    // ---- damage ----

    #[test]
    fn a_damaged_ledger_is_refused_and_left_in_place() {
        for bytes in [
            &b"not json"[..],
            &b"{}"[..],
            &b""[..],
            &br#"{"sha256":"00","body":{}}"#[..],
        ] {
            let world = World::new();
            drop(world.open(LedgerEntry::Fresh).unwrap());
            fs::write(world.file(), bytes).unwrap();
            for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
                assert_eq!(
                    world.open(entry).unwrap_err(),
                    LedgerRefusal::Corrupt,
                    "{entry:?}"
                );
            }
            assert_eq!(fs::read(world.file()).unwrap(), bytes);
        }
    }

    /// The envelope's digest is right, so only the content can tell.
    #[test]
    fn a_ledger_whose_digest_holds_but_whose_content_disagrees_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut ledger);
        drop(ledger);
        world.rewrite(|body| body["attempts"][0]["settlement"]["consumed"] = json!("0"));
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    /// The same check as `a_submission_cannot_name...`, on the way back in: the digest holds, and
    /// the send the submission names was released, not settled.
    #[test]
    fn a_ledger_with_a_submission_of_a_send_that_was_not_settled_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let settled = send(&mut ledger);
        let response = super::super::identity::ResponseIdentity::of_settled(settled);
        ledger
            .record_submission(LogicalKind::General, Some(response.call(0)))
            .unwrap();
        let released = reserve(&mut ledger).unwrap();
        ledger.release(released).unwrap();
        drop(ledger);
        // Its control: the same file with the source untouched opens.
        drop(world.open(LedgerEntry::Resume).unwrap());
        world.rewrite(|body| body["logicals"][0]["source"]["physical"] = json!(released));
        assert_eq!(
            world.open(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    /// A rate of zero for cached tokens is allowed, and a body that comes to nothing is not: a file
    /// that says one did is refused with its digest right. Its control, one token of output, opens.
    #[test]
    fn a_ledger_with_a_body_settled_at_nothing_is_refused() {
        let world = World::with(|v| v["tariff"]["cached_input_per_million"] = json!("0"));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let _permit = ledger.start(id, BODY).unwrap();
        let cost = BilledCostUsd::from_json_token("0").unwrap();
        let usage = ReportedUsage {
            cost: Some(&cost),
            prompt_tokens: Some(100),
            completion_tokens: Some(1),
            cached_tokens: Some(100),
            reasoning_tokens: None,
        };
        assert!(matches!(
            ledger.settle_reported(id, &usage),
            Ok(SettleReport::Settled(_))
        ));
        drop(ledger);
        drop(world.open(LedgerEntry::Resume).unwrap());
        world.rewrite(|body| {
            let settlement = &mut body["attempts"][0]["settlement"];
            settlement["completion_tokens"] = json!(0);
            settlement["computed"] = json!("0");
            settlement["consumed"] = json!("0");
        });
        assert_eq!(
            world.open(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    /// A record whose identity is wrong and whose state is fine: only the first half of the check
    /// can see it. A larger reservation breaks no limit and is still not what the formula gives.
    #[test]
    fn a_ledger_whose_reservation_alone_disagrees_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut ledger);
        drop(ledger);
        drop(world.open(LedgerEntry::Resume).unwrap());
        world.rewrite(|body| body["attempts"][0]["reserved"] = json!("720000000000001"));
        assert_eq!(
            world.open(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    /// The other way round: the identity is fine, and only the state's half of the check can see a
    /// reason on a send that was never unknown.
    #[test]
    fn a_ledger_whose_state_alone_disagrees_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        ledger.release(id).unwrap();
        drop(ledger);
        drop(world.open(LedgerEntry::Resume).unwrap());
        world.rewrite(|body| body["attempts"][0]["unknown_reason"] = json!("timeout"));
        assert_eq!(
            world.open(LedgerEntry::Resume).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_settled_send_with_cached_tokens_reopens() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let _permit = ledger.start(id, BODY).unwrap();
        let cost = BilledCostUsd::from_json_token("0").unwrap();
        let usage = ReportedUsage {
            cost: Some(&cost),
            prompt_tokens: Some(100),
            completion_tokens: Some(40),
            cached_tokens: Some(100),
            reasoning_tokens: Some(40),
        };
        assert!(matches!(
            ledger.settle_reported(id, &usage),
            Ok(SettleReport::Settled(_))
        ));
        drop(ledger);
        let reopened = world.open(LedgerEntry::Resume).unwrap();
        assert_eq!(reopened.wallet().physical_used(), 1);
        assert!(reopened.wallet().latch().is_none());
    }

    #[test]
    fn a_report_of_no_prompt_tokens_reaches_the_disk_as_unknown_and_stops_later_processes() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let _permit = ledger.start(id, BODY).unwrap();
        let cost = BilledCostUsd::from_json_token("0").unwrap();
        let usage = ReportedUsage {
            cost: Some(&cost),
            prompt_tokens: Some(0),
            completion_tokens: Some(0),
            ..ReportedUsage::default()
        };
        assert_eq!(
            ledger.settle_reported(id, &usage),
            Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent))
        );
        let disk = on_disk(&world);
        assert_eq!(disk["attempts"][0]["state"], json!("unknown"));
        assert_eq!(disk["latch"]["reason"], json!("unknown_billing"));
        drop(ledger);
        let mut reopened = world.open(LedgerEntry::Fresh).unwrap();
        assert_eq!(
            reopened.success_gate(),
            Err(LedgerFailure::Budget(BudgetRefusal::Latched))
        );
    }

    #[test]
    fn a_ledger_of_another_wallet_put_in_place_is_refused() {
        let world = World::new();
        drop(world.open(LedgerEntry::Fresh).unwrap());
        world.rewrite(|body| body["budget_id"] = json!("wallet-beta"));
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_read_takes_exactly_the_largest_size_and_refuses_one_byte_more() {
        let world = World::new();
        let store = Store::open(WalletDirectory::of(&world.policy)).unwrap();
        let path = world.file();
        let largest = usize::try_from(MAX_LEDGER_BYTES).unwrap();
        fs::write(&path, vec![b' '; largest]).unwrap();
        assert_eq!(store.read_file().unwrap().unwrap().len(), largest);
        fs::write(&path, vec![b' '; largest + 1]).unwrap();
        assert_eq!(store.read_file(), Err(LedgerRefusal::Corrupt));
    }

    #[test]
    fn a_write_past_the_largest_size_is_refused_and_leaves_the_file_alone() {
        assert!(fits_a_ledger(MAX_LEDGER_BYTES));
        assert!(!fits_a_ledger(MAX_LEDGER_BYTES + 1));
        let world = World::new();
        let mut store = Store::open(WalletDirectory::of(&world.policy)).unwrap();
        let name = "p".repeat(usize::try_from(MAX_LEDGER_BYTES).unwrap());
        let wallet = WalletState::new(&world.policy, "workspace".to_string(), Some(name));
        assert_eq!(store.write(&wallet), Err(LedgerRefusal::Unavailable));
        assert!(!world.file().exists());
    }

    #[test]
    fn a_symlink_or_an_oversized_file_in_place_of_the_ledger_is_refused() {
        let world = World::new();
        drop(world.open(LedgerEntry::Fresh).unwrap());
        let keep = world.file().with_extension("kept");
        fs::rename(world.file(), &keep).unwrap();
        std::os::unix::fs::symlink(&keep, world.file()).unwrap();
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Corrupt
        );
        fs::remove_file(world.file()).unwrap();
        fs::write(world.file(), vec![b' '; (8 << 20) + 1]).unwrap();
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Corrupt
        );
    }

    #[test]
    fn a_symlink_for_the_wallet_directory_is_refused() {
        let world = World::new();
        let elsewhere = world.ledgers.parent().unwrap().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        fs::create_dir_all(&world.ledgers).unwrap();
        std::os::unix::fs::symlink(&elsewhere, world.ledgers.join(WALLET)).unwrap();
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Unavailable
        );
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
    }

    #[test]
    fn a_symlink_for_the_lock_is_refused_and_what_it_names_is_not_touched() {
        let world = World::new();
        fs::create_dir_all(world.wallet_dir()).unwrap();
        let target = world.ledgers.join("not-a-lock");
        fs::write(&target, b"target-bytes-kept").unwrap();
        std::os::unix::fs::symlink(&target, world.wallet_dir().join(LOCK_FILE)).unwrap();
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Unavailable
        );
        assert_eq!(fs::read(&target).unwrap(), b"target-bytes-kept");
    }

    #[test]
    fn a_symlink_for_the_temporary_is_refused_and_what_it_names_is_not_touched() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let target = world.ledgers.join("not-a-temporary");
        fs::write(&target, b"target-bytes-kept").unwrap();
        std::os::unix::fs::symlink(&target, world.wallet_dir().join(LEDGER_TEMPORARY)).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(fs::read(&target).unwrap(), b"target-bytes-kept");
    }

    #[test]
    fn only_someone_holding_the_lock_is_a_holder() {
        assert_eq!(lock_refusal(Some(libc::EWOULDBLOCK)), LedgerRefusal::Held);
        assert_eq!(lock_refusal(Some(libc::EAGAIN)), LedgerRefusal::Held);
        assert_eq!(lock_refusal(Some(libc::EBADF)), LedgerRefusal::Unavailable);
        assert_eq!(lock_refusal(Some(libc::ENOLCK)), LedgerRefusal::Unavailable);
        assert_eq!(lock_refusal(None), LedgerRefusal::Unavailable);
    }

    /// A copy of the descriptor, as a child forked a moment ago has one. Only an explicit unlock
    /// frees the lock while the copy lives. The copy here is this test's own.
    #[test]
    fn a_lock_with_a_copy_of_its_descriptor_still_open_is_freed_when_the_holder_ends() {
        let world = World::new();
        let held = world.open(LedgerEntry::Fresh).unwrap();
        // SAFETY: the descriptor is open, and `dup` only uses it.
        let copy = unsafe { libc::dup(held.store.lock.as_raw_fd()) };
        assert!(copy >= 0);
        drop(held);
        let reopened = world.open(LedgerEntry::Resume).map(|_| ());
        // SAFETY: `copy` is this test's own, closed once.
        unsafe { libc::close(copy) };
        assert_eq!(reopened, Ok(()));
    }

    // ---- during a run ----

    #[test]
    fn a_start_past_the_deadline_is_refused_and_writes_nothing_and_exactly_at_it_passes() {
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        world.clock.advance(1_000);
        let permit = ledger.start(id, BODY).unwrap();
        assert_eq!(permit.physical(), id);
        assert_eq!(permit.into_body(), BODY.as_slice());

        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let before = fs::read(world.file()).unwrap();
        world.clock.advance(1_001);
        assert_eq!(
            ledger.start(id, BODY).map(|_| ()),
            Err(LedgerFailure::Budget(BudgetRefusal::WallExceeded))
        );
        assert_eq!(fs::read(world.file()).unwrap(), before);
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("reserved"));
        // Nothing left, so it goes back.
        ledger.release(id).unwrap();
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
    }

    #[test]
    fn a_response_past_the_deadline_is_settled_at_its_cost_and_is_not_a_success() {
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let _permit = ledger.start(id, BODY).unwrap();
        world.clock.advance(1_001);
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(ledger.wallet().spent().unwrap(), money("0.0006"));
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
        assert_eq!(
            ledger.success_gate(),
            Err(LedgerFailure::Budget(BudgetRefusal::Latched))
        );
        drop(ledger);
        let mut reopened = world.open(LedgerEntry::Resume).unwrap();
        assert_eq!(reopened.wallet().spent().unwrap(), money("0.0006"));
        assert_eq!(
            reopened.success_gate(),
            Err(LedgerFailure::Budget(BudgetRefusal::Latched))
        );
    }

    #[test]
    fn a_response_exactly_at_the_deadline_is_a_success() {
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let _permit = ledger.start(id, BODY).unwrap();
        world.clock.advance(1_000);
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(ledger.success_gate(), Ok(()));
    }

    /// The window runs from the first final repair, not from the start of the run.
    #[test]
    fn a_final_repairs_start_past_its_window_is_refused_and_writes_nothing_and_at_it_passes() {
        let world = World::with(|v| v["limits"]["final_repair_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut ledger);
        world.clock.advance(50_000);
        let id = new_final_repair(&mut ledger);
        world.clock.advance(1_000);
        assert_eq!(ledger.start(id, BODY).unwrap().physical(), id);

        let world = World::with(|v| v["limits"]["final_repair_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = new_final_repair(&mut ledger);
        let before = fs::read(world.file()).unwrap();
        world.clock.advance(1_001);
        assert_eq!(
            ledger.start(id, BODY).map(|_| ()),
            Err(LedgerFailure::Budget(
                BudgetRefusal::FinalRepairWallExceeded
            ))
        );
        assert_eq!(fs::read(world.file()).unwrap(), before);
        // Nothing left, so it goes back.
        ledger.release(id).unwrap();
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
    }

    #[test]
    fn a_final_repairs_response_past_its_window_is_settled_at_its_cost_and_is_not_a_success() {
        let world = World::with(|v| v["limits"]["final_repair_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = new_final_repair(&mut ledger);
        let _permit = ledger.start(id, BODY).unwrap();
        world.clock.advance(1_001);
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(ledger.wallet().spent().unwrap(), money("0.0006"));
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
        assert_eq!(
            ledger.wallet().latch().unwrap().reason,
            LatchReason::ResponseAfterFinalRepairWindow
        );
        assert_eq!(
            ledger.success_gate(),
            Err(LedgerFailure::Budget(BudgetRefusal::Latched))
        );
        drop(ledger);
        let mut reopened = world.open(LedgerEntry::Resume).unwrap();
        assert_eq!(reopened.wallet().spent().unwrap(), money("0.0006"));
        assert_eq!(
            reopened.success_gate(),
            Err(LedgerFailure::Budget(BudgetRefusal::Latched))
        );
    }

    #[test]
    fn a_final_repairs_response_exactly_at_its_window_is_a_success() {
        let world = World::with(|v| v["limits"]["final_repair_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = new_final_repair(&mut ledger);
        let _permit = ledger.start(id, BODY).unwrap();
        world.clock.advance(1_000);
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(ledger.success_gate(), Ok(()));
    }

    /// Past the window, a send that is not a final repair starts and is answered like any other.
    #[test]
    fn a_send_that_is_no_final_repair_is_outside_the_window_both_ways() {
        let world = World::with(|v| v["limits"]["final_repair_seconds"] = json!(1));
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let repair = new_final_repair(&mut ledger);
        let _permit = ledger.start(repair, BODY).unwrap();
        ledger.settle(repair, &known(100, 40)).unwrap();
        let id = reserve(&mut ledger).unwrap();
        world.clock.advance(1_001);
        let _permit = ledger.start(id, BODY).unwrap();
        world.clock.advance(1_000);
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(ledger.success_gate(), Ok(()));
    }

    /// The compile-time half: a start on a copy is typed to give out nothing, so this line stops
    /// compiling if it ever gives out anything a POST could use. Only the ledger's start, which
    /// writes, gives the permit.
    #[test]
    fn a_start_on_a_copy_of_the_wallet_gives_out_nothing_and_the_ledgers_gives_the_permit() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        let mut copy = ledger.wallet().clone();
        let nothing: Result<(), BudgetRefusal> = copy.start(&world.policy, id, BODY);
        assert_eq!(nothing, Ok(()));
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("reserved"));

        let permit = ledger.start(id, BODY).unwrap();
        assert_eq!(permit.physical(), id);
        assert_eq!(permit.into_body(), BODY.as_slice());
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("started"));
    }

    #[test]
    fn a_ledger_that_vanishes_during_a_run_refuses_everything_and_stays_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut ledger);
        fs::remove_file(world.file()).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Vanished))
        );
        assert_eq!(
            ledger.success_gate(),
            Err(LedgerFailure::Ledger(LedgerRefusal::Vanished))
        );
        assert!(!world.file().exists(), "nothing wrote it back");
    }

    #[test]
    fn a_ledger_changed_during_a_run_refuses_everything_and_stays_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut ledger);
        let held = fs::read(world.file()).unwrap();
        fs::write(world.file(), b"{}").unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Diverged))
        );
        fs::write(world.file(), held).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Diverged))
        );
    }

    #[test]
    fn an_older_file_put_back_during_a_run_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let older = fs::read(world.file()).unwrap();
        send(&mut ledger);
        fs::write(world.file(), older).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Diverged))
        );
    }

    #[test]
    fn a_success_gate_sees_the_file_change_too() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut ledger);
        assert_eq!(ledger.success_gate(), Ok(()));
        fs::write(world.file(), b"{}").unwrap();
        assert_eq!(
            ledger.success_gate(),
            Err(LedgerFailure::Ledger(LedgerRefusal::Diverged))
        );
    }

    #[test]
    fn a_lock_file_replaced_during_a_run_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let lock = world.ledgers.join(WALLET).join(LOCK_FILE);
        fs::remove_file(&lock).unwrap();
        fs::write(&lock, b"").unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Vanished))
        );
    }

    #[test]
    fn a_wallet_directory_moved_away_during_a_run_is_refused() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let away = world.ledgers.join("moved-away");
        fs::rename(world.ledgers.join(WALLET), &away).unwrap();
        assert_eq!(
            reserve(&mut ledger),
            Err(LedgerFailure::Ledger(LedgerRefusal::Vanished))
        );
    }

    // ---- one holder ----

    #[test]
    fn a_second_open_of_a_held_wallet_is_refused_until_the_holder_ends() {
        let world = World::new();
        let held = world.open(LedgerEntry::Fresh).unwrap();
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            assert_eq!(
                world.open(entry).unwrap_err(),
                LedgerRefusal::Held,
                "{entry:?}"
            );
        }
        drop(held);
        assert!(world.open(LedgerEntry::Resume).is_ok());
    }

    #[test]
    fn a_held_wallet_is_not_changed_by_the_refused_open() {
        let world = World::new();
        let mut held = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut held).unwrap();
        let _permit = held.start(id, BODY).unwrap();
        assert_eq!(
            world.open(LedgerEntry::Fresh).unwrap_err(),
            LedgerRefusal::Held
        );
        // Its started send is not treated as a dead process's.
        assert_eq!(on_disk(&world)["attempts"][0]["state"], json!("started"));
        assert!(held.settle(id, &known(100, 40)).is_ok());
    }

    // ---- where it may live ----

    /// The directory in the policy is what is judged, and its control next to it opens.
    #[test]
    fn a_directory_inside_the_workspace_is_refused_before_anything_is_made() {
        let world = World::new();
        let inside = world.workspace.join("ledgers");
        let policy = world.declaration(WALLET, |v| {
            v["ledger_dir"] = json!(inside.to_str().unwrap());
        });
        assert_eq!(
            world.open_under(policy, None).unwrap_err(),
            LedgerRefusal::DirectoryInsideWorkspace
        );
        assert!(!inside.exists());
        assert!(world.open_under(world.policy.clone(), None).is_ok());
    }

    /// `ledger_dir` is outside the workspace, and the ID lands the wallet on it: what is judged is
    /// the directory the wallet is in. Beside it, with the same parent, is where one may live.
    #[test]
    fn a_wallet_that_lands_on_the_workspace_by_its_id_is_refused_before_anything_is_made() {
        let world = World::new();
        let parent = world.workspace.parent().unwrap().to_path_buf();
        let parent_text = parent.to_str().unwrap();
        let on_it = world.declaration("workspace", |v| v["ledger_dir"] = json!(parent_text));
        assert_eq!(
            world.open_under(on_it, None).unwrap_err(),
            LedgerRefusal::DirectoryInsideWorkspace
        );
        assert_eq!(fs::read_dir(&world.workspace).unwrap().count(), 0);

        let beside = world.declaration("wallet-beside", |v| v["ledger_dir"] = json!(parent_text));
        assert!(world.open_under(beside, None).is_ok());
        assert!(parent.join("wallet-beside").join(LEDGER_FILE).exists());
    }

    // ---- every name on the way to it ----

    fn made(path: PathBuf) -> PathBuf {
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// `name` as a link to `target`, whether or not the target exists.
    fn linked(target: impl AsRef<Path>, name: PathBuf) -> PathBuf {
        fs::create_dir_all(name.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, &name).unwrap();
        name
    }

    /// Every name under a directory, and no link followed, so that nothing can be made unseen.
    fn every_name_under(directory: &Path) -> BTreeSet<PathBuf> {
        let mut found = BTreeSet::new();
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if fs::symlink_metadata(&path).unwrap().is_dir() {
                found.extend(every_name_under(&path));
            }
            found.insert(path);
        }
        found
    }

    fn open_at(world: &World, ledger_dir: &Path) -> Result<BudgetLedger, LedgerRefusal> {
        let text = ledger_dir.to_str().unwrap();
        let policy = world.declaration(WALLET, |v| v["ledger_dir"] = json!(text));
        world.open_under(policy, None)
    }

    /// What a path is judged by has to reach every name a walk of the path uses. Each row has a
    /// world of its own: what it opens is refused, and nothing appears under the world's directory.
    /// All the rows run, so that a failure says which of them are wrong.
    #[test]
    fn a_way_to_the_wallet_is_judged_by_every_name_on_it() {
        use LedgerRefusal::{DirectoryInsideWorkspace as Inside, Unavailable};
        type Way = fn(&Path, &Path) -> PathBuf;
        let rows: Vec<(&str, Way, LedgerRefusal)> = vec![
            (
                "a link in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(safe, workspace.join(".wallets"))
                },
                Inside,
            ),
            (
                "a link in the workspace, by a relative text",
                |base, workspace| {
                    made(base.join("safe"));
                    linked("../safe", workspace.join(".wallets"))
                },
                Inside,
            ),
            (
                "a link deeper in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(safe, workspace.join("deep/.wallets"))
                },
                Inside,
            ),
            (
                "a tail yet to be made, past a link in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(safe, workspace.join(".wallets")).join("more/deep")
                },
                Inside,
            ),
            (
                "a link whose text runs through a link in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    made(safe.join("x"));
                    linked(&safe, workspace.join("m"));
                    linked(workspace.join("m/x"), base.join("l"))
                },
                Inside,
            ),
            (
                "a link whose text ends at a link in the workspace, by its `/.`",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(&safe, workspace.join("m"));
                    linked(workspace.join("m/."), base.join("outer"))
                },
                Inside,
            ),
            (
                "a link whose text climbs out and back into a link in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(&safe, workspace.join("m"));
                    let own_name = base.file_name().unwrap().to_str().unwrap();
                    let text = format!("../{own_name}/workspace/m/.");
                    linked(text, base.join("l"))
                },
                Inside,
            ),
            (
                "a link whose text starts at `.` and ends at a link in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(&safe, workspace.join("m"));
                    linked("./workspace/m", base.join("l"))
                },
                Inside,
            ),
            (
                "a name past a link, and that name a link to a link in the workspace",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(&safe, workspace.join("m"));
                    made(base.join("mid"));
                    linked(workspace.join("m"), base.join("mid/y"));
                    linked("mid", base.join("l1")).join("y")
                },
                Inside,
            ),
            (
                "a link whose text runs through the workspace and climbs out",
                |base, _| {
                    made(base.join("safe"));
                    linked("workspace/../safe", base.join("l"))
                },
                Inside,
            ),
            (
                "a link to the workspace",
                |base, workspace| linked(workspace, base.join("alias")),
                Inside,
            ),
            (
                "a real directory in the workspace",
                |_, workspace| made(workspace.join("real")),
                Inside,
            ),
            (
                "a link to nothing, outside",
                |base, _| linked(base.join("yet"), base.join("d")),
                Unavailable,
            ),
            (
                "a link to nothing, in the workspace",
                |base, workspace| linked(workspace.join("yet"), base.join("d")),
                Unavailable,
            ),
            (
                "a link through a link in the workspace, to nothing",
                |base, workspace| {
                    let safe = made(base.join("safe"));
                    linked(&safe, workspace.join("m"));
                    linked(workspace.join("m/x"), base.join("l"))
                },
                Unavailable,
            ),
            (
                "a link whose text climbs out of a name that does not exist",
                |base, _| {
                    made(base.join("safe"));
                    linked("missing/../safe", base.join("l"))
                },
                Unavailable,
            ),
            (
                "a ring of two links",
                |base, _| {
                    linked(base.join("b"), base.join("a"));
                    linked(base.join("a"), base.join("b"))
                },
                Unavailable,
            ),
        ];
        let mut wrong = Vec::new();
        for (name, way, refusal) in rows {
            let world = World::new();
            let base = world.workspace.parent().unwrap().to_path_buf();
            let ledger_dir = way(&base, &world.workspace);
            let before = every_name_under(&base);
            let opened = open_at(&world, &ledger_dir).map(|_| ());
            let made_anything = every_name_under(&base) != before;
            if opened != Err(refusal) || made_anything {
                wrong.push(format!(
                    "{name}: {opened:?}, made anything: {made_anything}"
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// Links out of a command's reach are what a machine has, and they open, wherever they lead.
    #[test]
    fn a_way_through_links_commands_cannot_change_opens_where_it_leads() {
        type Way = fn(&Path) -> (PathBuf, PathBuf);
        let rows: Vec<(&str, Way)> = vec![
            ("one link", |base| {
                let safe = made(base.join("safe"));
                (linked(&safe, base.join("link")), safe)
            }),
            ("two links", |base| {
                let safe = made(base.join("safe"));
                let one = linked(&safe, base.join("one"));
                (linked(&one, base.join("two")), safe)
            }),
            ("a relative text", |base| {
                let safe = made(base.join("safe"));
                (linked("safe", base.join("link")), safe)
            }),
            ("a text that climbs out and back", |base| {
                let safe = made(base.join("safe"));
                let own_name = base.file_name().unwrap().to_str().unwrap();
                let text = format!("../{own_name}/safe");
                (linked(text, base.join("link")), safe)
            }),
            ("a tail yet to be made", |base| {
                let safe = made(base.join("safe"));
                let link = linked(&safe, base.join("link"));
                (link.join("more/deep"), safe.join("more/deep"))
            }),
        ];
        let mut wrong = Vec::new();
        for (name, way) in rows {
            let world = World::new();
            let base = world.workspace.parent().unwrap().to_path_buf();
            let (ledger_dir, leads_to) = way(&base);
            let opened = open_at(&world, &ledger_dir).map(|_| ());
            let file_there = leads_to.join(WALLET).join(LEDGER_FILE).exists();
            if opened != Ok(()) || !file_there {
                wrong.push(format!("{name}: {opened:?}, file there: {file_there}"));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// The whole of what a command could do with the link, in the order a run would meet it: there
    /// is no first wallet, so moving the link has no second one to give out.
    #[test]
    fn a_link_in_the_workspace_gives_no_wallet_before_or_after_it_is_moved() {
        let world = World::new();
        let base = world.workspace.parent().unwrap().to_path_buf();
        let (safe, elsewhere) = (made(base.join("safe")), made(base.join("elsewhere")));
        let link = linked(&safe, world.workspace.join(".wallets"));
        let text = link.to_str().unwrap();
        let policy = world.declaration(WALLET, |v| {
            v["ledger_dir"] = json!(text);
            v["limits"]["physical_sends"] = json!(1);
        });
        assert_eq!(
            world.open_under(policy.clone(), None).unwrap_err(),
            LedgerRefusal::DirectoryInsideWorkspace
        );
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert_eq!(
            world.open_under(policy, None).unwrap_err(),
            LedgerRefusal::DirectoryInsideWorkspace
        );
        assert_eq!(every_name_under(&safe), BTreeSet::new());
        assert_eq!(every_name_under(&elsewhere), BTreeSet::new());
    }

    /// A policy cannot spell `..` at all, so only the text of a link brings one to a walk.
    #[test]
    fn a_ledger_dir_that_spells_a_climb_is_refused_before_any_walk() {
        let world = World::new();
        let base = world.workspace.parent().unwrap().to_path_buf();
        let spelled = base.join("workspace/../safe");
        let mut value = policy_value();
        value["ledger_dir"] = json!(spelled.to_str().unwrap());
        assert_eq!(
            RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap_err(),
            super::super::policy::PolicyRefusal::InvalidLedgerDir
        );
    }

    const HOLDER_CHILD_ROOT: &str = "COMMANDAGENT_TEST_BUDGET_HOLDER_ROOT";
    const HOLDING_THE_WALLET: &str = "holding-the-wallet-976";

    /// Runs only as the child of the test below: it holds the wallet until the parent says so, by
    /// closing this process's standard input.
    #[test]
    fn a_wallet_held_by_another_process_child() {
        use std::io::{BufRead, Write};
        let Some(root) = std::env::var_os(HOLDER_CHILD_ROOT) else {
            return;
        };
        let root = PathBuf::from(root);
        let _held = open_in_a_cache_home(&root, &root.join("ledgers"), WALLET).unwrap();
        println!("{HOLDING_THE_WALLET}");
        std::io::stdout().flush().unwrap();
        let mut rest = String::new();
        std::io::stdin().lock().read_line(&mut rest).unwrap();
    }

    /// Ends only the child this test started, however the test ends.
    struct OwnChild(std::process::Child);

    impl Drop for OwnChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// The lock across two real processes, in step by what the child prints and when its input
    /// closes, so no length of time decides anything. The holder's end frees the wallet.
    #[test]
    fn a_wallet_held_by_another_process_is_refused_until_that_process_ends() {
        use std::io::BufRead;
        use std::process::Stdio;
        let root = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(root.path()).unwrap();
        let home = made(base.join("home"));
        made(base.join("workspace"));
        let ledger_dir = base.join("ledgers");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_wallet_held_by_another_process_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(HOLDER_CHILD_ROOT, &base)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map(OwnChild)
            .unwrap();
        let input = child.0.stdin.take().unwrap();
        let output = child.0.stdout.take().unwrap();
        let mut lines = std::io::BufReader::new(output).lines();
        assert!(
            lines.any(|line| line.is_ok_and(|line| line.contains(HOLDING_THE_WALLET))),
            "the child ended without holding the wallet"
        );

        assert_eq!(
            open_in_a_cache_home(&base, &ledger_dir, WALLET).unwrap_err(),
            LedgerRefusal::Held
        );

        drop(input);
        assert!(child.0.wait().unwrap().success());
        let opened = open_in_a_cache_home(&base, &ledger_dir, WALLET).unwrap();
        assert_eq!(opened.wallet().physical_used(), 0);
    }

    const CACHE_LINK_CHILD_ROOT: &str = "COMMANDAGENT_TEST_BUDGET_CACHE_LINK_ROOT";

    /// Runs only as the child of the test below, whose environment holds a home of its own.
    #[test]
    fn a_way_through_a_link_in_a_dependency_cache_child() {
        let Some(root) = std::env::var_os(CACHE_LINK_CHILD_ROOT) else {
            return;
        };
        let root = PathBuf::from(root);
        let home = root.join("home");
        for ledger_dir in [
            home.join(".cargo/registry/way"),
            home.join(".npm/way"),
            root.join("outer"),
        ] {
            assert_eq!(
                open_in_a_cache_home(&root, &ledger_dir, WALLET).unwrap_err(),
                LedgerRefusal::DirectoryWritableByCommands,
                "{}",
                ledger_dir.file_name().unwrap().to_string_lossy()
            );
        }
        assert!(open_in_a_cache_home(&root, &home.join(".cargo/beside"), WALLET).is_ok());
    }

    /// A cache is a place commands can write, as the workspace is, so a link there is judged by
    /// the same rule. The check reads `HOME`, so the child has a temporary one.
    #[test]
    fn a_way_through_a_link_in_a_dependency_cache_is_refused_before_anything_is_made() {
        let root = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(root.path()).unwrap();
        let home = base.join("home");
        let registry = made(home.join(".cargo/registry"));
        made(home.join(".npm"));
        made(base.join("workspace"));
        let safe = made(base.join("safe"));
        made(safe.join("x"));
        let beside = made(base.join("beside"));
        linked(&safe, registry.join("way"));
        linked(&safe, registry.join("m"));
        linked(&safe, home.join(".npm/way"));
        linked(registry.join("m/x"), base.join("outer"));
        linked(&beside, home.join(".cargo/beside"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_way_through_a_link_in_a_dependency_cache_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(CACHE_LINK_CHILD_ROOT, &base)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert_eq!(every_name_under(&safe), BTreeSet::from([safe.join("x")]));
        assert!(beside.join(WALLET).join(LEDGER_FILE).exists());
    }

    /// The walk alone, since the system refuses a ring before [`refuse_directory`] gets to it. A
    /// wallet's own name is not there yet, so a chain is followed to its end.
    #[test]
    fn a_walk_follows_as_many_links_as_it_allows_and_refuses_one_more_and_a_ring() {
        let world = World::new();
        let base = world.workspace.parent().unwrap().to_path_buf();
        let walk = |ledger_dir: &Path| {
            let text = ledger_dir.to_str().unwrap();
            let policy = world.declaration(WALLET, |v| v["ledger_dir"] = json!(text));
            refuse_every_name_on_the_way(&WalletDirectory::of(&policy), &world.workspace)
        };
        let mut last = made(base.join("end"));
        for number in 1..=MOST_LINKS_ON_THE_WAY {
            last = linked(&last, base.join(format!("chain-{number}")));
        }
        assert_eq!(walk(&last), Ok(()));
        let past = linked(&last, base.join("chain-past"));
        assert_eq!(walk(&past), Err(LedgerRefusal::Unavailable));

        linked(base.join("ring-b"), base.join("ring-a"));
        let ring = linked(base.join("ring-a"), base.join("ring-b"));
        assert_eq!(walk(&ring), Err(LedgerRefusal::Unavailable));
    }

    /// A path through what the system itself links, as `/tmp` and `/var` on macOS. Nothing is made:
    /// only the check runs, and the names are not there.
    #[test]
    fn a_way_through_the_links_of_the_system_itself_passes() {
        if cfg!(target_os = "macos") {
            assert!(fs::symlink_metadata("/tmp").unwrap().is_symlink());
            assert!(fs::symlink_metadata("/var").unwrap().is_symlink());
        }
        let world = World::new();
        for top in ["/tmp", "/var"] {
            let ledger_dir = Path::new(top).join("commandagent-976-no-such-name");
            let text = ledger_dir.to_str().unwrap();
            let policy = world.declaration(WALLET, |v| v["ledger_dir"] = json!(text));
            let directory = WalletDirectory::of(&policy);
            assert_eq!(
                refuse_directory(&directory, &world.workspace),
                Ok(()),
                "{top}"
            );
            assert!(!ledger_dir.exists(), "{top}");
        }
    }

    const CACHE_CHILD_ROOT: &str = "COMMANDAGENT_TEST_BUDGET_CACHE_ROOT";

    fn open_in_a_cache_home(
        root: &Path,
        ledger_dir: &Path,
        id: &str,
    ) -> Result<BudgetLedger, LedgerRefusal> {
        let mut value = policy_value();
        value["budget_id"] = json!(id);
        value["ledger_dir"] = json!(ledger_dir.to_str().unwrap());
        let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), id).unwrap();
        BudgetLedger::open(LedgerOpen {
            policy: Arc::new(policy),
            workspace: &root.join("workspace"),
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(1),
        })
    }

    /// Runs only as the child of the test below, whose environment holds a home of its own. The
    /// cache is reached under `ledger_dir` and by the ID alone, and beside it is the control.
    #[test]
    fn a_ledger_dir_under_a_dependency_cache_child() {
        let Some(root) = std::env::var_os(CACHE_CHILD_ROOT) else {
            return;
        };
        let root = PathBuf::from(root);
        let home = root.join("home");
        for (ledger_dir, id) in [
            (home.join(".npm/ledgers"), WALLET),
            (home.join(".cargo"), "registry"),
        ] {
            assert_eq!(
                open_in_a_cache_home(&root, &ledger_dir, id).unwrap_err(),
                LedgerRefusal::DirectoryWritableByCommands,
                "{id}"
            );
        }
        assert!(open_in_a_cache_home(&root, &home.join(".cargo"), "wallet-beside").is_ok());
    }

    /// The check reads `HOME`, so a child has a temporary one and nothing here sees the real one.
    /// The test binary's own process environment stays as it was.
    #[test]
    fn a_ledger_dir_under_a_dependency_cache_is_refused_before_anything_is_made() {
        let root = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(root.path())
            .map(|path| (root, path))
            .unwrap();
        let (_held, base) = root;
        let home = base.join("home");
        fs::create_dir_all(home.join(".npm")).unwrap();
        fs::create_dir_all(home.join(".cargo/registry")).unwrap();
        fs::create_dir_all(base.join("workspace")).unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_ledger_dir_under_a_dependency_cache_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(CACHE_CHILD_ROOT, &base)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert_eq!(fs::read_dir(home.join(".npm")).unwrap().count(), 0);
        assert_eq!(
            fs::read_dir(home.join(".cargo/registry")).unwrap().count(),
            0
        );
        assert!(home.join(".cargo/wallet-beside").join(LEDGER_FILE).exists());
    }

    #[test]
    fn a_workspace_that_cannot_be_resolved_is_refused() {
        let world = World::new();
        let missing = world.workspace.join("no-such-directory");
        assert_eq!(
            world
                .open_as(LedgerEntry::Fresh, &missing, None)
                .unwrap_err(),
            LedgerRefusal::Unavailable
        );
    }

    // ---- the clock across processes ----

    #[test]
    fn a_clock_set_back_between_processes_gives_no_time_back() {
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(10));
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut first);
        world.clock.advance(10_000);
        send(&mut first);
        drop(first);
        // Elapsed is exactly at the limit now. Going back an hour does not make room.
        world.clock.new_process();
        world.clock.set_wall(1);
        let mut second = world.open(LedgerEntry::Resume).unwrap();
        send(&mut second);
        world.clock.advance_monotonic_only(1);
        assert_eq!(
            reserve(&mut second),
            Err(LedgerFailure::Budget(BudgetRefusal::WallExceeded))
        );
    }

    #[test]
    fn the_time_between_processes_counts_and_the_start_stays() {
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(10));
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        send(&mut first);
        let started = first.wallet().clock.started_wall_ms;
        drop(first);
        world.clock.new_process();
        world.clock.set_wall(7_000 + 10_001);
        let mut second = world.open(LedgerEntry::Fresh).unwrap();
        assert_eq!(second.wallet().clock.started_wall_ms, started);
        assert_eq!(
            reserve(&mut second),
            Err(LedgerFailure::Budget(BudgetRefusal::WallExceeded))
        );
    }

    #[test]
    fn the_high_water_mark_reaches_the_disk() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        world.clock.advance(1_234);
        let _permit = ledger.start(id, BODY).unwrap();
        assert_eq!(
            on_disk(&world)["clock"]["high_water_elapsed_ms"],
            json!(1_234)
        );
    }

    // ---- no table anywhere ----

    /// Every Rust file at or below a directory.
    fn rust_files_under(directory: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                found.extend(rust_files_under(&path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                found.push(path);
            }
        }
        found
    }

    /// What a file holds of process-wide state, read up to its line of `mod tests`. Any other
    /// `#[cfg(test)]` is an item among the rest and ends nothing. A `'static` lifetime holds nothing,
    /// so it is the one thing taken out of what is read.
    fn process_wide_words(source: &str) -> Vec<&'static str> {
        let production = source
            .lines()
            .take_while(|line| line.trim() != "mod tests {")
            .collect::<Vec<_>>()
            .join("\n")
            .replace("'static", "");
        [
            "static ",
            "OnceLock",
            "LazyLock",
            "thread_local",
            "lazy_static",
        ]
        .into_iter()
        .filter(|word| production.contains(word))
        .collect()
    }

    #[test]
    fn the_search_for_process_wide_state_reads_every_level_and_stops_only_at_mod_tests() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("deep/er")).unwrap();
        let deep = root.path().join("deep/er/held-976.rs");
        fs::write(&deep, "static HELD_976: u8 = 0;\n").unwrap();
        fs::write(root.path().join("not-rust-976.txt"), "static ").unwrap();
        assert_eq!(rust_files_under(root.path()), [deep]);

        let after_an_attribute = "#[cfg(test)]\nuse std::fmt;\nstatic HELD_976: u8 = 0;\n";
        assert_eq!(process_wide_words(after_an_attribute), ["static "]);
        let in_its_tests =
            "pub(crate) fn f() {}\n#[cfg(test)]\nmod tests {\n    static T: u8 = 0;\n";
        assert!(process_wide_words(in_its_tests).is_empty());
    }

    /// A `'static` lifetime holds nothing. A word `static` in its place in the same source does.
    #[test]
    fn a_static_lifetime_is_not_process_wide_and_a_static_item_beside_it_is() {
        let lifetime = "pub(crate) const fn label(self) -> &'static str { \"a\" }\n";
        assert!(process_wide_words(lifetime).is_empty());
        let item = "pub(crate) static HELD_976: u8 = 0;\n";
        assert_eq!(process_wide_words(item), ["static "]);
        let both = "const A: &'static str = \"a\";\nstatic HELD_976: u8 = 0;\n";
        assert_eq!(process_wide_words(both), ["static "]);
        let one_line = "const A: &'static str = \"a\"; static HELD_976: u8 = 0;\n";
        assert_eq!(process_wide_words(one_line), ["static "]);
    }

    #[test]
    fn nothing_in_this_module_is_process_wide() {
        // Whatever files the module has by now, so a new one cannot be left out.
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/structured_budget");
        let files = rust_files_under(&directory);
        for path in &files {
            let words = process_wide_words(&fs::read_to_string(path).unwrap());
            assert!(words.is_empty(), "{words:?} in {}", path.display());
        }
        for known in ["mod.rs", "ledger.rs", "reservation.rs", "clock.rs"] {
            assert!(
                files.iter().any(|path| path.ends_with(known)),
                "{known}: {files:?}"
            );
        }
    }

    /// What a reservation or a start that went through has to be inside: the two kinds of refusal
    /// come from both moves, so a walk that only sees the refusals does not see one of them go.
    fn inside_its_time(wallet: &WalletState, policy: &RunBudgetPolicy, physical: PhysicalId) {
        let seen = wallet.clock.high_water_elapsed_ms;
        assert!(seen <= policy.limits.wall_ms, "{physical:?} at {seen}");
        if wallet.attempt(physical).unwrap().final_repair.is_some() {
            let began = wallet.clock.final_repair_started_ms.unwrap();
            let window = seen.saturating_sub(began);
            assert!(
                window <= policy.limits.final_repair_ms,
                "{physical:?} {window} into the window"
            );
        }
    }

    /// Reports stay inside their reservations, so a walk gets to the limits instead of to a latch.
    /// Each seed narrows one limit, through the real file, with processes ending, and has to find
    /// every limit saying no. A line at the start or at the settling moves the clock, which is what
    /// gets to the two checks of time after a reservation.
    #[test]
    fn a_walk_through_the_file_reaches_every_limit_and_stays_inside_them() {
        let mut refusals = BTreeMap::<String, u64>::new();
        let mut reasons = BTreeMap::<String, u64>::new();
        let mut total_posts = 0_u64;
        for seed in 1..=WALK_SEEDS {
            let mut rng = Rng::seeded(seed);
            let (sends, cost, tokens, wall, attempts, window) = match seed % 5 {
                0 => (3, "1000", 1_000_000, 100_000, 1, 100_000),
                1 => (1000, "0.0015", 1_000_000, 100_000, 1, 100_000),
                2 => (1000, "1000", 450, 100_000, 1, 100_000),
                3 => (1000, "1000", 1_000_000, 8, 1, 100_000),
                _ => (1000, "1000", 1_000_000, 100_000, 1000, 1),
            };
            let world = World::with(|v| {
                let limits = &mut v["limits"];
                limits["physical_sends"] = json!(sends);
                limits["cost_usd"] = json!(cost);
                limits["tokens"] = json!(tokens);
                limits["wall_seconds"] = json!(wall);
                limits["logical_global"] = json!(4);
                limits["logical_per_stage"] = json!(1);
                limits["final_repair_attempts"] = json!(attempts);
                limits["final_repair_seconds"] = json!(window);
            });
            let mut ledger = world.open_naming(None).unwrap();
            let mut posts = 0_u64;
            let mut floor = Money::ZERO;
            let mut last: Option<ResponseIdentity> = None;
            let mut repair: Option<FinalRepairId> = None;
            for _ in 0..WALK_STEPS {
                let held = ledger
                    .wallet()
                    .attempts
                    .iter()
                    .find(|attempt| {
                        matches!(
                            attempt.state,
                            AttemptState::Reserved | AttemptState::Started
                        )
                    })
                    .map(|attempt| (attempt.physical, attempt.state));
                let moves_the_clock = match held {
                    Some((_, AttemptState::Started)) => rng.next().is_multiple_of(3),
                    Some(_) => rng.next().is_multiple_of(8),
                    None => false,
                };
                if moves_the_clock {
                    world.clock.advance(3_000);
                }
                let result: Result<(), LedgerFailure> = match (rng.next() % 8, held) {
                    (_, Some((id, AttemptState::Reserved))) => match ledger.start(id, BODY) {
                        Ok(_) => {
                            posts += 1;
                            inside_its_time(ledger.wallet(), &world.policy, id);
                            Ok(())
                        }
                        Err(refusal) => {
                            ledger.release(id).unwrap();
                            Err(refusal)
                        }
                    },
                    (_, Some((id, _))) => {
                        let amount = BilledCostUsd::from_json_token("0.0001").unwrap();
                        let usage = ReportedUsage {
                            cost: Some(&amount),
                            prompt_tokens: Some(60),
                            completion_tokens: Some(20),
                            ..ReportedUsage::default()
                        };
                        ledger.settle_reported(id, &usage).map(|report| {
                            if let SettleReport::Settled(outcome) = report {
                                last = Some(outcome.response);
                            }
                        })
                    }
                    (0 | 1, None) => reserve(&mut ledger)
                        .map(|id| inside_its_time(ledger.wallet(), &world.policy, id)),
                    (2, None) => {
                        let claim = match repair {
                            Some(id) if rng.next().is_multiple_of(2) => {
                                FinalRepairClaim::Continue(id)
                            }
                            _ => FinalRepairClaim::New,
                        };
                        reserve_final_repair(&mut ledger, claim).map(|reservation| {
                            repair = reservation.final_repair;
                            inside_its_time(ledger.wallet(), &world.policy, reservation.physical);
                        })
                    }
                    (3, None) => {
                        let kind = if rng.next().is_multiple_of(2) {
                            LogicalKind::General
                        } else {
                            stage_kind("a")
                        };
                        let source = if rng.next().is_multiple_of(2) {
                            last.map(|response| response.call((rng.next() % 3) as u32))
                        } else {
                            None
                        };
                        ledger.record_submission(kind, source).map(|_| ())
                    }
                    (4, None) => {
                        world.clock.advance(rng.next() % 2_000);
                        Ok(())
                    }
                    (5, None) if rng.next().is_multiple_of(3) => {
                        drop(ledger);
                        world.clock.new_process();
                        ledger = world.open_naming(None).unwrap();
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(refusal) = result {
                    *refusals.entry(format!("{refusal:?}")).or_default() += 1;
                }

                let wallet = ledger.wallet();
                assert!(posts <= sends, "seed {seed}");
                let on_record = wallet
                    .attempts
                    .iter()
                    .filter(|attempt| {
                        !matches!(
                            attempt.state,
                            AttemptState::Reserved | AttemptState::Released
                        )
                    })
                    .count() as u64;
                assert_eq!(on_record, posts, "seed {seed}: every POST is on the record");
                assert!(wallet.is_consistent(&world.policy), "seed {seed}");
                assert!(wallet.logical_global_used() <= 4, "seed {seed}");
                assert!(wallet.logical_stage_used("a") <= 1, "seed {seed}");
                assert!(wallet.final_repair_used() <= attempts, "seed {seed}");
                let spent = wallet.spent().unwrap();
                assert!(spent >= floor, "seed {seed}");
                floor = spent;
                if wallet.latch().is_none() {
                    let money = spent.units() + wallet.held().unwrap().units();
                    assert!(money <= world.policy.limits.cost.units(), "seed {seed}");
                    let used = wallet.spent_tokens() + wallet.held_tokens();
                    assert!(used <= u128::from(tokens as u64), "seed {seed}");
                }
            }
            total_posts += posts;
            let reason = ledger
                .wallet()
                .latch()
                .map_or("none".to_string(), |latch| format!("{:?}", latch.reason));
            *reasons.entry(reason).or_default() += 1;
        }
        eprintln!("walk posts={total_posts} refusals={refusals:?} reasons={reasons:?}");
        for reached in [
            "PhysicalExceeded",
            "TokensExceeded",
            "CostExceeded",
            "WallExceeded",
            "LogicalGlobalExceeded",
            "LogicalStageExceeded",
            "FinalRepairAttemptsExceeded",
            "FinalRepairWallExceeded",
        ] {
            let key = format!("Budget({reached})");
            assert!(refusals.contains_key(&key), "{reached}: {refusals:?}");
        }
        for reached in ["ResponseAfterDeadline", "ResponseAfterFinalRepairWindow"] {
            assert!(reasons.contains_key(reached), "{reached}: {reasons:?}");
        }
    }

    const WALK_SEEDS: u64 = 30;
    const WALK_STEPS: usize = 60;

    #[test]
    fn submissions_count_across_processes() {
        let world = World::with(|v| v["limits"]["logical_global"] = json!(1));
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        first.record_submission(LogicalKind::General, None).unwrap();
        drop(first);
        let mut second = world.open(LedgerEntry::Resume).unwrap();
        assert_eq!(
            second
                .record_submission(LogicalKind::General, None)
                .map(|_| ()),
            Err(LedgerFailure::Budget(BudgetRefusal::LogicalGlobalExceeded))
        );
        // A send opens none, so it goes on.
        assert!(reserve(&mut second).is_ok());
    }

    #[test]
    fn a_wallet_started_by_a_submission_reopens() {
        let world = World::new();
        let mut first = world.open(LedgerEntry::Fresh).unwrap();
        first.record_submission(LogicalKind::General, None).unwrap();
        drop(first);
        let second = world.open(LedgerEntry::Resume).unwrap();
        assert_eq!(second.wallet().clock.started_wall_ms, Some(7_000));
    }
}
