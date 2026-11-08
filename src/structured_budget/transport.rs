//! One provider send under a wallet: reserve, start, POST, then settle or unknown, each on disk
//! before the next.
//!
//! Before an HTTP POST, a reservation and a start for the same bytes are on disk in the same wallet,
//! and the POST's body is what the permit gives. Every way a send can end without a known bill
//! leaves it `unknown`, holding its reservation, and stops the wallet: no usage, no cost, a cut or
//! unreadable body, an HTTP error, a redirect, a timeout, a cancel, a send error. There is no path
//! that settles it at zero. A retry takes a permit of its own, and since the send before it is
//! unknown it gets none, so a retry loop stays as it is and sends nothing more.
//!
//! The settling comes before anyone reads the response for what it says. A known report whose
//! response cannot be used has still been paid for.
//!
//! A wallet protects itself only from commands the sandbox holds, and only if the workspace it was
//! checked against is the one the sandbox holds them in. So a send also needs the sandbox applied
//! and on that workspace, and holding commands to the very places the wallet was judged against:
//! the workspace, the run's temporary, and the caches, as they were when it opened. A sandbox
//! installed again since, even on the same workspace, has a temporary of its own, and may let
//! commands write more; no send goes out under it.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, RequestBuilder, Response, redirect};
use serde_json::Value;
use serde_json::value::RawValue;

use super::identity::{
    AttemptBinding, LogicalId, LogicalKind, PhysicalId, ResponseIdentity, SendPurpose,
    SubmissionSource,
};
use super::ledger::{BudgetLedger, LedgerFailure, LedgerOpen, LedgerRefusal, StartedPermit};
use super::policy::RunBudgetPolicy;
use super::reservation::{AttemptState, ReserveRequest, WalletState};
use super::session::FinalRepairMarker;
use super::settlement::{ReportedUsage, SettleReport, UnknownReason};
use crate::providers::BilledCostUsd;
use crate::tools::os_sandbox;

/// The wallet of a run, on [`crate::config::ConfigFieldSources`]. That is not where a wallet belongs:
/// it is there because 68 literals of `Config` in the tests list every field and all of them fill
/// this one from its `Default`, so a clone of a `Config` carries the handle and no literal changes.
/// `None` is off, and then nothing here runs.
pub(crate) type RunBudgetSlot = Option<Arc<RunBudgetHandle>>;

/// Why a send did not leave. Fixed words only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendRefusal {
    Wallet(LedgerFailure),
    /// No sandbox holds the commands of this run, so nothing keeps them from the wallet.
    SandboxNotApplied,
    /// The sandbox holds them in another workspace than the one the wallet was checked against.
    SandboxOnAnotherWorkspace,
    /// The sandbox holds them to other places than the ones the wallet was judged against when it
    /// opened: another temporary, or caches added or gone.
    SandboxChangedSinceOpen,
    Unavailable,
}

impl std::fmt::Display for SendRefusal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "the run budget refused the send: {self:?}")
    }
}

impl std::error::Error for SendRefusal {}

impl From<LedgerFailure> for SendRefusal {
    fn from(failure: LedgerFailure) -> Self {
        Self::Wallet(failure)
    }
}

/// One wallet, held for as long as this value lives, and shared by every clone of a client.
#[derive(Debug)]
pub(crate) struct RunBudgetHandle {
    ledger: Mutex<BudgetLedger>,
    policy: Arc<RunBudgetPolicy>,
    /// As the wallet was checked against it, canonical.
    workspace: PathBuf,
    /// Where the sandbox applied on the run let commands write when the wallet was judged by it.
    opened_under: Option<SandboxSnapshot>,
    /// The command running now, and the first send of it that the wallet refused, in memory only:
    /// it is that command's, and stops every later send of it (`command.rs`).
    command: Mutex<CommandRecord>,
    /// What a test says the sandbox is, in place of the state of the process. `None` reads that
    /// state, as a run does.
    #[cfg(test)]
    sandbox_view: Option<SandboxView>,
}

