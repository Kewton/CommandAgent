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
//! Known limits: someone with the same user ID, outside the sandbox and before a run, can delete
//! the file or put an older one back. A fresh start then cannot tell; a resume is refused when the
//! file is gone. The same goes for an operator who rewrites `ledger_dir` in a policy: there is no
//! ledger at the new directory, so the same ID is a full wallet there. The digest of the file finds
//! damage and has no key, so it does not stop a change made on purpose.
//!
//! A profile the operator named is pinned, in its canonical name, by the first run that names one.
//! One that was only inferred is not: it can change as a run writes files, and it names no wallet.
//! So a run with no explicit profile continues the same wallet, whatever it infers.
//!
//! The file holds counts, digests, amounts, fixed words, the profile's name, and G4's stage and
//! target names and run nonce: no prompt, response body, key, or header, and the workspace only as
//! a digest.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::clock::{BudgetClock, RunClock};
use super::identity::{AttemptBinding, LogicalId, LogicalKind, PhysicalId, SubmissionSource};
use super::policy::RunBudgetPolicy;
use super::reservation::{BudgetRefusal, Reservation, ReserveRequest, StartedPermit, WalletState};
use super::settlement::{KnownBilling, ReportedUsage, SettleOutcome, SettleReport, UnknownReason};
use crate::planner::profile::canonical_profile_name;
use crate::planner::profile_descriptor::descriptor_for_name;
use crate::planner::structured_stages::ledger::{
    LedgerRefusal as StageLedgerRefusal, refuse_state_dir_inside_workspace,
};
use crate::planner::structured_stages::receipt::sha256_hex;

const LEDGER_FILE: &str = "ledger.json";
const LEDGER_TEMPORARY: &str = "ledger.json.tmp";
const LOCK_FILE: &str = "lock";
const MAX_LEDGER_BYTES: u64 = 8 << 20;

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

