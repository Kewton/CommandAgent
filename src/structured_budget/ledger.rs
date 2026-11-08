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
//! change, links and the text of links included. A link out of a command's reach is allowed. A
//! command's reach is what the sandbox applied on the run lets it write, read from that sandbox's
//! own policy: the workspace, the run's temporary, and the caches. With no such policy, no wallet
//! opens. The walk that judges the way holds each directory it judged, so what is made is made
//! there, one name at a time and never over a name that appeared since, and the wallet's directory
//! is held for as long as the wallet: no file of it is reached by its path again. What is made has
//! to be, when it is opened, the directory seen at its name just after the make, empty, and this
//! user's. A link at the wallet's own name is refused
//! before it is followed. Before anything is made, before the lock is opened and once it is held,
//! before every change, and after every rename, every directory held from the root down has to be
//! at the name it was entered by, with no link there, and the path has to name the directory held.
//! The lock, the ledger, and the temporary each have to have one name, their own.
//!
//! Known limits of the directory held:
//! - A place is judged by its name, the path as the walk spells it, and not by where a directory
//!   held is now. So it is the chain of names, checked again at the points above, that keeps a
//!   directory moved since from being used; between two checks nothing is seen.
//! - Someone with the same user ID, outside the sandbox, can still change things between a check
//!   and the call after it. What can be left behind before a refusal: one empty directory made
//!   where a directory of the way was moved to, or the lock file in it, and one version of the
//!   ledger renamed into place in the directory held, if that directory is moved between the check
//!   before a change and its rename. The check after the rename refuses that change and every one
//!   after it, so it is that one version, and the run stops.
//! - A directory made is seen at its name right after the make, and the one opened is compared
//!   with it. A swap between the make and that look is not seen; a directory put there then still
//!   has to be empty and this user's, and the chain is checked again after it.
//! - A directory made has to be this user's by the owner the file system reports. One that
//!   reports another owner for what this user makes (NFS with a squash, FAT or exFAT mounted with
//!   a `uid=`, some FUSE or SMB mounts) refuses a new wallet there; one already made is not made
//!   again, so it opens as before.
//! - One name of a file is seen at each check, and not between two. A second name made, used,
//!   and removed between two checks is not seen. That a command of a run cannot make one rests on
//!   the sandbox refusing a hard link into its reach: measured as refused on macOS, and only read
//!   for Linux Landlock, where a link across directories needs its `REFER` right.
//! - A writer with the same user ID outside the sandbox can write the file itself in its place;
//!   that is the limit below, and not one of the directory.
//! - A bind mount gives the same directory another name with the same device and inode, so it is
//!   not seen; neither is any other mount over a name of the way.
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

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::clock::{BudgetClock, RunClock};
use super::identity::{AttemptBinding, LogicalId, LogicalKind, PhysicalId, SubmissionSource};
use super::policy::RunBudgetPolicy;
use super::reservation::{BudgetRefusal, Reservation, ReserveRequest, WalletState};
use super::settlement::{KnownBilling, ReportedUsage, SettleOutcome, SettleReport, UnknownReason};
use super::storage::{self, Found, Identity};
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
    /// No sandbox applied on this workspace says where commands of the run can write, so no place
    /// can be judged out of their reach.
    SandboxPolicyUnknown,
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
    /// Where commands of this run can write, from the sandbox applied on it. Without it the wallet
    /// does not open.
    pub(crate) command_writes: Option<CommandWrites>,
}

/// One wallet, its lock held for as long as this value lives.
#[derive(Debug)]
pub(crate) struct BudgetLedger {
    policy: Arc<RunBudgetPolicy>,
    wallet: WalletState,
    store: Store,
    clock: RunClock,
    failed: Option<LedgerRefusal>,
    /// Whether this open made the wallet, there being no record of it before. In memory only: the
    /// file says nothing of it, and means what it meant.
    made_here: bool,
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

mod command_writes {
    use crate::tools::os_sandbox::{self, SandboxPolicy};

    /// Where commands of a run can write, as the sandbox applied on it holds them. Its field keeps
    /// every other policy from being one, so a place is judged by what is in force.
    #[derive(Debug, Clone)]
    pub(crate) struct CommandWrites(SandboxPolicy);

    impl CommandWrites {
        /// The policy of the sandbox applied on this run, or nothing when none is applied.
        pub(crate) fn applied() -> Option<Self> {
            os_sandbox::applied_policy().map(Self)
        }

        /// A policy a test holds, with no sandbox of the process read.
        #[cfg(test)]
        pub(crate) fn of(policy: SandboxPolicy) -> Self {
            Self(policy)
        }

        pub(crate) fn policy(&self) -> &SandboxPolicy {
            &self.0
        }
    }
}

pub(crate) use command_writes::CommandWrites;

/// Every place a command of the run can write besides the workspace, each as it really leads.
#[derive(Debug)]
struct WriteBounds {
    roots: Vec<PathBuf>,
}

impl WriteBounds {
    /// What is not known is not a bound: no policy, a policy of another workspace, or a temporary
    /// that is not there each refuse the wallet before anything is made.
    fn of(writes: Option<&CommandWrites>, workspace: &Path) -> Result<Self, LedgerRefusal> {
        let workspace = fs::canonicalize(workspace).map_err(unavailable)?;
        let policy = writes.ok_or(LedgerRefusal::SandboxPolicyUnknown)?.policy();
        let unknown = |_| LedgerRefusal::SandboxPolicyUnknown;
        if fs::canonicalize(&policy.workspace_root).map_err(unknown)? != workspace {
            return Err(LedgerRefusal::SandboxPolicyUnknown);
        }
        let mut roots = vec![fs::canonicalize(&policy.run_tmp_dir).map_err(unknown)?];
        let caches = &policy.caches;
        let caches = [
            &caches.cargo_registry,
            &caches.cargo_git,
            &caches.cargo_package_cache,
            &caches.npm,
            &caches.pip,
            &caches.playwright,
        ];
        roots.extend(
            caches
                .into_iter()
                .flatten()
                .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone())),
        );
        Ok(Self { roots })
    }
}

/// A real directory, one with no link on its way, inside a place commands of the run can write.
fn refuse_command_writes(real: &Path, bounds: &WriteBounds) -> Result<(), LedgerRefusal> {
    if bounds.roots.iter().any(|root| real.starts_with(root)) {
        return Err(LedgerRefusal::DirectoryWritableByCommands);
    }
    Ok(())
}

#[derive(Debug)]
struct Store {
    directory: WalletDirectory,
    /// Every real directory from the root to the wallet's own, as the walk entered or made it.
    chain: Chain,
    /// The wallet's own directory, reached from the walk that judged the way to it. Every file is
    /// opened, renamed, and fsynced through it, and never by the path again.
    held: File,
    lock: File,
    expected_sha256: Option<String>,
}

/// The real directories of a way from the root down, each held with the name it was entered by in
/// the one before it. The root's name is empty. Links are not in it: it is where they led.
type Chain = Vec<(File, OsString)>;

/// Where a walk that judged every name ends: the real directories it entered, held, the last of
/// them where it stopped, and the plain names below that are yet to be made.
#[derive(Debug)]
struct Way {
    chain: Chain,
    to_make: Vec<OsString>,
}

/// Whether every directory of the chain is still where it was entered: at its name in the one
/// before it, with no link there. A name is judged as where it leads, so a directory moved, even
/// with a link left to where it went, is no longer at the place that was judged.
fn chain_holds(chain: &[(File, OsString)]) -> bool {
    chain.windows(2).all(|pair| {
        let ((parent, _), (child, name)) = (&pair[0], &pair[1]);
        Identity::of(child).is_ok_and(|child| {
            storage::look_at(parent, name).is_ok_and(|found| found == Some(Found::Entry(child)))
        })
    })
}

/// A file of the wallet has one name, its own. Another name, in a directory a command can write,
/// would let the command change it there.
fn has_one_name(file: &File) -> Result<bool, LedgerRefusal> {
    Ok(file.metadata().map_err(unavailable)?.nlink() == 1)
}

/// What this module made is this user's and empty. Something else in its place since the make,
/// whatever it holds, was not judged.
fn refuse_what_was_not_made_here(made: &File) -> Result<(), LedgerRefusal> {
    let owner = made.metadata().map_err(unavailable)?.uid();
    if owner != own_user() || !storage::is_empty(made).map_err(unavailable)? {
        return Err(LedgerRefusal::Unavailable);
    }
    Ok(())
}

/// The user this process makes directories as.
fn effective_user() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

#[cfg(test)]
fn own_user() -> u32 {
    tests::pretended_user().unwrap_or_else(effective_user)
}

#[cfg(not(test))]
fn own_user() -> u32 {
    effective_user()
}

/// The lock has just been taken, and the place is about to be compared again: a point where a
/// test changes the world. It is apart from [`Seam`] so the order the seams are met in stays as a
/// test pins it.
#[cfg(test)]
fn at_lock_taken(at: &Path) {
    tests::fire_lock_taken(at);
}

#[cfg(not(test))]
fn at_lock_taken(_: &Path) {}

/// A point between two calls on the way to the disk, where a test changes the world. Outside a
/// test nothing happens at one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seam {
    /// A name of the way has been looked at, and is about to be opened.
    BeforeEntering,
    /// The way is judged, and nothing is made yet.
    AfterWalk,
    /// A directory has been made, and is about to be opened.
    AfterMake,
    /// The wallet's directory is held, and its lock is about to be opened.
    BeforeLock,
    /// A new version is in the temporary, about to take the ledger's name.
    BeforeRename,
}

#[cfg(test)]
fn at_seam(seam: Seam, at: &Path) {
    tests::fire_seam(seam, at);
}

#[cfg(not(test))]
fn at_seam(_: Seam, _: &Path) {}

/// Whether the wallet's path, read now, names the directory held, with no link at its own name.
/// The path is only compared: nothing is opened by it.
fn leads_to_held(directory: &WalletDirectory, held: &File) -> bool {
    let Ok(found) = fs::symlink_metadata(directory.path()) else {
        return false;
    };
    Identity::of(held).is_ok_and(|held| Identity::of_metadata(&found) == held)
}