/// The command running now, by its number from the first, and the first refusal of it. A REPL's
/// next command has the next number; a send keeps the number of the command it began in, so a
/// refusal a late worker of an earlier command meets is that command's, and lands on no other.
#[derive(Debug, Default)]
struct CommandRecord {
    number: u64,
    refusal: Option<SendRefusal>,
    /// The refusal of each earlier command the wallet refused, by its number: a late worker of one
    /// of them is refused with it, and no other command reads it.
    earlier: HashMap<u64, SendRefusal>,
}

/// The sandbox as one test sees it, so a test of a wallet does not have to install it in a state
/// every test of the process reads.
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct SandboxView {
    /// The workspace the sandbox holds commands in, or `None` when it is not applied.
    pub(crate) applied_on: Option<PathBuf>,
}

/// Two handles are equal only when they are one handle, whatever their wallets hold. This is what
/// comparing two [`RunBudgetSlot`]s says, and all it says.
impl PartialEq for RunBudgetHandle {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for RunBudgetHandle {}

/// Every place a sandbox policy lets commands write, each as it really leads.
///
/// The workspace and the run's temporary that cannot be resolved are `None`, and so never equal to
/// the resolved ones a wallet opens with. A cache that cannot be resolved is kept as the policy
/// spells it, with nothing to tell it apart: a cache whose path is already canonical, removed
/// since the open, compares equal to the one the wallet was judged against. That is meant. Its
/// path is still the policy's, so it is still where commands may write; the ledger's bounds keep
/// such a path as a root just the same (`WriteBounds::of` in `ledger.rs`), and a wallet was judged
/// against it as one. A cache added, or one under another path, does differ, and is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SandboxSnapshot {
    workspace: Option<PathBuf>,
    run_tmp_dir: Option<PathBuf>,
    caches: Vec<PathBuf>,
}

impl SandboxSnapshot {
    fn of(policy: &os_sandbox::SandboxPolicy) -> Self {
        let caches = &policy.caches;
        let mut roots = [
            &caches.cargo_registry,
            &caches.cargo_git,
            &caches.cargo_package_cache,
            &caches.npm,
            &caches.pip,
            &caches.playwright,
        ]
        .into_iter()
        .flatten()
        .map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.clone()))
        .collect::<Vec<_>>();
        roots.sort();
        Self {
            workspace: fs::canonicalize(&policy.workspace_root).ok(),
            run_tmp_dir: fs::canonicalize(&policy.run_tmp_dir).ok(),
            caches: roots,
        }
    }
}

/// Whether the sandbox holds commands in the workspace this wallet was checked against.
fn refuse_another_workspace(
    wallet_workspace: &Path,
    installed: Option<&Path>,
) -> Result<(), SendRefusal> {
    let installed = installed.ok_or(SendRefusal::SandboxNotApplied)?;
    match fs::canonicalize(installed) {
        Ok(installed) if installed == wallet_workspace => Ok(()),
        _ => Err(SendRefusal::SandboxOnAnotherWorkspace),
    }
}

impl RunBudgetHandle {
    pub(crate) fn open(request: LedgerOpen<'_>) -> Result<Self, LedgerRefusal> {
        let workspace =
            fs::canonicalize(request.workspace).map_err(|_| LedgerRefusal::Unavailable)?;
        let policy = request.policy.clone();
        let opened_under = request
            .command_writes
            .as_ref()
            .map(|writes| SandboxSnapshot::of(writes.policy()));
        let ledger = BudgetLedger::open(request)?;
        Ok(Self {
            ledger: Mutex::new(ledger),
            policy,
            workspace,
            opened_under,
            command: Mutex::new(CommandRecord::default()),
            #[cfg(test)]
            sandbox_view: None,
        })
    }

    fn command_record(&self) -> MutexGuard<'_, CommandRecord> {
        self.command.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The number of the command running now.
    pub(super) fn command_now(&self) -> u64 {
        self.command_record().number
    }

    /// The refusal kept for command `command`: the running one's while it runs, and an earlier
    /// one's after it, for its late workers only.
    pub(super) fn refusal_kept_for(&self, command: u64) -> Option<SendRefusal> {
        let record = self.command_record();
        if record.number == command {
            record.refusal
        } else {
            record.earlier.get(&command).copied()
        }
    }