#[derive(Debug)]
struct Store {
    directory: PathBuf,
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

/// The one name of a profile, by the tables the product already has.
fn canonical_explicit_profile(name: &str) -> String {
    descriptor_for_name(name).map_or_else(
        || canonical_profile_name(name),
        |descriptor| descriptor.canonical.to_string(),
    )
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
            .open(self.directory.join(LEDGER_FILE));
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

    /// The lock at the path is the one held, and the file is what this handle last wrote. A lock
    /// file replaced under a holder would let the next process hold another one.
    fn verify_unchanged(&self) -> Result<(), LedgerRefusal> {
        let held = self.lock.metadata().map_err(unavailable)?;
        let same_lock = fs::symlink_metadata(self.directory.join(LOCK_FILE))
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

/// The checks G4 made for its own ledger: a directory commands of the run can write proves nothing.
fn refuse_directory(directory: &Path, workspace: &Path) -> Result<(), LedgerRefusal> {
    refuse_state_dir_inside_workspace(directory, workspace).map_err(|refusal| match refusal {
        StageLedgerRefusal::StateDirInsideWorkspace => LedgerRefusal::DirectoryInsideWorkspace,
        StageLedgerRefusal::StateDirWritableByCommands => {
            LedgerRefusal::DirectoryWritableByCommands
        }
        _ => LedgerRefusal::Unavailable,
    })
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
        refuse_directory(&policy.ledger_dir, request.workspace)?;
        let workspace = fs::canonicalize(request.workspace).map_err(unavailable)?;
        let workspace_sha256 = sha256_hex(workspace.as_os_str().as_bytes());
        let explicit_profile = request.explicit_profile.map(canonical_explicit_profile);
        let mut store = Store::open(policy.ledger_dir.join(&policy.budget_id))?;
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
        self.apply(|wallet, policy, _| wallet.start(policy, physical, body))
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
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::super::identity::{LogicalKind, PhysicalId};
    use super::super::policy::RunBudgetPolicy;
    use super::super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money, WalletState};
    use super::super::settlement::{KnownBilling, ReportedUsage, SettleReport, UnknownReason};
    use super::super::test_support::{BODY, FakeClock, WALLET, policy_value, request, route};
    use super::{
        BudgetLedger, LEDGER_FILE, LEDGER_TEMPORARY, LOCK_FILE, LedgerEntry, LedgerFailure,
        LedgerOpen, LedgerRefusal, MAX_LEDGER_BYTES, Store, fits_a_ledger, lock_refusal,
    };
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
        let wire = route();
        let body = request(&wire);
        ledger
            .reserve(&body)
            .map(|reservation| reservation.physical)
    }

    fn send(ledger: &mut BudgetLedger) -> PhysicalId {
        let id = reserve(ledger).unwrap();
        ledger.start(id, BODY).unwrap();
        ledger.settle(id, &known(100, 40)).unwrap();
        id
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
        assert_eq!(second.store.directory, world.policy.ledger_dir.join(WALLET));
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
        let names = ["python-cli", "py-cli", "python", "py", "cli", " PY-CLI "];
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
        first.start(id, BODY).unwrap();
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
        first.start(id, BODY).unwrap();
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
        ledger.start(id, BODY).unwrap();
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
        let wire = route();
        let canary = format!("{:x<100}", "canary-prompt-body-text");
        let mut body = request(&wire);
        body.body = canary.as_bytes();
        let reservation = ledger.reserve(&body).unwrap();
        ledger
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

    #[test]
    fn a_settled_send_with_cached_tokens_reopens() {
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let id = reserve(&mut ledger).unwrap();
        ledger.start(id, BODY).unwrap();
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
        ledger.start(id, BODY).unwrap();
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
        let store = Store::open(world.wallet_dir()).unwrap();
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
        let mut store = Store::open(world.wallet_dir()).unwrap();
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
        assert_eq!(permit.body(), BODY.as_slice());

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
        ledger.start(id, BODY).unwrap();
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
        ledger.start(id, BODY).unwrap();
        world.clock.advance(1_000);
        ledger.settle(id, &known(100, 40)).unwrap();
        assert_eq!(ledger.success_gate(), Ok(()));
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
        held.start(id, BODY).unwrap();
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

    const CACHE_CHILD_ROOT: &str = "COMMANDAGENT_TEST_BUDGET_CACHE_ROOT";

    /// Runs only as the child of the test below, whose environment holds a home of its own.
    #[test]
    fn a_ledger_dir_under_a_dependency_cache_child() {
        let Some(root) = std::env::var_os(CACHE_CHILD_ROOT) else {
            return;
        };
        let root = PathBuf::from(root);
        let inside = root.join("home/.npm/ledgers");
        let mut value = policy_value();
        value["ledger_dir"] = json!(inside.to_str().unwrap());
        let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
        let refused = BudgetLedger::open(LedgerOpen {
            policy: Arc::new(policy),
            workspace: &root.join("workspace"),
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(1),
        });
        assert_eq!(
            refused.unwrap_err(),
            LedgerRefusal::DirectoryWritableByCommands
        );
        assert!(!inside.exists());
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
        ledger.start(id, BODY).unwrap();
        assert_eq!(
            on_disk(&world)["clock"]["high_water_elapsed_ms"],
            json!(1_234)
        );
    }

    // ---- no table anywhere ----

    #[test]
    fn nothing_in_this_module_is_process_wide() {
        // Whatever files the module has by now, so a new one cannot be left out.
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/structured_budget");
        let mut read = Vec::new();
        for entry in fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(&path).unwrap();
                let production = source.split("#[cfg(test)]").next().unwrap().to_string();
                read.push(path.file_name().unwrap().to_owned());
                for word in [
                    "static ",
                    "OnceLock",
                    "LazyLock",
                    "thread_local",
                    "lazy_static",
                ] {
                    assert!(!production.contains(word), "{word} in {}", path.display());
                }
            }
        }
        for known in ["mod.rs", "ledger.rs", "reservation.rs", "clock.rs"] {
            assert!(read.iter().any(|name| name == known), "{known}: {read:?}");
        }
    }

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