/// The plain names a walk left to be made. Anything else could lead anywhere once made.
fn names_to_make(first: &OsStr, rest: &Path) -> Result<Vec<OsString>, LedgerRefusal> {
    let mut names = vec![first.to_os_string()];
    for name in rest.components() {
        match name {
            Component::Normal(name) => names.push(name.to_os_string()),
            _ => return Err(LedgerRefusal::Unavailable),
        }
    }
    Ok(names)
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
    /// A directory a test reaches with no place judged, to measure what is inside it.
    #[cfg(test)]
    fn open(directory: WalletDirectory) -> Result<Self, LedgerRefusal> {
        let way = walk_the_way(&directory, None)?;
        Self::open_on(directory, way)
    }

    /// Makes what is yet to be made from the directory the walk reached, one name at a time, and
    /// holds the last. A name there by now was not judged, so it is refused, whatever it is, and so
    /// is a directory opened at the name of one made here that is not the one seen there just after
    /// the make, or not empty and this user's. Nothing
    /// is made, and no lock is opened, in a directory of the way that is no longer where it was
    /// judged.
    fn open_on(directory: WalletDirectory, way: Way) -> Result<Self, LedgerRefusal> {
        let mut chain = way.chain;
        if !chain_holds(&chain) {
            return Err(LedgerRefusal::Unavailable);
        }
        for name in &way.to_make {
            let (reached, _) = chain.last().ok_or(LedgerRefusal::Unavailable)?;
            storage::make_directory_at(reached, name).map_err(unavailable)?;
            // What the make made, seen before anything else can be put at its name.
            let Some(Found::Entry(made_as)) =
                storage::look_at(reached, name).map_err(unavailable)?
            else {
                return Err(LedgerRefusal::Unavailable);
            };
            at_seam(Seam::AfterMake, directory.path());
            let made = storage::open_made(reached, name).map_err(unavailable)?;
            if Identity::of(&made).map_err(unavailable)? != made_as {
                return Err(LedgerRefusal::Unavailable);
            }
            refuse_what_was_not_made_here(&made)?;
            chain.push((made, name.clone()));
            if !chain_holds(&chain) {
                return Err(LedgerRefusal::Unavailable);
            }
        }
        let (reached, _) = chain.last().ok_or(LedgerRefusal::Unavailable)?;
        let held = storage::open_itself(reached).map_err(unavailable)?;
        if !leads_to_held(&directory, &held) {
            return Err(LedgerRefusal::Unavailable);
        }
        at_seam(Seam::BeforeLock, directory.path());
        if !chain_holds(&chain) {
            return Err(LedgerRefusal::Unavailable);
        }
        let lock = storage::open_file_at(
            &held,
            LOCK_FILE,
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
        .map_err(unavailable)?;
        if !lock.metadata().map_err(unavailable)?.is_file() || !has_one_name(&lock)? {
            return Err(LedgerRefusal::Unavailable);
        }
        // SAFETY: `lock` is an open descriptor this function owns, and flock only uses it.
        // LOCK_NB makes it return at once instead of waiting for the other holder.
        let result = unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            return Err(lock_refusal(std::io::Error::last_os_error().raw_os_error()));
        }
        at_lock_taken(directory.path());
        // The lock is in the directory held: it has to be where it was judged, and the path has
        // to lead there still.
        if !chain_holds(&chain) || !leads_to_held(&directory, &held) {
            return Err(LedgerRefusal::Unavailable);
        }
        Ok(Self {
            directory,
            chain,
            held,
            lock,
            expected_sha256: None,
        })
    }

    /// The directory held is still where it was judged, and the wallet's path still names it.
    fn in_its_place(&self) -> bool {
        chain_holds(&self.chain) && leads_to_held(&self.directory, &self.held)
    }

    /// `None` only when nothing is there. A symlink, or something too large to be a ledger, is
    /// damage. `O_NONBLOCK` is for a FIFO in its place: the open returns, and the file-type check
    /// refuses it.
    fn read_file(&self) -> Result<Option<Vec<u8>>, LedgerRefusal> {
        let opened = storage::open_file_at(
            &self.held,
            LEDGER_FILE,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0,
        );
        let file = match opened {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => {
                return Err(LedgerRefusal::Corrupt);
            }
            Err(error) => return Err(unavailable(error)),
        };
        if !file.metadata().map_err(unavailable)?.is_file() || !has_one_name(&file)? {
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
        let mut file = storage::open_file_at(
            &self.held,
            LEDGER_TEMPORARY,
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
        .map_err(unavailable)?;
        // The truncate keeps a temporary's inode: one with another name would be the ledger's
        // after the rename, and changeable by that name.
        if !has_one_name(&file)? {
            return Err(LedgerRefusal::Unavailable);
        }
        file.write_all(&bytes).map_err(unavailable)?;
        file.sync_all().map_err(unavailable)?;
        drop(file);
        at_seam(Seam::BeforeRename, self.directory.path());
        storage::rename_at(&self.held, LEDGER_TEMPORARY, LEDGER_FILE).map_err(unavailable)?;
        self.held.sync_all().map_err(unavailable)?;
        self.expected_sha256 = Some(sha256_hex(&bytes));
        // What was written is in the directory held: if it is no longer where it was judged, or
        // the path no longer leads there, nothing after this goes on there.
        if !self.in_its_place() {
            return Err(LedgerRefusal::Vanished);
        }
        Ok(())
    }

    /// The lock in the directory held is the one held, with no other name, every directory of the
    /// way is where it was judged, the wallet's path still leads to the one held, and the file is
    /// what this handle last wrote, with no other name. A lock file replaced under a holder would
    /// let the next process hold another one, and a directory moved from where it was judged, or
    /// a file with a name elsewhere, could be where commands of the run write.
    fn verify_unchanged(&self) -> Result<(), LedgerRefusal> {
        let held = Identity::of(&self.lock).map_err(unavailable)?;
        let same_lock = storage::look_at(&self.held, OsStr::new(LOCK_FILE))
            .is_ok_and(|found| found == Some(Found::Entry(held)));
        if !same_lock {
            return Err(LedgerRefusal::Vanished);
        }
        if !has_one_name(&self.lock)? {
            return Err(LedgerRefusal::Corrupt);
        }
        if !self.in_its_place() {
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

/// Where a walk ends at a name that is not there: that name and the rest, below the real directory
/// the walk reached. A place a command could write is no less one for being gone now, as a cache
/// the sandbox holds can be, so where the wallet will be made is judged too. A rest that is not
/// plain names could lead anywhere once made, so it is not judged but refused, as G4 refuses one.
fn refuse_what_is_yet_to_be_made(
    missing: PathBuf,
    rest: &Path,
    bounds: &WriteBounds,
) -> Result<(), LedgerRefusal> {
    let mut leads_to = missing;
    for name in rest.components() {
        match name {
            Component::Normal(name) => leads_to.push(name),
            _ => return Err(LedgerRefusal::Unavailable),
        }
    }
    refuse_command_writes(&leads_to, bounds)
}

/// What a walk judges each name by.
#[derive(Clone, Copy)]
struct Judge<'a> {
    workspace: &'a Path,
    bounds: &'a WriteBounds,
}

/// Walks the path as the system does, one name at a time and with no link followed unseen, and
/// judges each real directory it enters. So a directory holding a name of the way, even a name in
/// the text of a link, has been judged before that name is read. What is not there yet ends the
/// walk: nothing below it can be a link, and what will be made there is judged as it will lead.
///
/// Every name is looked at and read from the directory before it, as a descriptor holds it, and a
/// directory is entered only as the one it was judged: the descriptor opened for it has the device
/// and inode its name had when it was looked at. So what is judged is what is held, and the walk
/// ends holding the directory where what is yet to be made will be made, with every directory
/// above it, so an open can ask that none of them has moved since. A link at the wallet's own name
/// is refused before it is followed: the path would not name the directory held, so nothing is
/// made where it leads. Only a test walks with no judge.
fn walk_the_way(
    directory: &WalletDirectory,
    judge: Option<Judge<'_>>,
) -> Result<Way, LedgerRefusal> {
    let mut place = PathBuf::new();
    // The directories of `place` from the root down, each held as it was entered, by its name.
    let mut held: Chain = Vec::new();
    let mut rest = directory.path().to_path_buf();
    let mut followed = 0;
    loop {
        let mut names = rest.components();
        let Some(name) = names.next() else {
            if held.is_empty() {
                return Err(LedgerRefusal::Unavailable);
            }
            return Ok(Way {
                chain: held,
                to_make: Vec::new(),
            });
        };
        let after = names.as_path().to_path_buf();
        rest = match name {
            Component::RootDir => {
                place = PathBuf::from(Component::RootDir.as_os_str());
                held = vec![(storage::open_root().map_err(unavailable)?, OsString::new())];
                after
            }
            Component::CurDir => after,
            // `place` holds no link, so its parent is the directory the system climbs to, and has
            // been judged and held on the way down, with the name it has there.
            Component::ParentDir => {
                place.pop();
                if held.len() > 1 {
                    held.pop();
                }
                after
            }
            Component::Prefix(_) => return Err(LedgerRefusal::Unavailable),
            Component::Normal(name) => {
                let (parent, _) = held.last().ok_or(LedgerRefusal::Unavailable)?;
                let next = place.join(name);
                match storage::look_at(parent, name).map_err(unavailable)? {
                    // Nothing follows the last name of what is left but the name itself: a link
                    // there is a link at the wallet's own name.
                    Some(Found::Link) if after.as_os_str().is_empty() => {
                        return Err(LedgerRefusal::Unavailable);
                    }
                    Some(Found::Link) => {
                        followed += 1;
                        if followed > MOST_LINKS_ON_THE_WAY {
                            return Err(LedgerRefusal::Unavailable);
                        }
                        storage::read_link_at(parent, name)
                            .map_err(unavailable)?
                            .join(after)
                    }
                    Some(Found::Entry(seen)) => {
                        if let Some(judge) = judge {
                            refuse_place(&next, judge.workspace)?;
                            refuse_command_writes(&next, judge.bounds)?;
                        }
                        at_seam(Seam::BeforeEntering, &next);
                        let entered =
                            storage::open_on_the_way(parent, name).map_err(unavailable)?;
                        if Identity::of(&entered).map_err(unavailable)? != seen {
                            return Err(LedgerRefusal::Unavailable);
                        }
                        held.push((entered, name.to_os_string()));
                        place = next;
                        after
                    }
                    None => {
                        if let Some(judge) = judge {
                            refuse_what_is_yet_to_be_made(next, &after, judge.bounds)?;
                        }
                        let to_make = names_to_make(name, &after)?;
                        return Ok(Way {
                            chain: held,
                            to_make,
                        });
                    }
                }
            }
        };
    }
}

/// The walk alone, as a test reads it.
#[cfg(test)]
fn refuse_every_name_on_the_way(
    directory: &WalletDirectory,
    workspace: &Path,
    bounds: &WriteBounds,
) -> Result<(), LedgerRefusal> {
    walk_the_way(directory, Some(Judge { workspace, bounds })).map(drop)
}

/// A directory commands of the run can write proves nothing, and neither does a path that runs
/// through one, so both are judged, once, when the wallet opens, and the walk that judges them
/// holds what it judged. After that no use resolves the path again: every file is reached from the
/// directory held. Every change first compares the lock in that directory with the one held, each
/// directory held with the one at its name in the directory before it, the directory at the
/// wallet's path with the one held, by device and inode, and the file with the digest of what this
/// handle last wrote ([`Store::verify_unchanged`]). So a swap that changes which lock is there,
/// where a directory of the way is, which directory the path names, or what the ledger holds is
/// refused. Where it leads comes first, so its refusals stay as G4 words them.
fn judge_the_way(
    directory: &WalletDirectory,
    workspace: &Path,
    bounds: &WriteBounds,
) -> Result<Way, LedgerRefusal> {
    refuse_place(directory.path(), workspace)?;
    walk_the_way(directory, Some(Judge { workspace, bounds }))
}

/// The judgement alone, as a test reads it.
#[cfg(test)]
fn refuse_directory(
    directory: &WalletDirectory,
    workspace: &Path,
    bounds: &WriteBounds,
) -> Result<(), LedgerRefusal> {
    judge_the_way(directory, workspace, bounds).map(drop)
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
        let bounds = WriteBounds::of(request.command_writes.as_ref(), request.workspace)?;
        let way = judge_the_way(&directory, request.workspace, &bounds)?;
        let workspace = fs::canonicalize(request.workspace).map_err(unavailable)?;
        let workspace_sha256 = sha256_hex(workspace.as_os_str().as_bytes());
        let explicit_profile = request
            .explicit_profile
            .map(canonical_explicit_profile)
            .transpose()?;
        at_seam(Seam::AfterWalk, directory.path());
        let mut store = Store::open_on(directory, way)?;
        let mut made_here = false;
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
                made_here = true;
                wallet
            }
        };
        Ok(Self {
            policy,
            wallet,
            store,
            clock: RunClock::new(request.clock),
            failed: None,
            made_here,
        })
    }

    /// Whether a resume may go on in this wallet: one this open made has no run before it to go on
    /// with, and the file has to be what this handle last wrote. Nothing is opened again: a send
    /// of this process may still be out, and an open would take it for one a crash left.
    pub(crate) fn refuse_resume(&mut self) -> Result<(), LedgerRefusal> {
        if self.made_here {
            return Err(LedgerRefusal::Missing);
        }
        self.verified()
    }

    /// Pins the first explicit profile, in this wallet's own transaction, once; another one later
    /// is refused, and so is one with nothing in its name. The same one again changes nothing.
    pub(crate) fn pin_profile(&mut self, explicit: &str) -> Result<(), LedgerRefusal> {
        let named = canonical_explicit_profile(explicit)?;
        self.verified()?;
        match self.wallet.profile.as_deref() {
            Some(pinned) if pinned == named => Ok(()),
            Some(_) => Err(LedgerRefusal::ProfileChanged),
            None => {
                let mut next = self.wallet.clone();
                next.profile = Some(named);
                if let Err(refusal) = self.store.write(&next) {
                    self.failed = Some(refusal);
                    return Err(refusal);
                }
                self.wallet = next;
                Ok(())
            }
        }
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
    use super::super::reservation::{
        AttemptState, BudgetRefusal, LatchReason, Money, WalletState, reservation_amount,
    };
    use super::super::settlement::{
        KnownBilling, ReportedUsage, SettleReport, UnknownReason, computed_charge,
    };
    use super::super::test_support::{
        BODY, FakeClock, Rng, WALLET, policy_value, request, stage_kind,
    };
    use super::{
        BudgetLedger, CommandWrites, LEDGER_FILE, LEDGER_TEMPORARY, LOCK_FILE, LedgerEntry,
        LedgerFailure, LedgerOpen, LedgerRefusal, MAX_LEDGER_BYTES, MOST_LINKS_ON_THE_WAY, Store,
        WalletDirectory, WriteBounds, fits_a_ledger, lock_refusal, refuse_directory,
        refuse_every_name_on_the_way,
    };
    use super::{Seam, own_user};
    use crate::planner::profile::canonical_profile_name;
    use crate::planner::structured_stages::receipt::sha256_hex;
    use crate::providers::BilledCostUsd;
    use crate::tools::os_sandbox::SandboxPolicy;

    /// The run's own temporary under `base`, as a sandbox makes one under its state directory.
    fn run_tmp_under(base: &Path) -> PathBuf {
        let run_tmp = base.join("state/sandbox/run-976");
        fs::create_dir_all(&run_tmp).unwrap();
        run_tmp
    }

    /// What a sandbox applied on `workspace` lets commands write, its temporary under `base` and
    /// its caches under `base/home`. No sandbox of the process is read.
    fn writes_under(base: &Path, workspace: &Path) -> CommandWrites {
        CommandWrites::of(SandboxPolicy::with_home(
            workspace,
            &run_tmp_under(base),
            base.join("home"),
        ))
    }

    struct World {
        // Held so the directories outlive the test.
        _root: tempfile::TempDir,
        base: PathBuf,
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
            // Made now, so that a test that sees what an open makes does not see this.
            run_tmp_under(&base);
            let ledgers = base.join("operator-ledgers");
            let mut value = policy_value();
            value["ledger_dir"] = json!(ledgers.to_str().unwrap());
            change(&mut value);
            let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
            Self {
                ledgers,
                workspace,
                base,
                _root: root,
                policy: Arc::new(policy),
                clock: FakeClock::at(7_000),
            }
        }

        /// What commands of a run on `workspace` can write, in this world.
        fn writes(&self, workspace: &Path) -> CommandWrites {
            writes_under(&self.base, workspace)
        }

        fn bounds(&self) -> WriteBounds {
            WriteBounds::of(Some(&self.writes(&self.workspace)), &self.workspace).unwrap()
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
                command_writes: Some(self.writes(&self.workspace)),
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
                command_writes: Some(self.writes(workspace)),
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

    /// Two ledgers as stages 1 and 2 wrote them (the record's types are as they were at fea0a54e),
    /// with the run's own digests left out: one with a known send, a released one and a
    /// submission, and no profile; one with those, an unknown send and its latch, and a profile.
    const STAGE_TWO_KNOWN: &str = r#"{"attempts":[{"body_sha256":"6012542fd3f685b595377ee835722576da753cfaac9f7e152946dc11c8bd7fd4","final_repair":null,"input_bound":110,"output_bound":50,"physical":1,"purpose":"executor","reserved":"720000000000000","settlement":{"billed":"0","billed_usd":"0","cached_tokens":null,"completion_tokens":40,"computed":"600000000000000","consumed":"600000000000000","late":false,"prompt_tokens":100,"reasoning_tokens":null},"state":"settled","unknown_reason":null},{"body_sha256":"6012542fd3f685b595377ee835722576da753cfaac9f7e152946dc11c8bd7fd4","final_repair":null,"input_bound":110,"output_bound":50,"physical":2,"purpose":"executor","reserved":"720000000000000","settlement":null,"state":"released","unknown_reason":null}],"budget_id":"wallet-alpha","clock":{"final_repair_started_ms":null,"high_water_elapsed_ms":0,"started_wall_ms":7000},"latch":null,"logicals":[{"attempt":null,"id":1,"kind":{"kind":"general"},"source":null}],"policy_sha256":"POLICY","profile":null,"schema":"run-budget.ledger/v1","workspace_sha256":"WORKSPACE"}"#;
    const STAGE_TWO_LATCHED: &str = r#"{"attempts":[{"body_sha256":"6012542fd3f685b595377ee835722576da753cfaac9f7e152946dc11c8bd7fd4","final_repair":null,"input_bound":110,"output_bound":50,"physical":1,"purpose":"executor","reserved":"720000000000000","settlement":{"billed":"0","billed_usd":"0","cached_tokens":null,"completion_tokens":40,"computed":"600000000000000","consumed":"600000000000000","late":false,"prompt_tokens":100,"reasoning_tokens":null},"state":"settled","unknown_reason":null},{"body_sha256":"6012542fd3f685b595377ee835722576da753cfaac9f7e152946dc11c8bd7fd4","final_repair":null,"input_bound":110,"output_bound":50,"physical":2,"purpose":"executor","reserved":"720000000000000","settlement":null,"state":"released","unknown_reason":null},{"body_sha256":"6012542fd3f685b595377ee835722576da753cfaac9f7e152946dc11c8bd7fd4","final_repair":null,"input_bound":110,"output_bound":50,"physical":3,"purpose":"executor","reserved":"720000000000000","settlement":null,"state":"unknown","unknown_reason":"timeout"}],"budget_id":"wallet-alpha","clock":{"final_repair_started_ms":null,"high_water_elapsed_ms":0,"started_wall_ms":7000},"latch":{"physical":3,"reason":"unknown_billing"},"logicals":[{"attempt":null,"id":1,"kind":{"kind":"general"},"source":null}],"policy_sha256":"POLICY","profile":"profile-a","schema":"run-budget.ledger/v1","workspace_sha256":"WORKSPACE"}"#;

    /// Stage 1 and 2's ledgers, read by this code: the counts, what is held and what is spent are
    /// what they were, and nothing reads them as more money.
    #[test]
    fn a_ledger_of_stages_one_and_two_reads_as_it_was() {
        type Row = (
            &'static str,
            Option<&'static str>,
            u64,
            &'static str,
            &'static str,
            u128,
            u128,
            bool,
        );
        let rows: [Row; 2] = [
            (STAGE_TWO_KNOWN, None, 1, "0.0006", "0", 140, 0, false),
            (
                STAGE_TWO_LATCHED,
                Some("profile-a"),
                2,
                "0.0006",
                "0.00072",
                140,
                160,
                true,
            ),
        ];
        for (body, profile, physical, spent, held, spent_tokens, held_tokens, latched) in rows {
            let world = World::new();
            drop(world.open_naming(profile).unwrap());
            let workspace = fs::canonicalize(&world.workspace).unwrap();
            let body = body.replace("POLICY", &world.policy.sha256).replace(
                "WORKSPACE",
                &sha256_hex(workspace.as_os_str().as_encoded_bytes()),
            );
            let sha256 = sha256_hex(body.as_bytes());
            fs::write(
                world.file(),
                format!("{{\"sha256\":\"{sha256}\",\"body\":{body}}}"),
            )
            .unwrap();
            for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
                let ledger = world.open_as(entry, &world.workspace, profile).unwrap();
                let wallet = ledger.wallet();
                assert_eq!(wallet.physical_used(), physical, "{entry:?}");
                assert_eq!(wallet.logical_global_used(), 1, "{entry:?}");
                assert_eq!(wallet.spent().unwrap(), money(spent), "{entry:?}");
                assert_eq!(wallet.held().unwrap(), money(held), "{entry:?}");
                assert_eq!(wallet.spent_tokens(), spent_tokens, "{entry:?}");
                assert_eq!(wallet.held_tokens(), held_tokens, "{entry:?}");
                assert_eq!(wallet.latch().is_some(), latched, "{entry:?}");
                assert_eq!(wallet.profile.as_deref(), profile, "{entry:?}");
                assert_eq!(wallet.attempts.len(), physical as usize + 1, "{entry:?}");
            }
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
            command_writes: Some(world.writes(&world.workspace)),
        };
        let LedgerOpen {
            policy,
            workspace,
            explicit_profile,
            entry,
            clock,
            command_writes,
        } = request;
        let mut first = BudgetLedger::open(LedgerOpen {
            policy,
            workspace,
            explicit_profile,
            entry,
            clock,
            command_writes,
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

    /// Records `reserve` could never write, each settled as nothing billed, with the digest right:
    /// no bytes past the overhead, or no output, even with a body read. The controls are the file
    /// as it was, and records one byte past the overhead with some output, which open as they did
    /// if their report hides no body.
    #[test]
    fn a_ledger_holding_a_send_reserve_could_never_write_is_refused() {
        let overhead = policy_value()["input_overhead"].as_u64().unwrap();
        let read_one = money("0.000002");
        /// The input bound, the output bound and the prompt tokens of the record.
        type Record = Option<(u64, u64, u64)>;
        let rows: [(&str, Record, Result<Money, LedgerRefusal>); 8] = [
            ("as it was", None, Ok(money("0.0006"))),
            (
                "at the overhead, nothing read",
                Some((overhead, 50, 0)),
                Err(LedgerRefusal::Corrupt),
            ),
            (
                "no input at all, nothing read",
                Some((0, 50, 0)),
                Err(LedgerRefusal::Corrupt),
            ),
            (
                "at the overhead with no output, nothing read",
                Some((overhead, 0, 0)),
                Err(LedgerRefusal::Corrupt),
            ),
            (
                "one past the overhead, nothing read",
                Some((overhead + 1, 50, 0)),
                Err(LedgerRefusal::Corrupt),
            ),
            (
                "one past the overhead with no output, one read",
                Some((overhead + 1, 0, 1)),
                Err(LedgerRefusal::Corrupt),
            ),
            (
                "one past the overhead, one read",
                Some((overhead + 1, 50, 1)),
                Ok(read_one),
            ),
            (
                "one past the overhead and one output, one read",
                Some((overhead + 1, 1, 1)),
                Ok(read_one),
            ),
        ];
        let mut wrong = Vec::new();
        for (name, record, expected) in rows {
            let world = World::new();
            let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
            send(&mut ledger);
            drop(ledger);
            if let Some((input_bound, output_bound, prompt_tokens)) = record {
                let tariff = &world.policy.tariff;
                let reserved = reservation_amount(tariff, input_bound, output_bound).unwrap();
                let computed = computed_charge(tariff, prompt_tokens, 0, None).unwrap();
                world.rewrite(|body| {
                    let attempt = &mut body["attempts"][0];
                    attempt["input_bound"] = json!(input_bound);
                    attempt["output_bound"] = json!(output_bound);
                    attempt["reserved"] = serde_json::to_value(reserved).unwrap();
                    let settlement = &mut attempt["settlement"];
                    settlement["billed_usd"] = json!("0");
                    settlement["billed"] = serde_json::to_value(Money::ZERO).unwrap();
                    settlement["computed"] = serde_json::to_value(computed).unwrap();
                    settlement["consumed"] = serde_json::to_value(computed).unwrap();
                    settlement["prompt_tokens"] = json!(prompt_tokens);
                    settlement["completion_tokens"] = json!(0);
                });
            }
            let got = world
                .open(LedgerEntry::Resume)
                .map(|reopened| reopened.wallet().spent().unwrap());
            if got != expected {
                wrong.push(format!("{name}: {got:?}"));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
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

    // ---- where commands of the run can write ----

    fn open_with(
        world: &World,
        ledger_dir: &Path,
        command_writes: Option<CommandWrites>,
    ) -> Result<BudgetLedger, LedgerRefusal> {
        let text = ledger_dir.to_str().unwrap();
        let policy = world.declaration(WALLET, |v| v["ledger_dir"] = json!(text));
        BudgetLedger::open(LedgerOpen {
            policy,
            workspace: &world.workspace,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: world.clock.clone(),
            command_writes,
        })
    }

    /// The run's own temporary is a place commands write, as the workspace is: a wallet in it, or
    /// on a way through it, proves nothing. Each row has a world of its own, and nothing is made.
    #[test]
    fn a_wallet_where_commands_of_the_run_can_write_is_refused_before_anything_is_made() {
        // The place the wallet is named by, and the name the policy gives the temporary by, when
        // it is not the temporary's own.
        type Way = fn(&Path, &Path) -> (PathBuf, Option<PathBuf>);
        let rows: Vec<(&str, Way)> = vec![
            ("R1 under the run's temporary", |_, tmp| {
                (tmp.join("wallets"), None)
            }),
            ("R2 one link into the run's temporary", |base, tmp| {
                let inner = made(tmp.join("inner"));
                (linked(&inner, base.join("outer")), None)
            }),
            (
                "R3 two links, the second in the run's temporary",
                |base, tmp| {
                    let safe = made(base.join("safe"));
                    let in_tmp = linked(&safe, tmp.join("m"));
                    (linked(&in_tmp, base.join("outer")), None)
                },
            ),
            ("R4 a temporary the policy names by a link", |base, tmp| {
                let alias = linked(tmp, base.join("tmp-alias"));
                (tmp.join("w"), Some(alias))
            }),
        ];
        let mut wrong = Vec::new();
        for (name, way) in rows {
            let world = World::new();
            let tmp = run_tmp_under(&world.base);
            let (ledger_dir, named) = way(&world.base, &tmp);
            let writes = match named {
                Some(alias) => CommandWrites::of(SandboxPolicy::with_home(
                    &world.workspace,
                    &alias,
                    world.base.join("home"),
                )),
                None => world.writes(&world.workspace),
            };
            let before = every_name_under(&world.base);
            let opened = open_with(&world, &ledger_dir, Some(writes)).map(|_| ());
            let made_anything = every_name_under(&world.base) != before;
            if opened != Err(LedgerRefusal::DirectoryWritableByCommands) || made_anything {
                wrong.push(format!(
                    "{name}: {opened:?}, made anything: {made_anything}"
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// The control: beside the temporary, and through links that stay out of every place commands
    /// write, a wallet opens where it leads.
    #[test]
    fn a_wallet_beside_the_run_s_temporary_or_through_links_outside_it_opens() {
        type Way = fn(&Path, &Path) -> (PathBuf, PathBuf);
        let rows: Vec<(&str, Way)> = vec![
            (
                "beside the temporary, under the sandbox's own directory",
                |_, tmp| {
                    let beside = tmp.parent().unwrap().join("wallets");
                    (beside.clone(), beside)
                },
            ),
            ("a link outside to a place outside", |base, _| {
                let safe = made(base.join("safe"));
                (linked(&safe, base.join("link")), safe)
            }),
        ];
        let mut wrong = Vec::new();
        for (name, way) in rows {
            let world = World::new();
            let tmp = run_tmp_under(&world.base);
            let (ledger_dir, leads_to) = way(&world.base, &tmp);
            let opened = open_at(&world, &ledger_dir).map(|_| ());
            let file_there = leads_to.join(WALLET).join(LEDGER_FILE).exists();
            if opened != Ok(()) || !file_there {
                wrong.push(format!("{name}: {opened:?}, file there: {file_there}"));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// With no bound known, no place is out of reach. The control is this run's own policy.
    #[test]
    fn a_wallet_with_no_sandbox_policy_known_is_refused_before_anything_is_made() {
        use LedgerRefusal::SandboxPolicyUnknown as Unknown;
        type Writes = fn(&World) -> Option<CommandWrites>;
        let rows: Vec<(&str, Writes, Result<(), LedgerRefusal>)> = vec![
            ("no policy", |_| None, Err(Unknown)),
            (
                "a policy of another workspace",
                |world| Some(world.writes(&made(world.base.join("other-workspace")))),
                Err(Unknown),
            ),
            (
                "a temporary that is not there",
                |world| {
                    Some(CommandWrites::of(SandboxPolicy::with_home(
                        &world.workspace,
                        &world.base.join("no-such-run-tmp"),
                        world.base.join("home"),
                    )))
                },
                Err(Unknown),
            ),
            (
                "this run's policy",
                |world| Some(world.writes(&world.workspace)),
                Ok(()),
            ),
        ];
        let mut wrong = Vec::new();
        for (name, writes, expected) in rows {
            let world = World::new();
            let writes = writes(&world);
            let before = every_name_under(&world.base);
            let opened = open_with(&world, &world.ledgers, writes).map(|_| ());
            let made_anything = every_name_under(&world.base) != before;
            let as_expected = match expected {
                Ok(()) => world.file().exists(),
                Err(_) => !made_anything,
            };
            if opened != expected || !as_expected {
                wrong.push(format!(
                    "{name}: {opened:?}, made anything: {made_anything}"
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    const APPLIED_POLICY_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_APPLIED_POLICY_CHILD";
    const APPLIED_POLICY_RAN: &str =
        "child-976-ran commands_writes_are_known_only_from_an_applied_sandbox";

    /// Runs only as the child of the test below: the sandbox it installs is its own process's.
    #[test]
    fn commands_writes_are_known_only_from_an_applied_sandbox_child() {
        use crate::tools::os_sandbox::{self, SandboxAvailability, SandboxMode, SandboxTestState};
        if std::env::var_os(APPLIED_POLICY_CHILD).is_none() {
            return;
        }
        let _sandbox = SandboxTestState::acquire();
        let root = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(root.path()).unwrap();
        let workspace = made(base.join("workspace"));
        let run_tmp = run_tmp_under(&base);
        let policy = || SandboxPolicy::with_home(&workspace, &run_tmp, base.join("home"));
        let known = || CommandWrites::applied().map(|writes| writes.policy().run_tmp_dir.clone());
        let mut seen = vec![("not installed", known())];
        let no_backend = SandboxAvailability::Unavailable("no-backend-s2v-976".to_string());
        os_sandbox::install_for_test(SandboxMode::Required, policy(), no_backend);
        seen.push(("required, no backend", known()));
        os_sandbox::install_for_test(SandboxMode::Off, policy(), SandboxAvailability::Available);
        seen.push(("off", known()));
        os_sandbox::install_for_test(SandboxMode::Auto, policy(), SandboxAvailability::Available);
        seen.push(("auto, applied", known()));
        os_sandbox::install_for_test(
            SandboxMode::Required,
            policy(),
            SandboxAvailability::Available,
        );
        seen.push(("required, applied", known()));
        os_sandbox::cleanup();
        seen.push(("after cleanup", known()));
        assert_eq!(
            seen,
            vec![
                ("not installed", None),
                ("required, no backend", None),
                ("off", None),
                ("auto, applied", Some(run_tmp.clone())),
                ("required, applied", Some(run_tmp.clone())),
                ("after cleanup", None),
            ]
        );
        println!("{APPLIED_POLICY_RAN}");
    }

    /// Only a sandbox applied on the run says where its commands write. The state is the
    /// process's, so the child has a process of its own.
    #[test]
    fn commands_writes_are_known_only_from_an_applied_sandbox() {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::commands_writes_are_known_only_from_an_applied_sandbox_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", home.path())
            .env(APPLIED_POLICY_CHILD, "marker-976-applied-policy-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(APPLIED_POLICY_RAN),
            "{APPLIED_POLICY_RAN} missing\n{stdout}\n{stderr}"
        );
    }

    const YET_TO_BE_MADE_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_YET_TO_BE_MADE_CHILD";
    const YET_TO_BE_MADE_RAN: &str =
        "child-976-ran what_is_yet_to_be_made_is_judged_as_it_will_lead";

    /// A world whose sandbox held a cache at `home/.npm` when it was installed, and the cache gone
    /// since, as a command or the operator can remove one. The policy keeps the cache it found.
    fn a_world_whose_cache_is_gone() -> (World, PathBuf, CommandWrites) {
        let world = World::new();
        let home = made(world.base.join("home"));
        let cache = made(home.join(".npm"));
        let policy =
            SandboxPolicy::with_home(&world.workspace, &run_tmp_under(&world.base), home.clone());
        assert_eq!(policy.caches.npm, Some(fs::canonicalize(&cache).unwrap()));
        fs::remove_dir(&cache).unwrap();
        (world, home, CommandWrites::of(policy))
    }

    /// Runs only as the child of the test below, whose environment holds a home of its own.
    #[test]
    fn what_is_yet_to_be_made_is_judged_as_it_will_lead_child() {
        use LedgerRefusal::{DirectoryWritableByCommands as Writable, Unavailable};
        if std::env::var_os(YET_TO_BE_MADE_CHILD).is_none() {
            return;
        }
        type Way = fn(&World, &Path) -> PathBuf;
        let rows: Vec<(&str, Way, Result<(), LedgerRefusal>)> = vec![
            (
                "a cache gone since the install",
                |_, home| home.join(".npm/wallets"),
                Err(Writable),
            ),
            (
                "a cache gone since the install, named through a link",
                |world, home| linked(home, world.base.join("to-home")).join(".npm/wallets"),
                Err(Writable),
            ),
            (
                "a cache gone since the install, spelled with a `.`",
                |_, home| home.join("./.npm/wallets"),
                Err(Writable),
            ),
            (
                "a rest that climbs out of a name that is not there",
                |world, _| {
                    made(world.base.join("real"));
                    linked(
                        world.base.join("real/missing/../elsewhere"),
                        world.base.join("l"),
                    )
                },
                Err(Unavailable),
            ),
            (
                "the control: beside the cache gone",
                |_, home| home.join("elsewhere/wallets"),
                Ok(()),
            ),
        ];
        let mut wrong = Vec::new();
        for (name, way, expected) in rows {
            let (world, home, writes) = a_world_whose_cache_is_gone();
            let ledger_dir = way(&world, &home);
            let before = every_name_under(&world.base);
            let opened = open_with(&world, &ledger_dir, Some(writes)).map(|_| ());
            let made_anything = every_name_under(&world.base) != before;
            let as_expected = match expected {
                Ok(()) => ledger_dir.join(WALLET).join(LEDGER_FILE).exists(),
                Err(_) => !made_anything,
            };
            if opened != expected || !as_expected {
                wrong.push(format!(
                    "{name}: {opened:?}, made anything: {made_anything}"
                ));
            }
        }

        // The walk alone, since G4 refuses a link to what is not there before the walk is reached.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let walked = |ledger_dir: &Path| {
            let text = ledger_dir.to_str().unwrap();
            let policy = world.declaration(WALLET, |v| v["ledger_dir"] = json!(text));
            refuse_every_name_on_the_way(
                &WalletDirectory::of(&policy),
                &world.workspace,
                &world.bounds(),
            )
        };
        let climbs_in = linked(
            tmp.parent().unwrap().join("missing/../run-976/w"),
            world.base.join("climbs-in"),
        );
        let walk = walked(&climbs_in);
        if walk != Err(Unavailable) {
            wrong.push(format!("the walk alone, a rest that climbs in: {walk:?}"));
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{YET_TO_BE_MADE_RAN}");
    }

    /// Where a walk ends at a name that is not there, what will be made is judged by where it will
    /// lead. The check of G4 reads `HOME`, so the child has a temporary one.
    #[test]
    fn what_is_yet_to_be_made_is_judged_as_it_will_lead() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::what_is_yet_to_be_made_is_judged_as_it_will_lead_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(YET_TO_BE_MADE_CHILD, "marker-976-yet-to-be-made-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(YET_TO_BE_MADE_RAN),
            "{YET_TO_BE_MADE_RAN} missing\n{stdout}\n{stderr}"
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
        let bounds = world.bounds();
        let walk = |ledger_dir: &Path| {
            let text = ledger_dir.to_str().unwrap();
            let policy = world.declaration(WALLET, |v| v["ledger_dir"] = json!(text));
            refuse_every_name_on_the_way(&WalletDirectory::of(&policy), &world.workspace, &bounds)
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
                refuse_directory(&directory, &world.workspace, &world.bounds()),
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
        let workspace = root.join("workspace");
        BudgetLedger::open(LedgerOpen {
            policy: Arc::new(policy),
            workspace: &workspace,
            explicit_profile: None,
            entry: LedgerEntry::Fresh,
            clock: FakeClock::at(1),
            command_writes: Some(writes_under(root, &workspace)),
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

    // ---- the window between the walk and the disk ----

    type Hook = Box<dyn FnMut(Seam, &Path)>;

    thread_local! {
        static SEAM_HOOK: std::cell::RefCell<Option<Hook>> = const { std::cell::RefCell::new(None) };
    }

    /// What a seam does in a test: the hook this thread installed, if any. It is taken out of its
    /// slot while it runs.
    pub(super) fn fire_seam(seam: Seam, at: &Path) {
        let hook = SEAM_HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(mut hook) = hook {
            hook(seam, at);
            SEAM_HOOK.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.is_none() {
                    *slot = Some(hook);
                }
            });
        }
    }

    /// Takes the hook away when dropped, however the test ends.
    struct SeamHook;

    impl Drop for SeamHook {
        fn drop(&mut self) {
            SEAM_HOOK.with(|slot| drop(slot.borrow_mut().take()));
        }
    }

    fn hook_seams(hook: impl FnMut(Seam, &Path) + 'static) -> SeamHook {
        SEAM_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
        SeamHook
    }

    /// A change made once, at the first `seam` whose path is `at`, or at the first `seam` at all.
    fn once_at(seam: Seam, at: Option<PathBuf>, change: impl FnOnce() + 'static) -> SeamHook {
        let mut change = Some(change);
        hook_seams(move |seen, path| {
            if seen == seam
                && at.as_deref().is_none_or(|at| at == path)
                && let Some(change) = change.take()
            {
                change();
            }
        })
    }

    /// How many sends the ledger in `directory` holds.
    fn attempts_in(directory: &Path) -> usize {
        let bytes = fs::read(directory.join(LEDGER_FILE)).unwrap();
        let envelope: Value = serde_json::from_slice(&bytes).unwrap();
        envelope["body"]["attempts"].as_array().map_or(0, Vec::len)
    }

    fn a_ledger_file_under(directory: &Path) -> bool {
        every_name_under(directory)
            .iter()
            .any(|path| path.ends_with(LEDGER_FILE))
    }

    const WINDOW_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_WINDOW_CHILD";
    const WINDOW_RAN: &str =
        "child-976-ran a_place_changed_after_its_walk_gives_no_wallet_where_commands_write";

    /// Runs only as the child of the test below, whose environment holds a home of its own. Each
    /// row changes the world at one seam of an open, and what it opens is refused. No ledger is
    /// ever in the run's temporary, and in the rows that say so nothing at all is made there.
    #[test]
    fn a_place_changed_after_its_walk_gives_no_wallet_where_commands_write_child() {
        if std::env::var_os(WINDOW_CHILD).is_none() {
            return;
        }
        type Way = fn(&World, &Path) -> (PathBuf, SeamHook);
        let rows: Vec<(&str, Way, bool)> = vec![
            (
                "a name yet to be made, linked into the temporary after the walk",
                |world, tmp| {
                    let (tmp, ledgers) = (tmp.to_path_buf(), world.ledgers.clone());
                    let hook = once_at(Seam::AfterWalk, None, move || {
                        linked(&tmp, ledgers);
                    });
                    (world.ledgers.clone(), hook)
                },
                true,
            ),
            (
                "a judged directory on the way, swapped for a link into the temporary after the walk",
                |world, tmp| {
                    let mid = made(world.base.join("mid"));
                    let (tmp, away, at) =
                        (tmp.to_path_buf(), world.base.join("mid-away"), mid.clone());
                    let hook = once_at(Seam::AfterWalk, None, move || {
                        fs::rename(&at, away).unwrap();
                        linked(&tmp, at);
                    });
                    (mid.join("ledgers"), hook)
                },
                true,
            ),
            (
                "a directory just made, swapped for a link into the temporary",
                |world, tmp| {
                    let (tmp, ledgers) = (tmp.to_path_buf(), world.ledgers.clone());
                    let aside = world.base.join("aside");
                    let hook = once_at(Seam::AfterMake, None, move || {
                        fs::rename(&ledgers, aside).unwrap();
                        linked(&tmp, ledgers);
                    });
                    (world.ledgers.clone(), hook)
                },
                true,
            ),
            (
                "the wallet's directory, swapped for a link into the temporary before its lock",
                |world, tmp| {
                    let (tmp, wallet) = (tmp.to_path_buf(), world.wallet_dir());
                    let aside = world.base.join("aside");
                    let hook = once_at(Seam::BeforeLock, None, move || {
                        fs::rename(&wallet, aside).unwrap();
                        linked(&tmp, wallet);
                    });
                    (world.ledgers.clone(), hook)
                },
                true,
            ),
            (
                "the wallet's directory, moved into the temporary before its lock, its path linked there",
                |world, tmp| {
                    let (moved, wallet) = (tmp.join("moved"), world.wallet_dir());
                    let hook = once_at(Seam::BeforeLock, None, move || {
                        fs::rename(&wallet, &moved).unwrap();
                        linked(&moved, wallet);
                    });
                    (world.ledgers.clone(), hook)
                },
                // The move itself puts a name in the temporary, so this row cannot ask for none.
                // That no lock is made in what was moved there is asked by
                // `a_way_moved_from_where_it_was_judged_is_used_no_more`.
                false,
            ),
            (
                "a judged directory on the way, swapped for one from the temporary before it is entered",
                |world, tmp| {
                    let mid = made(world.base.join("mid"));
                    let inner = made(tmp.join("inner"));
                    let (away, at) = (world.base.join("mid-away"), mid.clone());
                    let hook = once_at(Seam::BeforeEntering, Some(mid.clone()), move || {
                        fs::rename(&at, away).unwrap();
                        fs::rename(inner, &at).unwrap();
                    });
                    (mid.join("ledgers"), hook)
                },
                true,
            ),
        ];
        let mut wrong = Vec::new();
        for (name, way, nothing_made_there) in rows {
            let world = World::new();
            let tmp = run_tmp_under(&world.base);
            let (ledger_dir, hook) = way(&world, &tmp);
            let before = every_name_under(&tmp);
            let opened = open_at(&world, &ledger_dir).map(|_| ());
            drop(hook);
            let untouched = every_name_under(&tmp).is_subset(&before);
            let ledger_there = a_ledger_file_under(&tmp);
            if opened != Err(LedgerRefusal::Unavailable)
                || ledger_there
                || (nothing_made_there && !untouched)
            {
                wrong.push(format!(
                    "{name}: {opened:?}, ledger in the temporary: {ledger_there}, untouched: {untouched}"
                ));
            }
        }

        // A real directory where one was yet to be made is not made into a wallet either.
        let world = World::new();
        let ledgers = world.ledgers.clone();
        let hook = once_at(Seam::AfterWalk, None, move || {
            made(ledgers);
        });
        let opened = open_at(&world, &world.ledgers).map(|_| ());
        drop(hook);
        let inside = every_name_under(&world.ledgers);
        if opened != Err(LedgerRefusal::Unavailable) || !inside.is_empty() {
            wrong.push(format!(
                "a real directory where one is yet to be made: {opened:?}, {} names in it",
                inside.len()
            ));
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{WINDOW_RAN}");
    }

    /// The window between the walk and the disk, one seam at a time, with nothing timed: each
    /// change happens at a point the open passes. The check of G4 reads `HOME`, so the child has a
    /// temporary one.
    #[test]
    fn a_place_changed_after_its_walk_gives_no_wallet_where_commands_write() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_place_changed_after_its_walk_gives_no_wallet_where_commands_write_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(WINDOW_CHILD, "marker-976-window-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(WINDOW_RAN),
            "{WINDOW_RAN} missing\n{stdout}\n{stderr}"
        );
    }

    const HELD_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_HELD_CHILD";
    const HELD_RAN: &str = "child-976-ran a_wallet_moved_from_under_its_path_is_written_no_more";

    /// Runs only as the child of the test below, whose environment holds a home of its own. Each
    /// row opens a wallet, changes where its path leads, and asks for two changes.
    #[test]
    fn a_wallet_moved_from_under_its_path_is_written_no_more_child() {
        if std::env::var_os(HELD_CHILD).is_none() {
            return;
        }
        let vanished = Err(LedgerFailure::Ledger(LedgerRefusal::Vanished));
        let mut wrong = Vec::new();

        // Between two changes: moved into the temporary, its path linked there.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let moved = tmp.join("moved");
        fs::rename(world.wallet_dir(), &moved).unwrap();
        linked(&moved, world.wallet_dir());
        let (first, second) = (
            reserve(&mut ledger).map(|_| ()),
            reserve(&mut ledger).map(|_| ()),
        );
        let held = attempts_in(&moved);
        if first != vanished || second != vanished || held != 0 {
            wrong.push(format!(
                "moved into the temporary, linked: {first:?} {second:?}, sends there: {held}"
            ));
        }

        // Between two changes: another directory with a copy of the wallet in its place.
        let world = World::new();
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let copy = made(world.ledgers.join("copy"));
        for name in [LEDGER_FILE, LOCK_FILE] {
            fs::copy(world.wallet_dir().join(name), copy.join(name)).unwrap();
        }
        fs::rename(world.wallet_dir(), world.ledgers.join("away")).unwrap();
        fs::rename(&copy, world.wallet_dir()).unwrap();
        let first = reserve(&mut ledger).map(|_| ());
        let copied = attempts_in(&world.wallet_dir());
        if first != vanished || copied != 0 {
            wrong.push(format!(
                "a copy in its place: {first:?}, sends there: {copied}"
            ));
        }

        // Before a rename: the path swapped for a link into the temporary. The new version lands
        // in the directory held, which is not where commands write, and is the last.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let before = every_name_under(&tmp);
        let aside = world.ledgers.join("aside");
        let (wallet, to) = (world.wallet_dir(), tmp.clone());
        let at = aside.clone();
        let hook = once_at(Seam::BeforeRename, None, move || {
            fs::rename(&wallet, at).unwrap();
            linked(&to, wallet);
        });
        let first = reserve(&mut ledger).map(|_| ());
        drop(hook);
        let second = reserve(&mut ledger).map(|_| ());
        let landed = attempts_in(&aside);
        let untouched = every_name_under(&tmp).is_subset(&before);
        if first != vanished || second != vanished || landed != 1 || !untouched {
            wrong.push(format!(
                "a link before a rename: {first:?} {second:?}, sends held: {landed}, temporary untouched: {untouched}"
            ));
        }

        // Before a rename: the directory held moved into the temporary, its path linked there.
        // Known limit: that rename lands there, in the directory held; nothing after it does.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let moved = tmp.join("moved");
        let (wallet, to) = (world.wallet_dir(), moved.clone());
        let hook = once_at(Seam::BeforeRename, None, move || {
            fs::rename(&wallet, &to).unwrap();
            linked(&to, wallet);
        });
        let first = reserve(&mut ledger).map(|_| ());
        drop(hook);
        let second = reserve(&mut ledger).map(|_| ());
        let landed = attempts_in(&moved);
        if first != vanished || second != vanished || landed != 1 {
            wrong.push(format!(
                "moved before a rename: {first:?} {second:?}, sends there: {landed}"
            ));
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{HELD_RAN}");
    }

    /// A wallet whose path no longer names the directory held is not written again, wherever
    /// the directory went. The check of G4 reads `HOME`, so the child has a temporary one.
    #[test]
    fn a_wallet_moved_from_under_its_path_is_written_no_more() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_wallet_moved_from_under_its_path_is_written_no_more_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(HELD_CHILD, "marker-976-held-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(HELD_RAN),
            "{HELD_RAN} missing\n{stdout}\n{stderr}"
        );
    }

    const PASSED_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_PASSED_CHILD";
    const PASSED_RAN: &str = "child-976-ran a_way_through_links_outside_passes_every_seam";

    /// Runs only as the child of the test below. The control: through a link out of every
    /// command's reach, and with a tail to make, the open passes every seam and makes what it
    /// makes private to this user; a second open makes nothing and passes the rest.
    #[test]
    fn a_way_through_links_outside_passes_every_seam_child() {
        use std::os::unix::fs::PermissionsExt;
        if std::env::var_os(PASSED_CHILD).is_none() {
            return;
        }
        let world = World::new();
        let safe = made(world.base.join("safe"));
        let ledger_dir = linked(&safe, world.base.join("link")).join("more");
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let record = |seen: &std::rc::Rc<std::cell::RefCell<Vec<Seam>>>| {
            let seen = seen.clone();
            hook_seams(move |seam, _| {
                let mut seen = seen.borrow_mut();
                if seen.last() != Some(&seam) {
                    seen.push(seam);
                }
            })
        };
        let hook = record(&seen);
        let opened = open_at(&world, &ledger_dir).map(|_| ());
        drop(hook);
        let made_here = [safe.join("more"), safe.join("more").join(WALLET)];
        let modes: Vec<u32> = made_here
            .iter()
            .map(|path| fs::metadata(path).unwrap().permissions().mode() & 0o777)
            .collect();
        let first = seen.borrow_mut().drain(..).collect::<Vec<_>>();

        let hook = record(&seen);
        let again = open_at(&world, &ledger_dir).map(|_| ());
        drop(hook);
        let second = seen.borrow_mut().drain(..).collect::<Vec<_>>();

        assert_eq!(opened, Ok(()));
        assert!(safe.join("more").join(WALLET).join(LEDGER_FILE).exists());
        assert_eq!(modes, [0o700, 0o700]);
        use Seam::{AfterMake, AfterWalk, BeforeEntering, BeforeLock, BeforeRename};
        assert_eq!(
            first,
            [
                BeforeEntering,
                AfterWalk,
                AfterMake,
                BeforeLock,
                BeforeRename
            ]
        );
        assert_eq!(again, Ok(()));
        assert_eq!(second, [BeforeEntering, AfterWalk, BeforeLock]);
        println!("{PASSED_RAN}");
    }

    #[test]
    fn a_way_through_links_outside_passes_every_seam() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_way_through_links_outside_passes_every_seam_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(PASSED_CHILD, "marker-976-passed-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(PASSED_RAN),
            "{PASSED_RAN} missing\n{stdout}\n{stderr}"
        );
    }

    // ---- a way that moves, a name more, someone else's directory ----

    type Change = Box<dyn FnOnce()>;

    thread_local! {
        static PRETENDED_USER: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
        static LOCK_TAKEN_HOOK: std::cell::RefCell<Option<Change>> =
            const { std::cell::RefCell::new(None) };
    }

    /// The user an open takes itself to be, in this thread, when a test says another.
    pub(super) fn pretended_user() -> Option<u32> {
        PRETENDED_USER.with(std::cell::Cell::get)
    }

    struct Pretending;

    impl Drop for Pretending {
        fn drop(&mut self) {
            PRETENDED_USER.with(|user| user.set(None));
        }
    }

    fn pretend_to_be(user: u32) -> Pretending {
        PRETENDED_USER.with(|slot| slot.set(Some(user)));
        Pretending
    }

    pub(super) fn fire_lock_taken(_: &Path) {
        let change = LOCK_TAKEN_HOOK.with(|slot| slot.borrow_mut().take());
        if let Some(change) = change {
            change();
        }
    }

    struct LockTakenHook;

    impl Drop for LockTakenHook {
        fn drop(&mut self) {
            LOCK_TAKEN_HOOK.with(|slot| drop(slot.borrow_mut().take()));
        }
    }

    fn once_lock_taken(change: impl FnOnce() + 'static) -> LockTakenHook {
        LOCK_TAKEN_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(change)));
        LockTakenHook
    }

    /// What the temporary holds right after a change made it so. Compared with it later, it shows
    /// only what the open itself made there.
    type Snapshot = std::rc::Rc<std::cell::RefCell<BTreeSet<PathBuf>>>;

    /// `from` moved to `to` in the temporary `tmp`, with a link to where it went left at its name
    /// when `link` says so, and what `tmp` then holds in `seen`.
    fn move_into(from: &Path, to: &Path, link: bool, tmp: &Path, seen: &Snapshot) {
        fs::rename(from, to).unwrap();
        if link {
            linked(to, from.to_path_buf());
        }
        *seen.borrow_mut() = every_name_under(tmp);
    }

    fn policy_naming(world: &World, ledger_dir: &Path) -> Arc<RunBudgetPolicy> {
        let text = ledger_dir.to_str().unwrap();
        world.declaration(WALLET, |v| v["ledger_dir"] = json!(text))
    }

    const MOVED_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_MOVED_CHILD";
    const MOVED_RAN: &str = "child-976-ran a_way_moved_from_where_it_was_judged_is_used_no_more";

    /// Runs only as the child of the test below, whose environment holds a home of its own. Each
    /// row moves a directory of the way into the run's temporary at one point of an open, most of
    /// them with a link left at its name to where it went, so the path still leads to it. The
    /// open is refused, and nothing is made in the temporary but what the move put there.
    #[test]
    fn a_way_moved_from_where_it_was_judged_is_used_no_more_child() {
        if std::env::var_os(MOVED_CHILD).is_none() {
            return;
        }
        type Hook = Box<dyn std::any::Any>;
        type Row = fn(&World, &Path, &Snapshot) -> (PathBuf, Hook);
        let rows: Vec<(&str, Row)> = vec![
            (
                "after the walk, a directory on the way moved, a link left to it",
                |world, tmp, seen| {
                    let mid = made(world.base.join("mid"));
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("mid"));
                    let at = mid.clone();
                    let hook = once_at(Seam::AfterWalk, None, move || {
                        move_into(&at, &to, true, &tmp, &seen);
                    });
                    (mid.join("ledgers"), Box::new(hook))
                },
            ),
            (
                "after the walk, a directory on the way moved, no link left",
                |world, tmp, seen| {
                    let mid = made(world.base.join("mid"));
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("mid"));
                    let at = mid.clone();
                    let hook = once_at(Seam::AfterWalk, None, move || {
                        move_into(&at, &to, false, &tmp, &seen);
                    });
                    (mid.join("ledgers"), Box::new(hook))
                },
            ),
            (
                "before it is entered, a directory on the way moved, a link left to it",
                |world, tmp, seen| {
                    let mid = made(world.base.join("mid"));
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("mid"));
                    let at = mid.clone();
                    let hook = once_at(Seam::BeforeEntering, Some(mid.clone()), move || {
                        move_into(&at, &to, true, &tmp, &seen);
                    });
                    (mid.join("ledgers"), Box::new(hook))
                },
            ),
            (
                "after a make, the directory above it moved, a link left to it",
                |world, tmp, seen| {
                    let mid = made(world.base.join("mid"));
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("mid"));
                    let at = mid.clone();
                    let hook = once_at(Seam::AfterMake, None, move || {
                        move_into(&at, &to, true, &tmp, &seen);
                    });
                    (mid.join("ledgers"), Box::new(hook))
                },
            ),
            (
                "before the lock, the wallet's parent moved, a link left to it",
                |world, tmp, seen| {
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("ledgers"));
                    let at = world.ledgers.clone();
                    let hook = once_at(Seam::BeforeLock, None, move || {
                        move_into(&at, &to, true, &tmp, &seen);
                    });
                    (world.ledgers.clone(), Box::new(hook))
                },
            ),
            (
                "before the lock, the wallet's own directory moved, a link left to it",
                |world, tmp, seen| {
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("moved"));
                    let at = world.wallet_dir();
                    let hook = once_at(Seam::BeforeLock, None, move || {
                        move_into(&at, &to, true, &tmp, &seen);
                    });
                    (world.ledgers.clone(), Box::new(hook))
                },
            ),
            (
                "once the lock is held, the wallet's parent moved, a link left to it",
                |world, tmp, seen| {
                    let (tmp, seen, to) = (tmp.to_path_buf(), seen.clone(), tmp.join("ledgers"));
                    let at = world.ledgers.clone();
                    let hook = once_lock_taken(move || {
                        move_into(&at, &to, true, &tmp, &seen);
                    });
                    (world.ledgers.clone(), Box::new(hook))
                },
            ),
        ];
        let mut wrong = Vec::new();
        for (name, row) in rows {
            let world = World::new();
            let tmp = run_tmp_under(&world.base);
            let seen: Snapshot = std::rc::Rc::default();
            *seen.borrow_mut() = every_name_under(&tmp);
            let (ledger_dir, hook) = row(&world, &tmp, &seen);
            let opened = open_at(&world, &ledger_dir).map(|_| ());
            drop(hook);
            let made_there: Vec<_> = every_name_under(&tmp)
                .difference(&seen.borrow())
                .cloned()
                .collect();
            let ledger_there = a_ledger_file_under(&tmp);
            if opened != Err(LedgerRefusal::Unavailable) || ledger_there || !made_there.is_empty() {
                wrong.push(format!(
                    "{name}: {opened:?}, ledger in the temporary: {ledger_there}, made there: {}",
                    made_there.len()
                ));
            }
        }

        // The walk alone refuses a directory swapped for a link to where it went, before it is
        // entered: it does not enter by a link it did not read.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mid = made(world.base.join("mid"));
        let seen: Snapshot = std::rc::Rc::default();
        let (to, at, tmp_for) = (tmp.join("mid"), mid.clone(), tmp.clone());
        let hook = once_at(Seam::BeforeEntering, Some(mid.clone()), move || {
            move_into(&at, &to, true, &tmp_for, &seen);
        });
        let policy = policy_naming(&world, &mid.join("ledgers"));
        let walked = refuse_every_name_on_the_way(
            &WalletDirectory::of(&policy),
            &world.workspace,
            &world.bounds(),
        );
        drop(hook);
        if walked != Err(LedgerRefusal::Unavailable) {
            wrong.push(format!(
                "the walk alone, entering by a link to where it went: {walked:?}"
            ));
        }

        // Between two changes: the wallet's parent moved, a link left to it. Nothing is written
        // there.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let to = tmp.join("ledgers");
        move_into(&world.ledgers, &to, true, &tmp, &std::rc::Rc::default());
        let vanished = Err(LedgerFailure::Ledger(LedgerRefusal::Vanished));
        let (first, second) = (
            reserve(&mut ledger).map(|_| ()),
            reserve(&mut ledger).map(|_| ()),
        );
        let there = attempts_in(&to.join(WALLET));
        if first != vanished || second != vanished || there != 0 {
            wrong.push(format!(
                "the parent moved between changes: {first:?} {second:?}, sends there: {there}"
            ));
        }

        // Before a rename: the same move. That one version lands in the directory held, as the
        // module's limit says, and the change is refused at once, and every one after it.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let to = tmp.join("ledgers");
        let (at, to_for, tmp_for) = (world.ledgers.clone(), to.clone(), tmp.clone());
        let hook = once_at(Seam::BeforeRename, None, move || {
            move_into(&at, &to_for, true, &tmp_for, &std::rc::Rc::default());
        });
        let first = reserve(&mut ledger).map(|_| ());
        drop(hook);
        let second = reserve(&mut ledger).map(|_| ());
        let landed = attempts_in(&to.join(WALLET));
        if first != vanished || second != vanished || landed != 1 {
            wrong.push(format!(
                "the parent moved before a rename: {first:?} {second:?}, sends there: {landed}"
            ));
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{MOVED_RAN}");
    }

    /// A directory of the way moved from the place that was judged is not used, whether or not
    /// the path still leads to it. The check of G4 reads `HOME`, so the child has a temporary one.
    #[test]
    fn a_way_moved_from_where_it_was_judged_is_used_no_more() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_way_moved_from_where_it_was_judged_is_used_no_more_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(MOVED_CHILD, "marker-976-moved-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(MOVED_RAN),
            "{MOVED_RAN} missing\n{stdout}\n{stderr}"
        );
    }

    const NAMED_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_NAMED_CHILD";
    const NAMED_RAN: &str = "child-976-ran nothing_is_made_or_kept_that_was_not_judged";

    /// Runs only as the child of the test below. A link at the wallet's own name, a directory made
    /// that is not empty or not this user's, and a file of the wallet with another name in the
    /// run's temporary are each refused, and nothing is made where the link leads.
    #[test]
    fn nothing_is_made_or_kept_that_was_not_judged_child() {
        use std::os::unix::fs::MetadataExt;
        if std::env::var_os(NAMED_CHILD).is_none() {
            return;
        }
        let mut wrong = Vec::new();
        let unavailable = Err(LedgerRefusal::Unavailable);

        // A dangling link at the wallet's own name: before the open (the control, refused by
        // G4's check), after the place is judged and before the walk reads it, and on a walk with
        // no judge. Nothing is made where it leads.
        let world = World::new();
        made(world.ledgers.clone());
        let target = world.base.join("elsewhere-new");
        linked(&target, world.wallet_dir());
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        if opened != unavailable || target.exists() {
            wrong.push(format!(
                "a dangling link before the open: {opened:?}, made: {}",
                target.exists()
            ));
        }
        let opened = Store::open(WalletDirectory::of(&world.policy)).map(|_| ());
        if opened != unavailable || target.exists() {
            wrong.push(format!(
                "a dangling link, with no judge: {opened:?}, made: {}",
                target.exists()
            ));
        }
        let world = World::new();
        let ledgers = made(world.ledgers.clone());
        let target = world.base.join("elsewhere-new");
        let (to, wallet) = (target.clone(), world.wallet_dir());
        let hook = once_at(Seam::BeforeEntering, Some(ledgers), move || {
            linked(&to, wallet);
        });
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        drop(hook);
        if opened != unavailable || target.exists() {
            wrong.push(format!(
                "a dangling link after the judgement: {opened:?}, made: {}",
                target.exists()
            ));
        }

        // A directory just made, swapped for a real one with something in it.
        let world = World::new();
        let ledgers = world.ledgers.clone();
        let hook = once_at(Seam::AfterMake, None, move || {
            fs::remove_dir(&ledgers).unwrap();
            made(ledgers.clone());
            fs::write(ledgers.join("planted"), b"planted").unwrap();
        });
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        drop(hook);
        if opened != unavailable || world.wallet_dir().exists() {
            wrong.push(format!(
                "a directory made, swapped for one not empty: {opened:?}, wallet made: {}",
                world.wallet_dir().exists()
            ));
        }

        // A directory made that is not this user's, as an open sees it when it takes itself for
        // another. With no such pretence it is the user the system says.
        let world = World::new();
        let real = fs::metadata(&world.base).unwrap().uid();
        let pretence = pretend_to_be(real.wrapping_add(1));
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        drop(pretence);
        if opened != unavailable || world.wallet_dir().exists() {
            wrong.push(format!(
                "a directory made, not this user's: {opened:?}, wallet made: {}",
                world.wallet_dir().exists()
            ));
        }
        if own_user() != real {
            wrong.push(format!("with no pretence: {} is not {real}", own_user()));
        }

        // The ledger with another name in the temporary, an older version written through it
        // between two processes: the next open refuses it.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let older = fs::read(world.file()).unwrap();
        send(&mut ledger);
        let other = tmp.join("ledger-by-another-name");
        fs::hard_link(world.file(), &other).unwrap();
        drop(ledger);
        fs::write(&other, &older).unwrap();
        let reopened = world.open(LedgerEntry::Resume).map(|_| ());
        if reopened != Err(LedgerRefusal::Corrupt) {
            wrong.push(format!(
                "the ledger with another name, at the next open: {reopened:?}"
            ));
        }

        // The lock with another name in the temporary, before the open.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        made(world.wallet_dir());
        fs::write(world.wallet_dir().join(LOCK_FILE), b"").unwrap();
        fs::hard_link(world.wallet_dir().join(LOCK_FILE), tmp.join("lock")).unwrap();
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        if opened != unavailable || world.file().exists() {
            wrong.push(format!(
                "the lock with another name, at the open: {opened:?}, ledger made: {}",
                world.file().exists()
            ));
        }

        // During a run: the lock, then the ledger, given another name in the temporary.
        let corrupt = Err(LedgerFailure::Ledger(LedgerRefusal::Corrupt));
        for file in [LOCK_FILE, LEDGER_FILE] {
            let world = World::new();
            let tmp = run_tmp_under(&world.base);
            let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
            fs::hard_link(world.wallet_dir().join(file), tmp.join(file)).unwrap();
            let first = reserve(&mut ledger).map(|_| ());
            let sends = attempts_in(&world.wallet_dir());
            if first != corrupt || sends != 0 {
                wrong.push(format!(
                    "{file} with another name during a run: {first:?}, sends: {sends}"
                ));
            }
        }

        // A temporary with another name in the temporary of the run, put in place before a change:
        // no version is written through it, and it does not become the ledger.
        let world = World::new();
        let tmp = run_tmp_under(&world.base);
        let mut ledger = world.open(LedgerEntry::Fresh).unwrap();
        let temporary = world.wallet_dir().join(LEDGER_TEMPORARY);
        fs::write(&temporary, b"planted").unwrap();
        fs::hard_link(&temporary, tmp.join("temporary")).unwrap();
        let first = reserve(&mut ledger).map(|_| ());
        let sends = attempts_in(&world.wallet_dir());
        let through = fs::read(tmp.join("temporary")).unwrap();
        let written_through = String::from_utf8_lossy(&through).contains("sha256");
        if first != Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
            || sends != 0
            || written_through
        {
            wrong.push(format!(
                "a temporary with another name: {first:?}, sends: {sends}, written through it: {written_through}"
            ));
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{NAMED_RAN}");
    }

    #[test]
    fn nothing_is_made_or_kept_that_was_not_judged() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::nothing_is_made_or_kept_that_was_not_judged_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(NAMED_CHILD, "marker-976-named-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(NAMED_RAN),
            "{NAMED_RAN} missing\n{stdout}\n{stderr}"
        );
    }

    const AGAIN_CHILD: &str = "COMMANDAGENT_TEST_BUDGET_AGAIN_CHILD";
    const AGAIN_RAN: &str =
        "child-976-ran a_link_on_the_way_and_a_directory_made_are_compared_again";

    /// Runs only as the child of the test below. A link on the way, out of every command's reach,
    /// is turned after the walk to another real directory outside with the same names below it:
    /// the open is refused, nothing is made where it now leads, and no ledger is written where it
    /// led. Turned before anything is held, no lock is opened either. A directory just made,
    /// swapped for another empty one of this user's, or given something inside, is refused, and
    /// no wallet is made.
    #[test]
    fn a_link_on_the_way_and_a_directory_made_are_compared_again_child() {
        if std::env::var_os(AGAIN_CHILD).is_none() {
            return;
        }
        let mut wrong = Vec::new();
        let unavailable = Err(LedgerRefusal::Unavailable);

        // The point the link is turned at, and whether a lock may be left where it first led.
        let points: [(&str, bool); 3] = [
            ("after the walk", false),
            ("before the lock", true),
            ("once the lock is held", true),
        ];
        for (point, lock_may_be_left) in points {
            let world = World::new();
            let first = made(world.base.join("safe1"));
            let second = made(world.base.join("safe2"));
            let (first_wallet, second_wallet) = (
                made(first.join("more").join(WALLET)),
                made(second.join("more").join(WALLET)),
            );
            let link = linked(&first, world.base.join("link"));
            let before = every_name_under(&second);
            let (at, to) = (link.clone(), second.clone());
            let turn = move || {
                fs::remove_file(&at).unwrap();
                linked(&to, at);
            };
            let hook: Box<dyn std::any::Any> = match point {
                "after the walk" => Box::new(once_at(Seam::AfterWalk, None, turn)),
                "before the lock" => Box::new(once_at(Seam::BeforeLock, None, turn)),
                _ => Box::new(once_lock_taken(turn)),
            };
            let opened = open_at(&world, &link.join("more")).map(|_| ());
            drop(hook);
            let made_there = every_name_under(&second).difference(&before).count();
            let lock_left = first_wallet.join(LOCK_FILE).exists();
            let ledger_left = first_wallet.join(LEDGER_FILE).exists();
            if opened != unavailable
                || made_there != 0
                || second_wallet.join(LEDGER_FILE).exists()
                || ledger_left
                || (!lock_may_be_left && lock_left)
            {
                wrong.push(format!(
                    "a link on the way turned {point}: {opened:?}, made where it leads now: \
                     {made_there}, ledger where it led: {ledger_left}, lock there: {lock_left}"
                ));
            }
        }

        // A directory just made, swapped for another empty one of this user's: only the device
        // and inode seen just after the make tell them apart.
        let world = World::new();
        let (ledgers, aside) = (world.ledgers.clone(), world.base.join("aside"));
        let (at, away) = (ledgers.clone(), aside.clone());
        let hook = once_at(Seam::AfterMake, None, move || {
            fs::rename(&at, away).unwrap();
            made(at);
        });
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        drop(hook);
        if opened != unavailable || world.wallet_dir().exists() || aside.join(WALLET).exists() {
            wrong.push(format!(
                "a directory made, swapped for another empty one: {opened:?}, wallet made: {}",
                world.wallet_dir().exists() || aside.join(WALLET).exists()
            ));
        }

        // A directory just made, the same one, with something put inside it.
        let world = World::new();
        let ledgers = world.ledgers.clone();
        let hook = once_at(Seam::AfterMake, None, move || {
            fs::write(ledgers.join("planted"), b"planted").unwrap();
        });
        let opened = world.open(LedgerEntry::Fresh).map(|_| ());
        drop(hook);
        if opened != unavailable || world.wallet_dir().exists() {
            wrong.push(format!(
                "a directory made, with something put inside: {opened:?}, wallet made: {}",
                world.wallet_dir().exists()
            ));
        }
        assert_eq!(wrong, Vec::<String>::new());
        println!("{AGAIN_RAN}");
    }

    #[test]
    fn a_link_on_the_way_and_a_directory_made_are_compared_again() {
        let root = tempfile::tempdir().unwrap();
        let home = made(root.path().join("home"));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "structured_budget::ledger::tests::a_link_on_the_way_and_a_directory_made_are_compared_again_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env_clear()
            .env("HOME", &home)
            .env(AGAIN_CHILD, "marker-976-again-child")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("1 passed"), "{stdout}\n{stderr}");
        assert!(
            stdout.contains(AGAIN_RAN),
            "{AGAIN_RAN} missing\n{stdout}\n{stderr}"
        );
    }
}