    /// Keeps a refusal of a send of command `command` for that command, before anyone could read
    /// it, while it is the one running. A refusal of an earlier command is kept for none.
    fn note_refused(&self, command: u64, refusal: SendRefusal) {
        let mut record = self.command_record();
        if record.number == command {
            record.refusal.get_or_insert(refusal);
        }
    }

    /// Keeps `refusal` for the command running now, as a refused send's, and gives it back.
    pub(super) fn refuse_for_command(&self, refusal: SendRefusal) -> SendRefusal {
        self.note_refused(self.command_now(), refusal);
        refusal
    }

    /// One submission of the command running now (`submission.rs`). Once the wallet refused
    /// anything of this command, nothing more of it is recorded; a refusal here is kept for the
    /// command like a refused send's.
    pub(super) fn record_submission(
        &self,
        kind: LogicalKind,
        source: Option<SubmissionSource>,
    ) -> Result<LogicalId, SendRefusal> {
        let command = self.command_now();
        if let Some(refused) = super::command::refusal_of(self, command) {
            return Err(refused);
        }
        self.ledger()
            .and_then(|mut ledger| Ok(ledger.record_submission(kind, source)?))
            .inspect_err(|refusal| self.note_refused(command, *refusal))
    }

    /// Ties G4's attempt to a submission, on disk. A refusal is kept for the command.
    pub(super) fn bind_attempt(
        &self,
        logical: LogicalId,
        binding: AttemptBinding,
    ) -> Result<(), SendRefusal> {
        let command = self.command_now();
        self.ledger()
            .and_then(|mut ledger| Ok(ledger.bind_attempt(logical, binding)?))
            .inspect_err(|refusal| self.note_refused(command, *refusal))
    }

    /// The first send of the command running now that the wallet refused.
    pub(crate) fn refused_in_command(&self) -> Option<SendRefusal> {
        self.command_record().refusal
    }

    /// A REPL's next command: it has the next number, and no refusal of its own yet. The last
    /// command's refusal stays its own.
    pub(crate) fn forget_command_refusals(&self) {
        let mut record = self.command_record();
        if let Some(refusal) = record.refusal.take() {
            let number = record.number;
            record.earlier.insert(number, refusal);
        }
        record.number = record.number.wrapping_add(1);
    }

    /// Whether a resume may go on in this wallet (`command.rs`). Nothing is opened again.
    pub(super) fn refuse_resume(&self) -> Result<(), LedgerRefusal> {
        self.ledger()
            .map_err(|_| LedgerRefusal::Unavailable)?
            .refuse_resume()
    }

    /// Pins `explicit` as the wallet's profile, in the ledger's own transaction (`command.rs`).
    pub(super) fn pin_profile(&self, explicit: &str) -> Result<(), LedgerRefusal> {
        self.ledger()
            .map_err(|_| LedgerRefusal::Unavailable)?
            .pin_profile(explicit)
    }

    /// This handle, judging the sandbox by `view` instead of by the state of the process.
    #[cfg(test)]
    pub(crate) fn with_sandbox_view(mut self, view: SandboxView) -> Self {
        self.sandbox_view = Some(view);
        self
    }

    /// The most output a body of this wallet may ask for.
    pub(crate) fn max_output_tokens(&self) -> u64 {
        self.policy.max_output_tokens
    }

    /// A panic elsewhere with the lock held says nothing about the file, and the next change holds
    /// the file to what this handle wrote, so the wallet is as safe to refuse from as ever.
    fn ledger(&self) -> Result<MutexGuard<'_, BudgetLedger>, SendRefusal> {
        self.ledger.lock().map_err(|_| SendRefusal::Unavailable)
    }

    pub(crate) fn wallet(&self) -> Result<WalletState, SendRefusal> {
        Ok(self.ledger()?.wallet().clone())
    }

    pub(crate) fn success_gate(&self) -> Result<(), SendRefusal> {
        Ok(self.ledger()?.success_gate()?)
    }

    fn refuse_without_the_sandbox(&self) -> Result<(), SendRefusal> {
        #[cfg(test)]
        if let Some(view) = &self.sandbox_view {
            return refuse_another_workspace(&self.workspace, view.applied_on.as_deref());
        }
        // One reading of what is applied now, before anything else: none is no sandbox.
        let Some(now) = os_sandbox::applied_policy() else {
            return Err(SendRefusal::SandboxNotApplied);
        };
        refuse_another_workspace(
            &self.workspace,
            os_sandbox::installed_workspace_root().as_deref(),
        )?;
        if self.opened_under.as_ref() != Some(&SandboxSnapshot::of(&now)) {
            return Err(SendRefusal::SandboxChangedSinceOpen);
        }
        Ok(())
    }

    /// Reserves and starts these bytes. A start that is refused has sent nothing, so its reservation
    /// goes back. A send of a final-repair session claims, with the ledger locked, a new final
    /// repair when its session holds none yet and the session's one otherwise, and the session
    /// keeps the one a new claim opened. Nothing of command `command` goes once the wallet refused
    /// anything of it.
    fn begin<'b>(
        self: &Arc<Self>,
        command: u64,
        body: &'b [u8],
        purpose: SendPurpose,
        final_repair: Option<&FinalRepairMarker>,
    ) -> Result<(OpenSend, StartedPermit<'b>), SendRefusal> {
        if let Some(refused) = super::command::refusal_of(self, command) {
            return Err(refused);
        }
        self.refuse_without_the_sandbox()?;
        let mut ledger = self.ledger()?;
        let reservation = ledger.reserve(&ReserveRequest {
            body,
            purpose,
            final_repair: final_repair.map(FinalRepairMarker::claim),
        })?;
        if let Some(marker) = final_repair {
            marker.opened(reservation.final_repair);
        }
        let physical = reservation.physical;
        match ledger.start(physical, body) {
            Ok(permit) => Ok((
                OpenSend {
                    handle: self.clone(),
                    command,
                    physical,
                    resolved: false,
                },
                permit,
            )),
            Err(refusal) => {
                let _ = ledger.release(physical);
                Err(refusal.into())
            }
        }
    }

    /// Leaves the send settled or unknown. A response read after the send went unknown is settled
    /// once and the latch stays; one that says nothing known changes nothing.
    pub(crate) fn settle_response(
        &self,
        physical: PhysicalId,
        text: &str,
    ) -> Result<Option<SettleReport>, SendRefusal> {
        let mut ledger = self.ledger()?;
        let usage = match read_usage(text) {
            Ok(usage) => usage,
            Err(reason) => {
                let state = ledger
                    .wallet()
                    .attempt(physical)
                    .map(|attempt| attempt.state);
                if state == Some(AttemptState::Started) {
                    ledger.mark_unknown(physical, reason)?;
                    return Ok(Some(SettleReport::Unknown(reason)));
                }
                return Ok(None);
            }
        };
        Ok(Some(ledger.settle_reported(physical, &usage.reported())?))
    }

    fn mark_unknown(&self, physical: PhysicalId, reason: UnknownReason) {
        // A file that cannot take it stays `started`, which the next process reads as unknown, and
        // the handle refuses everything from here on.
        if let Ok(mut ledger) = self.ledger() {
            let _ = ledger.mark_unknown(physical, reason);
        }
    }
}

/// A send that has left, or may have. It ends settled or unknown: dropped unresolved, as a cancel
/// does on its way out, it is unknown.
#[derive(Debug)]
pub(crate) struct OpenSend {
    handle: Arc<RunBudgetHandle>,
    /// The command it began in.
    command: u64,
    physical: PhysicalId,
    resolved: bool,
}

impl OpenSend {
    fn unknown(mut self, reason: UnknownReason) {
        self.resolved = true;
        self.handle.mark_unknown(self.physical, reason);
    }

    /// The response, when this settled it: only a settled send is one a call can come from.
    fn settle(mut self, text: &str) -> Result<Option<ResponseIdentity>, SendRefusal> {
        self.resolved = true;
        self.handle
            .settle_response(self.physical, text)
            .map(|report| match report {
                Some(SettleReport::Settled(outcome)) => Some(outcome.response),
                Some(SettleReport::Unknown(_)) | None => None,
            })
            .inspect_err(|refusal| self.handle.note_refused(self.command, *refusal))
    }
}

impl Drop for OpenSend {
    fn drop(&mut self) {
        if !self.resolved {
            self.handle
                .mark_unknown(self.physical, UnknownReason::Cancelled);
        }
    }
}

/// The bytes of one request under a wallet, fixed for all its tries, with what they are for. They
/// are serialized once, after everything a body gets has been added to it, so the digest on disk is
/// of what leaves. A request of a final-repair session is a final repair, whatever its client's
/// role, and holds that session's marker for every try.
#[derive(Debug)]
pub(crate) struct BudgetedBody {
    handle: Arc<RunBudgetHandle>,
    /// The command this request is of, as it was when the request was made.
    command: u64,
    bytes: Vec<u8>,
    purpose: SendPurpose,
    final_repair: Option<FinalRepairMarker>,
}

impl BudgetedBody {
    pub(crate) fn new(
        handle: &Arc<RunBudgetHandle>,
        body: &Value,
        purpose: SendPurpose,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            handle: handle.clone(),
            command: handle.command_now(),
            bytes: serde_json::to_vec(body)?,
            purpose,
            final_repair: None,
        })
    }

    /// The request of the session `final_repair` marks, if any: then it is a final repair.
    pub(crate) fn of_session(mut self, final_repair: Option<FinalRepairMarker>) -> Self {
        if final_repair.is_some() {
            self.purpose = SendPurpose::FinalRepair;
        }
        self.final_repair = final_repair;
        self
    }

    /// One try: its own reservation and start on disk, and a request whose body only the permit can
    /// give. The header is the one `RequestBuilder::json` sets.
    pub(crate) fn begin(
        &self,
        request: RequestBuilder,
    ) -> Result<(RequestBuilder, OpenSend), SendRefusal> {
        let (send, permit) = self
            .handle
            .begin(
                self.command,
                &self.bytes,
                self.purpose,
                self.final_repair.as_ref(),
            )
            .inspect_err(|refusal| self.handle.note_refused(self.command, *refusal))?;
        let request = request
            .header(CONTENT_TYPE, "application/json")
            .body(permit.into_body().to_vec());
        Ok((request, send))
    }
}

/// What ended a send in failure. A body that came and could not be read is a cut one.
const fn reason_of_error(timeout: bool, in_the_body: bool) -> UnknownReason {
    if timeout {
        UnknownReason::Timeout
    } else if in_the_body {
        UnknownReason::BodyTruncated
    } else {
        UnknownReason::SendError
    }
}

fn reason_of(error: &reqwest::Error) -> UnknownReason {
    reason_of_error(error.is_timeout(), error.is_body() || error.is_decode())
}

/// Leaves a send unknown when what came back cannot settle it. A success stays open for its body.
pub(crate) fn note_outcome(
    send: &mut Option<OpenSend>,
    outcome: &Result<Response, reqwest::Error>,
) {
    let reason = match outcome {
        Ok(response) if response.status().is_success() => return,
        Ok(_) => UnknownReason::HttpError,
        Err(error) => reason_of(error),
    };
    if let Some(send) = send.take() {
        send.unknown(reason);
    }
}

pub(crate) fn note_body_error(send: &mut Option<OpenSend>, error: &reqwest::Error) {
    if let Some(send) = send.take() {
        send.unknown(reason_of(error));
    }
}

pub(crate) fn settle_body(send: &mut Option<OpenSend>, text: &str) -> Result<(), SendRefusal> {
    settle_body_identified(send, text).map(|_| ())
}

/// The same, with the response it settled, if it did: what the calls of that response are of.
pub(crate) fn settle_body_identified(
    send: &mut Option<OpenSend>,
    text: &str,
) -> Result<Option<ResponseIdentity>, SendRefusal> {
    match send.take() {
        Some(send) => send.settle(text),
        None => Ok(None),
    }
}

/// The client of a budgeted run. A redirect is another send the wallet would never see, with the
/// body going to another URL, so it is not followed, and its response is an HTTP error like any
/// other. The timeouts and the pool are those of the shared client, which stays as it is.
pub(crate) fn no_redirect_client(timeout_secs: u64) -> reqwest::Result<Client> {
    let timeout = Duration::from_secs(timeout_secs);
    Client::builder()
        .connect_timeout(timeout)
        .timeout(timeout)
        .pool_max_idle_per_host(0)
        .redirect(redirect::Policy::none())
        .build()
}

/// One response's usage as it was sent, with nothing filled in.
#[derive(Debug, Default, PartialEq)]
struct ReadUsage {
    cost: Option<BilledCostUsd>,
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    cached_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
}

impl ReadUsage {
    fn reported(&self) -> ReportedUsage<'_> {
        ReportedUsage {
            cost: self.cost.as_ref(),
            prompt_tokens: self.prompt_tokens,
            completion_tokens: self.completion_tokens,
            cached_tokens: self.cached_tokens,
            reasoning_tokens: self.reasoning_tokens,
        }
    }
}

/// A count that is there has to be a count. `null` is not there, as a provider sends it for details
/// it does not have.
fn count(object: &serde_json::Map<String, Value>, key: &str) -> Result<Option<u64>, UnknownReason> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or(UnknownReason::UsageInconsistent),
    }
}

fn detail(
    usage: &serde_json::Map<String, Value>,
    details: &str,
    key: &str,
) -> Result<Option<u64>, UnknownReason> {
    match usage.get(details) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(object)) => count(object, key),
        Some(_) => Err(UnknownReason::UsageInconsistent),
    }
}

/// The amount as its raw token, before any number becomes an `f64`.
fn billed_cost(text: &str) -> Option<BilledCostUsd> {
    type Members<'a> = HashMap<String, &'a RawValue>;
    let response: Members<'_> = serde_json::from_str(text).ok()?;
    let usage: Members<'_> = serde_json::from_str(response.get("usage")?.get()).ok()?;
    BilledCostUsd::from_json_token(usage.get("cost")?.get())
}

/// Reads what a 2xx body reports. It is stricter than the reader of the reply: a part that is there
/// and is no count is damage, not something to leave out.
fn read_usage(text: &str) -> Result<ReadUsage, UnknownReason> {
    let response: Value = serde_json::from_str(text).map_err(|_| UnknownReason::ParseFailed)?;
    let usage = response
        .get("usage")
        .and_then(Value::as_object)
        .ok_or(UnknownReason::UsageMissing)?;
    Ok(ReadUsage {
        cost: billed_cost(text),
        prompt_tokens: count(usage, "prompt_tokens")?,
        completion_tokens: count(usage, "completion_tokens")?,
        cached_tokens: detail(usage, "prompt_tokens_details", "cached_tokens")?,
        reasoning_tokens: detail(usage, "completion_tokens_details", "reasoning_tokens")?,
    })
}

/// A refusal as the caller's error, in fixed words.
pub(crate) fn refused(refusal: SendRefusal) -> anyhow::Error {
    anyhow::Error::new(refusal)
}

/// Which limit said no, for a test that reads the error a client gives.
#[cfg(test)]
pub(crate) fn budget_refusal(error: &anyhow::Error) -> Option<super::reservation::BudgetRefusal> {
    match error.downcast_ref::<SendRefusal>() {
        Some(SendRefusal::Wallet(LedgerFailure::Budget(refusal))) => Some(*refusal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::super::settlement::{KnownBilling, UnknownReason};
    use super::{ReadUsage, SendRefusal, read_usage, reason_of_error, refuse_another_workspace};
    use crate::providers::BilledCostUsd;

    #[test]
    fn what_ended_a_send_is_named_in_this_order() {
        assert_eq!(reason_of_error(true, true), UnknownReason::Timeout);
        assert_eq!(reason_of_error(true, false), UnknownReason::Timeout);
        assert_eq!(reason_of_error(false, true), UnknownReason::BodyTruncated);
        assert_eq!(reason_of_error(false, false), UnknownReason::SendError);
    }

    #[test]
    fn a_workspace_is_the_same_only_as_itself_by_its_real_name() {
        let root = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(root.path()).unwrap();
        let (one, other) = (base.join("one"), base.join("other"));
        std::fs::create_dir_all(&one).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let alias = base.join("alias");
        std::os::unix::fs::symlink(&one, &alias).unwrap();
        assert_eq!(refuse_another_workspace(&one, Some(&one)), Ok(()));
        assert_eq!(refuse_another_workspace(&one, Some(&alias)), Ok(()));
        for (wallet, installed) in [(&one, &other), (&other, &one)] {
            assert_eq!(
                refuse_another_workspace(wallet, Some(installed)),
                Err(SendRefusal::SandboxOnAnotherWorkspace)
            );
        }
        assert_eq!(
            refuse_another_workspace(&one, Some(&base.join("gone"))),
            Err(SendRefusal::SandboxOnAnotherWorkspace)
        );
        assert_eq!(
            refuse_another_workspace(&one, None),
            Err(SendRefusal::SandboxNotApplied)
        );
    }

    fn known(text: &str) -> Result<KnownBilling, UnknownReason> {
        let usage: ReadUsage = read_usage(text)?;
        KnownBilling::from_reported(&usage.reported())
    }

    /// Each row is what comes out of a report as far as a settlement would see it.
    #[test]
    fn a_report_is_known_only_when_every_part_is_there_and_is_what_it_should_be() {
        let good = r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0.5}}"#;
        assert!(known(good).is_ok());
        let zero = r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0}}"#;
        assert!(known(zero).is_ok(), "an explicit zero is a value");
        let null_details = r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0,"prompt_tokens_details":null,"completion_tokens_details":null}}"#;
        assert!(known(null_details).is_ok());
        let details = r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":0,"prompt_tokens_details":{"cached_tokens":4},"completion_tokens_details":{"reasoning_tokens":3}}}"#;
        let read = read_usage(details).unwrap();
        assert_eq!(
            (read.cached_tokens, read.reasoning_tokens),
            (Some(4), Some(3))
        );

        use UnknownReason::{CostMissing, ParseFailed, UsageInconsistent, UsageMissing};
        for (name, text, reason) in [
            ("not JSON", "not json 976", ParseFailed),
            ("cut", r#"{"usage":{"prompt_tok"#, ParseFailed),
            ("past a float", r#"{"usage":{"cost":1e400}}"#, ParseFailed),
            ("no usage", r#"{"id":"a"}"#, UsageMissing),
            ("usage null", r#"{"usage":null}"#, UsageMissing),
            ("usage no object", r#"{"usage":3}"#, UsageMissing),
            (
                "no cost",
                r#"{"usage":{"prompt_tokens":10,"completion_tokens":5}}"#,
                CostMissing,
            ),
            (
                "cost null",
                r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":null}}"#,
                CostMissing,
            ),
            (
                "cost in text",
                r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":"0.5"}}"#,
                CostMissing,
            ),
            (
                "cost negative",
                r#"{"usage":{"prompt_tokens":10,"completion_tokens":5,"cost":-1}}"#,
                CostMissing,
            ),
            (
                "no prompt tokens",
                r#"{"usage":{"completion_tokens":5,"cost":0}}"#,
                UsageMissing,
            ),
            (
                "no completion tokens",
                r#"{"usage":{"prompt_tokens":5,"cost":0}}"#,
                UsageMissing,
            ),
            (
                "prompt tokens in text",
                r#"{"usage":{"prompt_tokens":"5","completion_tokens":5,"cost":0}}"#,
                UsageInconsistent,
            ),
            (
                "completion tokens negative",
                r#"{"usage":{"prompt_tokens":5,"completion_tokens":-5,"cost":0}}"#,
                UsageInconsistent,
            ),
            (
                "cached tokens in text",
                r#"{"usage":{"prompt_tokens":5,"completion_tokens":5,"cost":0,"prompt_tokens_details":{"cached_tokens":"1"}}}"#,
                UsageInconsistent,
            ),
            (
                "details no object",
                r#"{"usage":{"prompt_tokens":5,"completion_tokens":5,"cost":0,"completion_tokens_details":1}}"#,
                UsageInconsistent,
            ),
            (
                "cached past the prompt",
                r#"{"usage":{"prompt_tokens":5,"completion_tokens":5,"cost":0,"prompt_tokens_details":{"cached_tokens":6}}}"#,
                UsageInconsistent,
            ),
        ] {
            assert_eq!(known(text).err(), Some(reason), "{name}");
        }
    }

    #[test]
    fn a_read_keeps_the_amount_as_it_was_sent() {
        let text = r#"{"usage":{"prompt_tokens":1,"completion_tokens":1,"cost":1e-400}}"#;
        let read = read_usage(text).unwrap();
        assert_eq!(read.cost, BilledCostUsd::from_json_token("1e-400"));
    }
}
