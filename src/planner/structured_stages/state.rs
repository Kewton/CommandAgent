//! The stage state machine (Issue #975, G4). It holds no file and no process: every input is a
//! typed value and the clock is an argument.
//!
//! A target goes `not_ready → proposed → locally_valid → sealed`, and `unverified` when its
//! bytes changed or cannot be vouched for. `sealed` means only that this target's required
//! checks all passed on its current input revision and the host adopted those bytes.
//!
//! Its generation only goes up, and it goes up whenever a target's bytes or inputs move, so a
//! digest that returns does not bring anything back (S4). A revision a model sends is compared
//! with the host's value and never believed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::contract::{StageContract, TargetKey};
use super::invalidation::invalidate_downstream;
use super::receipt::{
    CheckResult, CheckStatus, DependencyPin, InputDigest, ReceiptParts, StageReceipt, ToolPins,
    is_sha256_hex, sha256_hex,
};
use crate::tools::profile_tools::commit::CommitReceipt;

pub(crate) const SNAPSHOT_SCHEMA: &str = "structured-stage.ledger/v1";
/// What a target's history keeps. It is a record for a reader; no judgment reads it.
pub(crate) const MAX_RECEIPT_HISTORY: usize = 8;
/// The revision before anything was adopted. It carries no content, so a model can start from
/// it, and once anything was adopted it is simply an old revision.
pub(crate) const INITIAL_REVISION: &str = "initial";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Phase {
    NotReady,
    Proposed,
    LocallyValid,
    Sealed,
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnverifiedReason {
    Resumed,
    PinChanged,
    InputChanged,
    BytesChanged,
    DependencyChanged,
    InterruptedCommit,
    CommitNotDurable,
    CancelledAfterCommit,
    CommitOutcomeUnknown,
    /// A commit stood after the compare stopped holding: the target's generation moved, for its
    /// own bytes or from upstream, or what it depends on did.
    ChangedDuringCommit,
    ReceiptRefused,
    LedgerDiverged,
}

/// One started attempt. Only the host makes one, and a result is judged against it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptTicket {
    pub(crate) run_nonce: String,
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) attempt: u32,
    pub(crate) generation: u64,
    pub(crate) turn: u64,
}

impl AttemptTicket {
    fn key(&self) -> TargetKey {
        TargetKey {
            stage: self.stage.clone(),
            target: self.target.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum InFlightProgress {
    Started,
    LocallyValid {
        candidate_sha256: String,
        checks: Vec<CheckResult>,
    },
    IntentRecorded {
        candidate_sha256: String,
        /// What the output held when the intent was written. `None`: nothing was there.
        previous_sha256: Option<String>,
        checks: Vec<CheckResult>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InFlight {
    pub(crate) ticket: AttemptTicket,
    pub(crate) pins: ToolPins,
    pub(crate) schema_sha256: String,
    pub(crate) input_revision: String,
    pub(crate) inputs: Vec<InputDigest>,
    pub(crate) dependencies: Vec<DependencyPin>,
    pub(crate) arguments_sha256: String,
    pub(crate) request_id: String,
    pub(crate) progress: InFlightProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetRecord {
    pub(crate) key: TargetKey,
    pub(crate) phase: Phase,
    pub(crate) generation: u64,
    pub(crate) attempts_used: u32,
    pub(crate) unverified_reason: Option<UnverifiedReason>,
    pub(crate) current_receipt: Option<StageReceipt>,
    pub(crate) history: Vec<StageReceipt>,
    pub(crate) in_flight: Option<InFlight>,
    /// Attempt to the digest of what it reported: the host's idempotency key is the attempt.
    pub(crate) reported: BTreeMap<u32, String>,
    /// The fixed inputs as first adopted. A change is refused, not adopted.
    pub(crate) fixed_inputs: Option<Vec<InputDigest>>,
}

impl TargetRecord {
    pub(crate) fn retire_current_receipt(&mut self) {
        if let Some(receipt) = self.current_receipt.take() {
            self.push_history(receipt);
        }
    }

    pub(crate) fn push_history(&mut self, receipt: StageReceipt) {
        self.history.push(receipt);
        if self.history.len() > MAX_RECEIPT_HISTORY {
            self.history.remove(0);
        }
    }

    /// The state only. An attempt under way is the caller's to judge.
    pub(crate) fn lower_to_unverified(&mut self, reason: UnverifiedReason) {
        self.retire_current_receipt();
        self.phase = Phase::Unverified;
        self.unverified_reason = Some(reason);
    }

    /// True once a rename may have happened. Only `record_commit` ends such an attempt in a live
    /// process, so that nothing forgets it (S15). A new process judges it by the bytes.
    pub(crate) fn has_recorded_intent(&self) -> bool {
        matches!(
            self.in_flight.as_ref().map(|in_flight| &in_flight.progress),
            Some(InFlightProgress::IntentRecorded { .. })
        )
    }

    /// Ends an attempt that has not reached its intent. It can no longer finish.
    pub(crate) fn drop_unless_intent(&mut self) {
        if !self.has_recorded_intent() {
            self.in_flight = None;
        }
    }

    fn drop_in_flight(&mut self) {
        self.in_flight = None;
        if matches!(self.phase, Phase::LocallyValid) {
            self.phase = Phase::Proposed;
        }
    }
}

/// Everything a ledger saves. A value read back goes through [`super::resume`] before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MachineSnapshot {
    pub(crate) schema: String,
    pub(crate) contract_sha256: String,
    pub(crate) run_nonce: String,
    pub(crate) process_nonce: String,
    pub(crate) started_at_ms: u64,
    /// The latest clock seen. A clock that goes back cannot give the deadline time back.
    pub(crate) latest_clock_ms: u64,
    pub(crate) attempts_used: u32,
    pub(crate) last_commit_turn: Option<u64>,
    pub(crate) targets: Vec<TargetRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ObservedBytes {
    Present(Vec<u8>),
    Absent,
    Unreadable,
}

impl ObservedBytes {
    pub(crate) fn sha256(&self) -> Option<String> {
        match self {
            Self::Present(bytes) => Some(sha256_hex(bytes)),
            Self::Absent | Self::Unreadable => None,
        }
    }
}

/// Reads what is at a workspace-relative path now, by the bytes.
pub(crate) trait BytesSource {
    fn observe(&self, path: &str) -> ObservedBytes;
}

/// What one tool call asks for. `expected_revision` is the model's and is only compared.
#[derive(Debug, Clone)]
pub(crate) struct AttemptRequest<'a> {
    pub(crate) turn: u64,
    pub(crate) now_ms: u64,
    pub(crate) expected_revision: &'a str,
    pub(crate) pins: ToolPins,
    pub(crate) schema_sha256: String,
    pub(crate) input_revision: String,
    pub(crate) inputs: Vec<InputDigest>,
    pub(crate) arguments_sha256: String,
    /// The host's ID for this call, which its commit receipt must repeat.
    pub(crate) request_id: String,
    pub(crate) current_output: &'a ObservedBytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExhaustedLimit {
    Target,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BeginDecision {
    /// Start the process. The attempt is already counted.
    Proceed(AttemptTicket),
    /// The first row of the table: nothing changes, no process, no progress.
    NoChange {
        revision: String,
    },
    Stale {
        current_revision: String,
    },
    UnknownRevision,
    UnknownTarget,
    NotReady {
        waiting_on: Vec<TargetKey>,
    },
    InputNotCurrent {
        path: String,
    },
    FixedInputChanged {
        path: String,
    },
    AttemptsExhausted(ExhaustedLimit),
    DeadlineExceeded,
    /// The last attempt on this target wrote its intent and has not been told how it ended.
    Refused(StageRefusal),
}

impl BeginDecision {
    pub(crate) fn starts_process(&self) -> bool {
        matches!(self, Self::Proceed(_))
    }

    /// A call that moved nothing weighs on the existing stagnation limit.
    pub(crate) fn counts_for_stagnation(&self) -> bool {
        !self.starts_process()
    }

    /// The limits, a changed fixed input, and an unfinished intent end the run by themselves: no
    /// later call can get past any of them.
    pub(crate) fn ends_run(&self) -> bool {
        matches!(
            self,
            Self::AttemptsExhausted(_)
                | Self::DeadlineExceeded
                | Self::FixedInputChanged { .. }
                | Self::Refused(_)
        )
    }
}

/// A check outcome a model may answer with another submission. The type decides this: no
/// process text is ever classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckFailureKind {
    Failed,
    NotRun,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalFailure {
    /// Stays terminal: a retry would turn the comparison into an oracle for the value.
    SecretValueInOutput,
    Binding,
    Pin,
    Process,
    Path,
    Cancelled,
    /// A commit stopped before any intent was written, so no rename can have happened. After an
    /// intent the only report is a [`CommitObservation`].
    CommitBeforeIntent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttemptFailure {
    Check(CheckFailureKind),
    Terminal(TerminalFailure),
}

impl AttemptFailure {
    pub(crate) const fn is_resubmittable(self) -> bool {
        matches!(self, Self::Check(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StageRefusal {
    OtherRun,
    UnknownTarget,
    Stale,
    OutOfOrder,
    MalformedDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalDecision {
    /// The result holds. Next: the CAS, the intent, and the rename.
    ReadyToCommit,
    /// The same bytes on a sealed target: no second commit, and no progress.
    Unchanged,
    Resubmit(CheckFailureKind),
    /// The same attempt reported the same thing again.
    Replay,
    /// The same attempt reported something else.
    Conflict,
    Refused(StageRefusal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FailureDecision {
    Resubmit(CheckFailureKind),
    EndRun(TerminalFailure),
    Refused(StageRefusal),
}

/// What the commit path says happened. The receipt is what proves a commit stood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommitObservation {
    Stood(Box<CommitReceipt>),
    /// The failure came before the rename: the old bytes are still there.
    NotAdopted,
    /// The rename may have happened. Treated as if it did.
    OutcomeUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommitDecision {
    Sealed {
        revision: String,
        receipt_sha256: String,
        invalidated: Vec<TargetKey>,
    },
    /// The bytes moved, but nothing vouches for them.
    StoodUnverified {
        revision: String,
        reason: UnverifiedReason,
        invalidated: Vec<TargetKey>,
    },
    NotAdopted,
}

impl CommitDecision {
    pub(crate) fn is_progress(&self) -> bool {
        matches!(self, Self::Sealed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObligationStatus {
    pub(crate) key: TargetKey,
    pub(crate) phase: Phase,
    pub(crate) generation: u64,
    pub(crate) unverified_reason: Option<UnverifiedReason>,
    pub(crate) receipt_sha256: Option<String>,
}

/// The one answer to "are the obligations met". `complete` says that and nothing wider: it is
/// an input a final gate can only be lowered by.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageVerdict {
    complete: bool,
    obligations: Vec<ObligationStatus>,
    invalidated: Vec<TargetKey>,
    ledger_intact: bool,
}

impl StageVerdict {
    pub(crate) fn complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn obligations(&self) -> &[ObligationStatus] {
        &self.obligations
    }

    pub(crate) fn invalidated(&self) -> &[TargetKey] {
        &self.invalidated
    }

    pub(crate) fn ledger_intact(&self) -> bool {
        self.ledger_intact
    }
}

/// The read-only identity a later budget can attach to a cost (G5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptIdentity {
    pub(crate) run_nonce: String,
    pub(crate) stage: String,
    pub(crate) target: String,
    pub(crate) attempt: u32,
    pub(crate) generation: u64,
}

/// Exactly the required IDs, once each, all passed. A missing one was not run, and one the
/// operator did not ask for is unknown.
pub(crate) fn judge_checks(
    required: &BTreeSet<String>,
    results: &[CheckResult],
) -> Result<(), CheckFailureKind> {
    let mut seen = BTreeSet::new();
    let mut outcome = None;
    let mut note = |kind: CheckFailureKind| {
        outcome = Some(match (outcome, kind) {
            (Some(CheckFailureKind::Failed), _) | (_, CheckFailureKind::Failed) => {
                CheckFailureKind::Failed
            }
            (Some(CheckFailureKind::Unknown), _) | (_, CheckFailureKind::Unknown) => {
                CheckFailureKind::Unknown
            }
            _ => CheckFailureKind::NotRun,
        });
    };
    for result in results {
        if !required.contains(&result.id) || !seen.insert(result.id.as_str()) {
            note(CheckFailureKind::Unknown);
            continue;
        }
        match result.status {
            CheckStatus::Passed => {}
            CheckStatus::Failed => note(CheckFailureKind::Failed),
            CheckStatus::NotRun => note(CheckFailureKind::NotRun),
            CheckStatus::Unknown => note(CheckFailureKind::Unknown),
        }
    }
    if required.iter().any(|id| !seen.contains(id.as_str())) {
        note(CheckFailureKind::NotRun);
    }
    match outcome {
        Some(kind) => Err(kind),
        None => Ok(()),
    }
}

fn revision_token(run_nonce: &str, key: &TargetKey, generation: u64) -> String {
    if generation == 0 {
        return INITIAL_REVISION.to_string();
    }
    let material = format!("{run_nonce}\0{}\0{}\0{generation}", key.stage, key.target);
    format!("g{generation}-{}", &sha256_hex(material.as_bytes())[..16])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RevisionClass {
    Current,
    Older,
    Unknown,
}

/// A token of another run or target does not recompute to the same value, so it is unknown.
fn classify_revision(
    expected: &str,
    run_nonce: &str,
    key: &TargetKey,
    generation: u64,
) -> RevisionClass {
    if expected == revision_token(run_nonce, key, generation) {
        return RevisionClass::Current;
    }
    let claimed = expected
        .strip_prefix('g')
        .and_then(|rest| rest.split_once('-'))
        .and_then(|(number, _)| number.parse::<u64>().ok())
        .unwrap_or(0);
    if expected == INITIAL_REVISION
        || (claimed > 0
            && claimed < generation
            && expected == revision_token(run_nonce, key, claimed))
    {
        return RevisionClass::Older;
    }
    RevisionClass::Unknown
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StageMachine {
    contract: Arc<StageContract>,
    snapshot: MachineSnapshot,
}

impl StageMachine {
    pub(crate) fn new(
        contract: Arc<StageContract>,
        run_nonce: String,
        process_nonce: String,
        now_ms: u64,
    ) -> Self {
        let targets = contract
            .targets()
            .map(|target| TargetRecord {
                key: target.key.clone(),
                phase: Phase::NotReady,
                generation: 0,
                attempts_used: 0,
                unverified_reason: None,
                current_receipt: None,
                history: Vec::new(),
                in_flight: None,
                reported: BTreeMap::new(),
                fixed_inputs: None,
            })
            .collect();
        let snapshot = MachineSnapshot {
            schema: SNAPSHOT_SCHEMA.to_string(),
            contract_sha256: contract.sha256(),
            run_nonce,
            process_nonce,
            started_at_ms: now_ms,
            latest_clock_ms: now_ms,
            attempts_used: 0,
            last_commit_turn: None,
            targets,
        };
        Self { contract, snapshot }
    }

    /// Only [`super::resume`] and a ledger's divergence check build one from a saved value.
    pub(super) fn from_lowered_snapshot(
        contract: Arc<StageContract>,
        snapshot: MachineSnapshot,
    ) -> Self {
        Self { contract, snapshot }
    }

    pub(crate) fn contract(&self) -> &Arc<StageContract> {
        &self.contract
    }

    pub(super) fn snapshot(&self) -> &MachineSnapshot {
        &self.snapshot
    }

    pub(super) fn snapshot_mut(&mut self) -> &mut MachineSnapshot {
        &mut self.snapshot
    }

    fn index_of(&self, key: &TargetKey) -> Option<usize> {
        self.snapshot
            .targets
            .iter()
            .position(|record| &record.key == key)
    }

    pub(super) fn record(&self, key: &TargetKey) -> Option<&TargetRecord> {
        self.index_of(key)
            .map(|index| &self.snapshot.targets[index])
    }

    pub(crate) fn revision_of(&self, key: &TargetKey) -> Option<String> {
        let record = self.record(key)?;
        Some(revision_token(
            &self.snapshot.run_nonce,
            key,
            record.generation,
        ))
    }

    /// The latest attempt of a target, read-only. `None` until one started. An attempt under
    /// way answers with the generation it started on.
    pub(crate) fn attempt_identity(&self, key: &TargetKey) -> Option<AttemptIdentity> {
        let record = self.record(key)?;
        if record.attempts_used == 0 {
            return None;
        }
        let generation = record
            .in_flight
            .as_ref()
            .map_or(record.generation, |in_flight| in_flight.ticket.generation);
        Some(AttemptIdentity {
            run_nonce: self.snapshot.run_nonce.clone(),
            stage: key.stage.clone(),
            target: key.target.clone(),
            attempt: record.attempts_used,
            generation,
        })
    }

    fn deadline_passed(&self) -> bool {
        let elapsed = self
            .snapshot
            .latest_clock_ms
            .saturating_sub(self.snapshot.started_at_ms);
        elapsed >= self.contract.limits().deadline_seconds.saturating_mul(1000)
    }

    /// The current versions of everything a stage depends on, or who is not sealed.
    fn current_dependencies(&self, stage: &str) -> Result<Vec<DependencyPin>, Vec<TargetKey>> {
        let mut pins = Vec::new();
        let mut waiting = Vec::new();
        let upstream = self
            .contract
            .stage(stage)
            .map(|admitted| admitted.depends_on.clone())
            .unwrap_or_default();
        for record in &self.snapshot.targets {
            if !upstream.contains(&record.key.stage) {
                continue;
            }
            match (&record.phase, &record.current_receipt) {
                (Phase::Sealed, Some(receipt)) => pins.push(DependencyPin {
                    stage: record.key.stage.clone(),
                    target: record.key.target.clone(),
                    generation: record.generation,
                    committed_sha256: receipt.committed_sha256.clone(),
                }),
                _ => waiting.push(record.key.clone()),
            }
        }
        if waiting.is_empty() {
            Ok(pins)
        } else {
            Err(waiting)
        }
    }

    /// Every condition of the table's first row. Anything short of this goes through the whole
    /// procedure, so "nothing changed" cannot stand in for a check that did not run (S7).
    fn is_unchanged_submission(&self, record: &TargetRecord, request: &AttemptRequest<'_>) -> bool {
        let Some(receipt) = &record.current_receipt else {
            return false;
        };
        let mut inputs = request.inputs.clone();
        inputs.sort();
        record.phase == Phase::Sealed
            && receipt.generation == record.generation
            && receipt.run_nonce == self.snapshot.run_nonce
            && receipt.process_nonce == self.snapshot.process_nonce
            && receipt.arguments_sha256 == request.arguments_sha256
            && receipt.pins == request.pins
            && receipt.schema_sha256 == request.schema_sha256
            && receipt.input_revision == request.input_revision
            && receipt.inputs == inputs
            && request.current_output.sha256().as_deref() == Some(receipt.committed_sha256.as_str())
            && self
                .current_dependencies(&record.key.stage)
                .is_ok_and(|mut pins| {
                    pins.sort();
                    pins == receipt.dependencies
                })
    }

    /// An upstream input must be what its stage adopted; a fixed input what was first adopted.
    /// Returns the digests of the fixed ones, to pin when the attempt proceeds.
    fn judge_inputs(
        &self,
        key: &TargetKey,
        record: &TargetRecord,
        inputs: &[InputDigest],
    ) -> Result<Vec<InputDigest>, BeginDecision> {
        let declared = self
            .contract
            .target(key)
            .map(|target| target.inputs.as_slice())
            .unwrap_or_default();
        let mut fixed = Vec::new();
        for input in declared {
            let not_current = || BeginDecision::InputNotCurrent {
                path: input.path.clone(),
            };
            let given = inputs
                .iter()
                .find(|given| given.path == input.path)
                .ok_or_else(not_current)?;
            match &input.upstream {
                Some(upstream) => {
                    let adopted = self
                        .record(upstream)
                        .and_then(|record| record.current_receipt.as_ref())
                        .map(|receipt| receipt.committed_sha256.as_str());
                    if adopted != Some(given.sha256.as_str()) {
                        return Err(not_current());
                    }
                }
                None => fixed.push(given.clone()),
            }
        }
        if inputs.len() != declared.len() {
            return Err(BeginDecision::InputNotCurrent {
                path: String::new(),
            });
        }
        fixed.sort();
        if let Some(pinned) = &record.fixed_inputs
            && let Some(changed) = fixed.iter().find(|given| !pinned.contains(given))
        {
            return Err(BeginDecision::FixedInputChanged {
                path: changed.path.clone(),
            });
        }
        Ok(fixed)
    }

    /// The D7 table, then the limits. Only `Proceed` counts an attempt.
    pub(crate) fn begin(&mut self, key: &TargetKey, request: AttemptRequest<'_>) -> BeginDecision {
        let Some(index) = self.index_of(key) else {
            return BeginDecision::UnknownTarget;
        };
        // Before anything is recorded: this call changes nothing about an unfinished intent.
        if self.snapshot.targets[index].has_recorded_intent() {
            return BeginDecision::Refused(StageRefusal::OutOfOrder);
        }
        self.snapshot.latest_clock_ms = self.snapshot.latest_clock_ms.max(request.now_ms);
        if self.deadline_passed() {
            return BeginDecision::DeadlineExceeded;
        }
        let record = &self.snapshot.targets[index];
        let current_revision = revision_token(&self.snapshot.run_nonce, key, record.generation);
        // S17: the input this call was written against moved earlier in the same reply.
        if self.snapshot.last_commit_turn == Some(request.turn) {
            return BeginDecision::Stale { current_revision };
        }
        match classify_revision(
            request.expected_revision,
            &self.snapshot.run_nonce,
            key,
            record.generation,
        ) {
            RevisionClass::Current => {}
            RevisionClass::Older => return BeginDecision::Stale { current_revision },
            RevisionClass::Unknown => return BeginDecision::UnknownRevision,
        }
        if self.is_unchanged_submission(record, &request) {
            return BeginDecision::NoChange {
                revision: current_revision,
            };
        }
        let dependencies = match self.current_dependencies(&key.stage) {
            Ok(pins) => pins,
            Err(waiting_on) => return BeginDecision::NotReady { waiting_on },
        };
        let fixed = match self.judge_inputs(key, record, &request.inputs) {
            Ok(fixed) => fixed,
            Err(refusal) => return refusal,
        };
        let limits = self.contract.limits();
        if record.attempts_used >= limits.attempts_per_target {
            return BeginDecision::AttemptsExhausted(ExhaustedLimit::Target);
        }
        if self.snapshot.attempts_used >= limits.attempts_per_run {
            return BeginDecision::AttemptsExhausted(ExhaustedLimit::Run);
        }

        self.snapshot.attempts_used += 1;
        let run_nonce = self.snapshot.run_nonce.clone();
        let record = &mut self.snapshot.targets[index];
        record.attempts_used += 1;
        record.fixed_inputs.get_or_insert(fixed);
        if record.phase != Phase::Sealed {
            record.phase = Phase::Proposed;
        }
        let ticket = AttemptTicket {
            run_nonce,
            stage: key.stage.clone(),
            target: key.target.clone(),
            attempt: record.attempts_used,
            generation: record.generation,
            turn: request.turn,
        };
        record.in_flight = Some(InFlight {
            ticket: ticket.clone(),
            pins: request.pins,
            schema_sha256: request.schema_sha256,
            input_revision: request.input_revision,
            inputs: request.inputs,
            dependencies,
            arguments_sha256: request.arguments_sha256,
            request_id: request.request_id,
            progress: InFlightProgress::Started,
        });
        BeginDecision::Proceed(ticket)
    }

    fn ticket_index(&self, ticket: &AttemptTicket) -> Result<usize, StageRefusal> {
        if ticket.run_nonce != self.snapshot.run_nonce {
            return Err(StageRefusal::OtherRun);
        }
        self.index_of(&ticket.key())
            .ok_or(StageRefusal::UnknownTarget)
    }

    fn is_in_flight(record: &TargetRecord, ticket: &AttemptTicket) -> bool {
        record
            .in_flight
            .as_ref()
            .is_some_and(|in_flight| &in_flight.ticket == ticket)
    }

    /// The compare of the compare-and-swap. Run right before the rename too.
    pub(crate) fn cas_check(&self, ticket: &AttemptTicket) -> Result<(), StageRefusal> {
        let index = self.ticket_index(ticket)?;
        let record = &self.snapshot.targets[index];
        let in_flight = record
            .in_flight
            .as_ref()
            .filter(|in_flight| &in_flight.ticket == ticket)
            .ok_or(StageRefusal::Stale)?;
        if record.generation != ticket.generation {
            return Err(StageRefusal::Stale);
        }
        let mut current = self
            .current_dependencies(&ticket.stage)
            .map_err(|_| StageRefusal::Stale)?;
        current.sort();
        let mut pinned = in_flight.dependencies.clone();
        pinned.sort();
        if current != pinned {
            return Err(StageRefusal::Stale);
        }
        Ok(())
    }

    /// The bounded process's result, judged by required ID. A later report of the same attempt
    /// is a replay or a conflict, by its content.
    pub(crate) fn record_local_result(
        &mut self,
        ticket: &AttemptTicket,
        candidate_sha256: &str,
        checks: Vec<CheckResult>,
        current_output: &ObservedBytes,
    ) -> LocalDecision {
        let index = match self.ticket_index(ticket) {
            Ok(index) => index,
            Err(refusal) => return LocalDecision::Refused(refusal),
        };
        if !is_sha256_hex(candidate_sha256) {
            return LocalDecision::Refused(StageRefusal::MalformedDigest);
        }
        let content = sha256_hex(
            format!(
                "{candidate_sha256}\0{}",
                serde_json::to_string(&checks).unwrap_or_default()
            )
            .as_bytes(),
        );
        let record = &self.snapshot.targets[index];
        if let Some(reported) = record.reported.get(&ticket.attempt) {
            return if reported == &content {
                LocalDecision::Replay
            } else {
                LocalDecision::Conflict
            };
        }
        if !Self::is_in_flight(record, ticket) {
            return LocalDecision::Refused(StageRefusal::Stale);
        }
        let verdict = self
            .contract
            .target(&ticket.key())
            .map_or(Err(CheckFailureKind::Unknown), |target| {
                judge_checks(&target.required_checks, &checks)
            });
        let record = &mut self.snapshot.targets[index];
        record.reported.insert(ticket.attempt, content);
        if let Err(kind) = verdict {
            record.drop_in_flight();
            return LocalDecision::Resubmit(kind);
        }
        let same_as_sealed = record.phase == Phase::Sealed
            && record.current_receipt.as_ref().is_some_and(|receipt| {
                receipt.committed_sha256 == candidate_sha256
                    && current_output.sha256().as_deref() == Some(candidate_sha256)
            });
        if same_as_sealed {
            record.drop_in_flight();
            return LocalDecision::Unchanged;
        }
        if record.phase != Phase::Sealed {
            record.phase = Phase::LocallyValid;
        }
        if let Some(in_flight) = &mut record.in_flight {
            in_flight.progress = InFlightProgress::LocallyValid {
                candidate_sha256: candidate_sha256.to_string(),
                checks,
            };
        }
        LocalDecision::ReadyToCommit
    }

    /// A failure that came typed from the process or result boundary.
    pub(crate) fn record_failure(
        &mut self,
        ticket: &AttemptTicket,
        failure: AttemptFailure,
    ) -> FailureDecision {
        let index = match self.ticket_index(ticket) {
            Ok(index) => index,
            Err(refusal) => return FailureDecision::Refused(refusal),
        };
        let record = &mut self.snapshot.targets[index];
        if !Self::is_in_flight(record, ticket) {
            return FailureDecision::Refused(StageRefusal::Stale);
        }
        if record.has_recorded_intent() {
            return FailureDecision::Refused(StageRefusal::OutOfOrder);
        }
        record.drop_in_flight();
        match failure {
            AttemptFailure::Check(kind) => FailureDecision::Resubmit(kind),
            AttemptFailure::Terminal(kind) => FailureDecision::EndRun(kind),
        }
    }

    /// Must be durable before the rename: it is how a later process learns one may have
    /// happened (S15).
    pub(crate) fn record_intent(
        &mut self,
        ticket: &AttemptTicket,
        previous_sha256: Option<String>,
    ) -> Result<(), StageRefusal> {
        self.cas_check(ticket)?;
        let index = self.ticket_index(ticket)?;
        let in_flight = self.snapshot.targets[index]
            .in_flight
            .as_mut()
            .ok_or(StageRefusal::Stale)?;
        let InFlightProgress::LocallyValid {
            candidate_sha256,
            checks,
        } = &in_flight.progress
        else {
            return Err(StageRefusal::OutOfOrder);
        };
        in_flight.progress = InFlightProgress::IntentRecorded {
            candidate_sha256: candidate_sha256.clone(),
            previous_sha256,
            checks: checks.clone(),
        };
        Ok(())
    }

    /// The commit path's answer. Once a rename may have happened, the bytes are new whatever
    /// else went wrong: the generation moves and everything below loses its state.
    pub(crate) fn record_commit(
        &mut self,
        ticket: &AttemptTicket,
        observation: CommitObservation,
    ) -> Result<CommitDecision, StageRefusal> {
        let index = self.ticket_index(ticket)?;
        let record = &self.snapshot.targets[index];
        if !Self::is_in_flight(record, ticket) {
            return Err(StageRefusal::Stale);
        }
        let compare_held = self.cas_check(ticket).is_ok();
        let record = &mut self.snapshot.targets[index];
        let Some(in_flight) = record.in_flight.take() else {
            return Err(StageRefusal::Stale);
        };
        let InFlightProgress::IntentRecorded {
            candidate_sha256,
            checks,
            ..
        } = in_flight.progress.clone()
        else {
            record.in_flight = Some(in_flight);
            return Err(StageRefusal::OutOfOrder);
        };
        let commit = match observation {
            CommitObservation::NotAdopted => {
                record.drop_in_flight();
                return Ok(CommitDecision::NotAdopted);
            }
            CommitObservation::OutcomeUnknown => None,
            CommitObservation::Stood(commit) => Some(*commit),
        };
        record.generation += 1;
        record.retire_current_receipt();
        let generation = record.generation;
        let bound = commit.map(|commit| {
            StageReceipt::bind(
                ReceiptParts {
                    run_nonce: ticket.run_nonce.clone(),
                    process_nonce: self.snapshot.process_nonce.clone(),
                    stage: ticket.stage.clone(),
                    target: ticket.target.clone(),
                    output_path: self
                        .contract
                        .target(&ticket.key())
                        .map(|target| target.output_path.clone())
                        .unwrap_or_default(),
                    attempt: ticket.attempt,
                    generation,
                    pins: in_flight.pins,
                    schema_sha256: in_flight.schema_sha256,
                    input_revision: in_flight.input_revision,
                    inputs: in_flight.inputs,
                    dependencies: in_flight.dependencies,
                    arguments_sha256: in_flight.arguments_sha256,
                    request_id: in_flight.request_id,
                    candidate_sha256,
                    checks,
                },
                commit,
            )
        });
        let reason = match &bound {
            None => Some(UnverifiedReason::CommitOutcomeUnknown),
            Some(Err(_)) => Some(UnverifiedReason::ReceiptRefused),
            Some(Ok(receipt)) => {
                let commit = receipt.commit.as_ref();
                if commit.is_some_and(|commit| commit.cancelled_after_commit) {
                    Some(UnverifiedReason::CancelledAfterCommit)
                } else if !commit.is_some_and(|commit| commit.durable) {
                    Some(UnverifiedReason::CommitNotDurable)
                } else if !compare_held {
                    Some(UnverifiedReason::ChangedDuringCommit)
                } else {
                    None
                }
            }
        };
        let receipt = bound.and_then(Result::ok);
        let receipt_sha256 = receipt.as_ref().map(StageReceipt::sha256);
        match (reason, receipt) {
            (None, Some(receipt)) => {
                record.current_receipt = Some(receipt);
                record.phase = Phase::Sealed;
                record.unverified_reason = None;
            }
            (reason, receipt) => {
                if let Some(receipt) = receipt {
                    record.push_history(receipt);
                }
                record.lower_to_unverified(reason.unwrap_or(UnverifiedReason::ReceiptRefused));
            }
        }
        let sealed = record.phase == Phase::Sealed;
        let unverified_reason = record.unverified_reason;
        self.snapshot.last_commit_turn = Some(ticket.turn);
        let invalidated =
            invalidate_downstream(&self.contract, &mut self.snapshot.targets, &ticket.stage);
        let revision = revision_token(&ticket.run_nonce, &ticket.key(), generation);
        if sealed && let Some(receipt_sha256) = receipt_sha256 {
            return Ok(CommitDecision::Sealed {
                revision,
                receipt_sha256,
                invalidated,
            });
        }
        Ok(CommitDecision::StoodUnverified {
            revision,
            reason: unverified_reason.unwrap_or(UnverifiedReason::ReceiptRefused),
            invalidated,
        })
    }

    fn why_no_longer_sealed(
        &self,
        record: &TargetRecord,
        output_path: &str,
        bytes: &dyn BytesSource,
    ) -> Option<UnverifiedReason> {
        let Some(receipt) = &record.current_receipt else {
            return Some(UnverifiedReason::ReceiptRefused);
        };
        if bytes.observe(output_path).sha256().as_deref() != Some(receipt.committed_sha256.as_str())
        {
            return Some(UnverifiedReason::BytesChanged);
        }
        let input_moved = receipt.inputs.iter().any(|input| {
            bytes.observe(&input.path).sha256().as_deref() != Some(input.sha256.as_str())
        });
        if input_moved {
            return Some(UnverifiedReason::InputChanged);
        }
        let dependencies = self
            .current_dependencies(&record.key.stage)
            .map(|mut pins| {
                pins.sort();
                pins
            });
        if dependencies.as_ref() != Ok(&receipt.dependencies) {
            return Some(UnverifiedReason::DependencyChanged);
        }
        None
    }

    /// The judgment behind `StageLedger::verdict`. Reads the bytes, never a timestamp. A sealed
    /// target whose bytes moved loses its state, and so does everything below it, for good: a
    /// later restore of the same bytes brings nothing back.
    pub(super) fn judge(&mut self, bytes: &dyn BytesSource) -> StageVerdict {
        let mut invalidated = Vec::new();
        for index in 0..self.snapshot.targets.len() {
            let record = &self.snapshot.targets[index];
            if record.phase != Phase::Sealed {
                continue;
            }
            let output_path = self
                .contract
                .target(&record.key)
                .map(|target| target.output_path.clone())
                .unwrap_or_default();
            let Some(reason) = self.why_no_longer_sealed(record, &output_path, bytes) else {
                continue;
            };
            let record = &mut self.snapshot.targets[index];
            record.generation += 1;
            record.drop_unless_intent();
            record.lower_to_unverified(reason);
            let key = record.key.clone();
            invalidated.push(key.clone());
            invalidated.extend(invalidate_downstream(
                &self.contract,
                &mut self.snapshot.targets,
                &key.stage,
            ));
        }
        let obligations = self.obligations();
        // Bytes that are about to move, or have, are not a finished stage set, and a call that
        // forgot `record_commit` should not pass for one.
        let intent_open = self
            .snapshot
            .targets
            .iter()
            .any(TargetRecord::has_recorded_intent);
        StageVerdict {
            complete: !intent_open
                && !obligations.is_empty()
                && obligations
                    .iter()
                    .all(|status| status.phase == Phase::Sealed && status.receipt_sha256.is_some()),
            obligations,
            invalidated,
            ledger_intact: true,
        }
    }

    /// The answer when the ledger behind this state cannot be vouched for. It is the only other
    /// way to get a verdict, and it can only say no.
    pub(super) fn verdict_without_ledger(&self) -> StageVerdict {
        StageVerdict {
            complete: false,
            obligations: self.obligations(),
            invalidated: Vec::new(),
            ledger_intact: false,
        }
    }

    fn obligations(&self) -> Vec<ObligationStatus> {
        self.snapshot
            .targets
            .iter()
            .map(|record| ObligationStatus {
                key: record.key.clone(),
                phase: record.phase,
                generation: record.generation,
                unverified_reason: record.unverified_reason,
                receipt_sha256: record.current_receipt.as_ref().map(StageReceipt::sha256),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use super::super::contract::{
        NarrowedStageLimits, StageContract, StageSetDeclaration, TargetDeclaration, TargetKey,
    };
    use super::super::receipt::{CheckResult, CheckStatus, StageReceipt, is_sha256_hex};
    use super::super::test_support::{
        FakeBytes, admit_enforced as admit, commit_for, digest, document_index_contract, facts,
        key, one_target, passed_checks, request_for, seal, setting_object_contract, sha_of,
        two_stage_contract,
    };
    use super::{
        AttemptFailure, AttemptRequest, AttemptTicket, BeginDecision, CheckFailureKind,
        CommitDecision, CommitObservation, ExhaustedLimit, FailureDecision, INITIAL_REVISION,
        LocalDecision, MAX_RECEIPT_HISTORY, ObservedBytes, Phase, StageMachine, StageRefusal,
        TerminalFailure, UnverifiedReason, judge_checks,
    };

    const NOW: u64 = 1_000_000;

    fn machine(contract: StageContract) -> StageMachine {
        StageMachine::new(
            Arc::new(contract),
            "run-a".to_string(),
            "process-a".to_string(),
            NOW,
        )
    }

    fn alpha() -> TargetKey {
        key("alpha", "main")
    }

    fn beta() -> TargetKey {
        key("beta", "main")
    }

    fn proceed(decision: BeginDecision) -> AttemptTicket {
        match decision {
            BeginDecision::Proceed(ticket) => ticket,
            other => panic!("expected to proceed, got {other:?}"),
        }
    }

    /// `alpha` sealed, then `beta` on top of it.
    fn both_sealed() -> (StageMachine, FakeBytes) {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(
            &mut machine,
            &alpha(),
            b"alpha v1",
            &mut bytes,
            1,
            NOW,
            "args-a",
        );
        seal(
            &mut machine,
            &beta(),
            b"beta v1",
            &mut bytes,
            2,
            NOW,
            "args-b",
        );
        (machine, bytes)
    }

    fn phase(machine: &StageMachine, target: &TargetKey) -> Phase {
        machine.record(target).unwrap().phase
    }

    #[test]
    fn a_target_walks_the_phases_and_the_receipt_digests_agree() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        assert_eq!(phase(&machine, &alpha()), Phase::NotReady);

        let output = bytes.observe_path("out/alpha.json");
        let expected = machine.revision_of(&alpha()).unwrap();
        assert_eq!(expected, INITIAL_REVISION);
        let request = request_for(
            machine.contract(),
            &alpha(),
            &expected,
            &bytes,
            1,
            NOW,
            "args",
            &output,
        );
        let (pins, schema, revision) = (
            request.pins.clone(),
            request.schema_sha256.clone(),
            request.input_revision.clone(),
        );
        let ticket = proceed(machine.begin(&alpha(), request));
        assert_eq!(phase(&machine, &alpha()), Phase::Proposed);

        let candidate = sha_of(b"alpha v1");
        assert_eq!(
            machine.record_local_result(
                &ticket,
                &candidate,
                passed_checks(machine.contract(), &alpha()),
                &output,
            ),
            LocalDecision::ReadyToCommit
        );
        assert_eq!(phase(&machine, &alpha()), Phase::LocallyValid);
        assert!(machine.record(&alpha()).unwrap().current_receipt.is_none());

        machine.record_intent(&ticket, None).unwrap();
        bytes.put("out/alpha.json", b"alpha v1");
        let commit = commit_for(
            machine.contract(),
            &alpha(),
            &pins,
            &schema,
            &revision,
            &candidate,
        );
        let decision = machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit.clone())))
            .unwrap();
        assert!(decision.is_progress());
        assert_eq!(phase(&machine, &alpha()), Phase::Sealed);

        let receipt = machine
            .record(&alpha())
            .unwrap()
            .current_receipt
            .clone()
            .unwrap();
        assert_eq!(receipt.committed_sha256, commit.committed_sha256);
        assert_eq!(receipt.committed_sha256, sha_of(b"alpha v1"));
        assert_eq!(receipt.schema_sha256, schema);
        assert_eq!(receipt.input_revision, revision);
        assert_eq!(receipt.run_nonce, "run-a");
        assert_eq!((receipt.attempt, receipt.generation), (1, 1));
        let CommitDecision::Sealed {
            revision: answered,
            receipt_sha256,
            invalidated,
        } = decision
        else {
            panic!("sealed");
        };
        assert!(is_sha256_hex(&receipt_sha256));
        assert_eq!(answered, machine.revision_of(&alpha()).unwrap());
        assert_ne!(answered, INITIAL_REVISION);
        assert_eq!(invalidated, vec![beta()]);
    }

    /// The same machine over stage sets that differ only in their names and paths. It shows the
    /// names mean nothing to the machine. It is not the evidence for two kinds of profile: that
    /// takes real manifests through the public entry, in the corpus.
    #[test]
    fn the_machine_does_not_care_what_a_stage_set_names_its_stages() {
        for (contract, first, second, fixed) in [
            (
                setting_object_contract(),
                key("settings", "object"),
                key("settings_index", "lookup"),
                "seed/settings-schema.json",
            ),
            (
                document_index_contract(),
                key("catalog", "documents"),
                key("summary", "digest"),
                "seed/document-list.json",
            ),
        ] {
            let mut machine = machine(contract);
            let mut bytes = FakeBytes::default();
            bytes.put(fixed, b"the seed");
            seal(&mut machine, &first, b"first", &mut bytes, 1, NOW, "a");
            seal(&mut machine, &second, b"second", &mut bytes, 2, NOW, "b");
            let verdict = machine.judge(&bytes);
            assert!(verdict.complete);
            let upstream = machine.record(&first).unwrap();
            let downstream = machine.record(&second).unwrap();
            let dependency = &downstream.current_receipt.as_ref().unwrap().dependencies[0];
            assert_eq!(
                dependency.committed_sha256,
                upstream.current_receipt.as_ref().unwrap().committed_sha256
            );
            assert_eq!(dependency.generation, upstream.generation);
        }
    }

    #[test]
    fn d7_row_one_changes_nothing_starts_nothing_and_counts_nothing() {
        let (mut machine, bytes) = both_sealed();
        let before = machine.clone();
        let revision = machine.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let decision = machine.begin(
            &alpha(),
            request_for(
                machine.contract(),
                &alpha(),
                &revision,
                &bytes,
                3,
                NOW,
                "args-a",
                &output,
            ),
        );
        assert_eq!(decision, BeginDecision::NoChange { revision });
        assert!(!decision.starts_process());
        assert!(decision.counts_for_stagnation());
        assert_eq!(machine.snapshot(), before.snapshot());
    }

    #[test]
    fn d7_row_one_needs_every_condition_or_the_whole_procedure_runs() {
        let (base, bytes) = both_sealed();
        let revision = base.revision_of(&alpha()).unwrap();
        let same_bytes = bytes.observe_path("out/alpha.json");
        let changed_bytes = ObservedBytes::Present(b"edited".to_vec());
        let contract = base.contract().clone();
        let starts = |machine: &mut StageMachine,
                      arguments: &str,
                      output: &ObservedBytes,
                      change: &dyn Fn(&mut AttemptRequest<'_>)| {
            let revision_now = machine.revision_of(&alpha()).unwrap();
            let mut request = request_for(
                &contract,
                &alpha(),
                &revision_now,
                &bytes,
                3,
                NOW,
                arguments,
                output,
            );
            change(&mut request);
            machine.begin(&alpha(), request).starts_process()
        };
        let unchanged: &dyn Fn(&mut AttemptRequest<'_>) = &|_| {};

        // With every condition held, it is the first row: the control for the rows below.
        let mut machine = base.clone();
        assert!(!starts(&mut machine, "args-a", &same_bytes, unchanged));
        assert_eq!(machine.revision_of(&alpha()).unwrap(), revision);

        // Other arguments.
        assert!(starts(&mut base.clone(), "other", &same_bytes, unchanged));
        // The output's bytes moved.
        assert!(starts(
            &mut base.clone(),
            "args-a",
            &changed_bytes,
            unchanged
        ));
        // Another runtime pin.
        assert!(starts(
            &mut base.clone(),
            "args-a",
            &same_bytes,
            &|request| {
                request.pins.runtime_sha256 = digest("swapped");
            }
        ));
        // Another schema.
        assert!(starts(
            &mut base.clone(),
            "args-a",
            &same_bytes,
            &|request| {
                request.schema_sha256 = digest("other schema");
            }
        ));
        // Another input revision.
        assert!(starts(
            &mut base.clone(),
            "args-a",
            &same_bytes,
            &|request| {
                request.input_revision = digest("other inputs");
            }
        ));
        // Another process wrote the receipt.
        let mut machine = base.clone();
        machine.snapshot_mut().process_nonce = "process-b".to_string();
        assert!(starts(&mut machine, "args-a", &same_bytes, unchanged));
        // The receipt is of another generation than the target's.
        let mut machine = base.clone();
        machine.snapshot_mut().targets[0].generation += 1;
        assert!(starts(&mut machine, "args-a", &same_bytes, unchanged));
        // Not sealed any more.
        let mut machine = base.clone();
        let verdict = machine.judge(&bytes.with("out/alpha.json", b"edited"));
        assert!(!verdict.complete);
        assert_ne!(phase(&machine, &alpha()), Phase::Sealed);
        assert!(starts(&mut machine, "args-a", &same_bytes, unchanged));
    }

    #[test]
    fn d7_row_two_is_a_new_version_that_takes_the_downstream_with_it() {
        let (mut machine, mut bytes) = both_sealed();
        let attempts = machine.snapshot().attempts_used;
        let upstream = machine.record(&alpha()).unwrap().generation;
        let downstream = machine.record(&beta()).unwrap().generation;
        let downstream_revision = machine.revision_of(&beta()).unwrap();
        seal(
            &mut machine,
            &alpha(),
            b"alpha v2",
            &mut bytes,
            3,
            NOW,
            "args-a2",
        );
        assert_eq!(machine.snapshot().attempts_used, attempts + 1);
        assert_eq!(phase(&machine, &alpha()), Phase::Sealed);
        assert_eq!(machine.record(&alpha()).unwrap().generation, upstream + 1);
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
        // The downstream's generation moves too, so its old revision no longer starts anything.
        assert_eq!(machine.record(&beta()).unwrap().generation, downstream + 1);
        assert_ne!(machine.revision_of(&beta()).unwrap(), downstream_revision);
        assert!(machine.record(&beta()).unwrap().current_receipt.is_none());
        assert_eq!(machine.record(&beta()).unwrap().history.len(), 1);
        assert!(!machine.judge(&bytes).complete);
    }

    #[test]
    fn d7_row_three_is_stale_starts_nothing_and_spends_no_attempt() {
        let (mut machine, mut bytes) = both_sealed();
        let old = machine.revision_of(&alpha()).unwrap();
        seal(
            &mut machine,
            &alpha(),
            b"alpha v2",
            &mut bytes,
            3,
            NOW,
            "args-a2",
        );
        let current = machine.revision_of(&alpha()).unwrap();
        assert_ne!(old, current);
        let attempts = machine.snapshot().attempts_used;
        let output = bytes.observe_path("out/alpha.json");
        for expected in [old.as_str(), INITIAL_REVISION] {
            let decision = machine.begin(
                &alpha(),
                request_for(
                    machine.contract(),
                    &alpha(),
                    expected,
                    &bytes,
                    4,
                    NOW,
                    "args-a",
                    &output,
                ),
            );
            assert_eq!(
                decision,
                BeginDecision::Stale {
                    current_revision: current.clone()
                },
                "{expected}"
            );
            assert!(decision.counts_for_stagnation());
        }
        assert_eq!(machine.snapshot().attempts_used, attempts);
    }

    #[test]
    fn d7_row_four_refuses_what_the_host_never_issued() {
        let (mut machine, bytes) = both_sealed();
        let current = machine.revision_of(&alpha()).unwrap();
        let generation = machine.record(&alpha()).unwrap().generation;
        let attempts = machine.snapshot().attempts_used;

        let other_run = StageMachine::new(
            machine.contract().clone(),
            "run-b".to_string(),
            "process-a".to_string(),
            NOW,
        );
        let mut other = other_run;
        other.snapshot_mut().targets[0].generation = generation;
        let of_other_run = other.revision_of(&alpha()).unwrap();
        let mut of_other_target = machine.clone();
        of_other_target.snapshot_mut().targets[1].generation = generation;
        let of_other_target = of_other_target.revision_of(&beta()).unwrap();
        let future = format!("g{}-{}", generation + 1, &digest("x")[..16]);

        let output = bytes.observe_path("out/alpha.json");
        for claimed in [
            "",
            "latest",
            "accepted",
            "g",
            "g0-0",
            "g-1-abc",
            future.as_str(),
            of_other_run.as_str(),
            of_other_target.as_str(),
        ] {
            assert_ne!(claimed, current);
            let decision = machine.begin(
                &alpha(),
                request_for(
                    machine.contract(),
                    &alpha(),
                    claimed,
                    &bytes,
                    5,
                    NOW,
                    "args-a",
                    &output,
                ),
            );
            assert_eq!(decision, BeginDecision::UnknownRevision, "{claimed:?}");
        }
        assert_eq!(machine.snapshot().attempts_used, attempts);
    }

    #[test]
    fn a_later_call_in_the_reply_that_committed_is_stale() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(&mut machine, &alpha(), b"alpha v1", &mut bytes, 7, NOW, "a");
        let attempts = machine.snapshot().attempts_used;
        let revision = machine.revision_of(&beta()).unwrap();
        let output = bytes.observe_path("out/beta.json");
        let same_turn = machine.begin(
            &beta(),
            request_for(
                machine.contract(),
                &beta(),
                &revision,
                &bytes,
                7,
                NOW,
                "b",
                &output,
            ),
        );
        assert!(matches!(same_turn, BeginDecision::Stale { .. }));
        assert_eq!(machine.snapshot().attempts_used, attempts);

        let revision = machine.revision_of(&beta()).unwrap();
        let next_turn = machine.begin(
            &beta(),
            request_for(
                machine.contract(),
                &beta(),
                &revision,
                &bytes,
                8,
                NOW,
                "b",
                &output,
            ),
        );
        assert!(next_turn.starts_process());
    }

    #[test]
    fn a_downstream_cannot_start_until_its_upstream_is_sealed() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        bytes.put("out/alpha.json", b"written by something else");
        let output = bytes.observe_path("out/beta.json");
        let revision = machine.revision_of(&beta()).unwrap();
        let decision = machine.begin(
            &beta(),
            request_for(
                machine.contract(),
                &beta(),
                &revision,
                &bytes,
                1,
                NOW,
                "b",
                &output,
            ),
        );
        assert_eq!(
            decision,
            BeginDecision::NotReady {
                waiting_on: vec![alpha()]
            }
        );
        assert_eq!(machine.snapshot().attempts_used, 0);
    }

    #[test]
    fn an_input_that_is_not_what_the_upstream_adopted_is_refused() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(&mut machine, &alpha(), b"alpha v1", &mut bytes, 1, NOW, "a");
        let edited = bytes.with("out/alpha.json", b"edited behind the host");
        let output = bytes.observe_path("out/beta.json");
        let revision = machine.revision_of(&beta()).unwrap();
        let decision = machine.begin(
            &beta(),
            request_for(
                machine.contract(),
                &beta(),
                &revision,
                &edited,
                2,
                NOW,
                "b",
                &output,
            ),
        );
        assert_eq!(
            decision,
            BeginDecision::InputNotCurrent {
                path: "out/alpha.json".to_string()
            }
        );
        assert_eq!(machine.record(&beta()).unwrap().attempts_used, 0);
    }

    #[test]
    fn a_fixed_input_is_never_silently_adopted_again() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        seal(&mut machine, &alpha(), b"alpha v1", &mut bytes, 1, NOW, "a");
        let attempts = machine.snapshot().attempts_used;
        let changed = bytes.with("fixed/source.json", b"quietly rewritten");
        let output = changed.observe_path("out/alpha.json");
        let revision = machine.revision_of(&alpha()).unwrap();
        let decision = machine.begin(
            &alpha(),
            request_for(
                machine.contract(),
                &alpha(),
                &revision,
                &changed,
                2,
                NOW,
                "a2",
                &output,
            ),
        );
        assert!(decision.ends_run());
        assert_eq!(
            decision,
            BeginDecision::FixedInputChanged {
                path: "fixed/source.json".to_string()
            }
        );
        assert_eq!(machine.snapshot().attempts_used, attempts);
    }

    #[test]
    fn required_checks_must_match_exactly() {
        let required = ["a", "b"]
            .iter()
            .map(|id| id.to_string())
            .collect::<BTreeSet<_>>();
        let check = |id: &str, status| CheckResult {
            id: id.to_string(),
            status,
        };
        use CheckStatus::{Failed, NotRun, Passed, Unknown};
        assert_eq!(
            judge_checks(&required, &[check("a", Passed), check("b", Passed)]),
            Ok(())
        );
        for (results, expected) in [
            (vec![], CheckFailureKind::NotRun),
            (vec![check("a", Passed)], CheckFailureKind::NotRun),
            (
                vec![check("a", Passed), check("b", NotRun)],
                CheckFailureKind::NotRun,
            ),
            (
                vec![check("a", Passed), check("b", Unknown)],
                CheckFailureKind::Unknown,
            ),
            (
                vec![check("a", Passed), check("b", Failed)],
                CheckFailureKind::Failed,
            ),
            // One the operator never asked for does not make up for one it did, or add to it.
            (
                vec![check("a", Passed), check("c", Passed)],
                CheckFailureKind::Unknown,
            ),
            (
                vec![check("a", Passed), check("b", Passed), check("c", Passed)],
                CheckFailureKind::Unknown,
            ),
            (
                vec![check("a", Passed), check("a", Passed), check("b", Passed)],
                CheckFailureKind::Unknown,
            ),
            // The worst result wins, in any order.
            (
                vec![check("a", Unknown), check("b", Failed)],
                CheckFailureKind::Failed,
            ),
            (
                vec![check("a", Failed), check("b", Unknown)],
                CheckFailureKind::Failed,
            ),
            (
                vec![check("b", NotRun), check("z", Passed)],
                CheckFailureKind::Unknown,
            ),
        ] {
            assert_eq!(
                judge_checks(&required, &results),
                Err(expected),
                "{results:?}"
            );
        }
    }

    #[test]
    fn only_the_check_failures_are_resubmittable_and_the_type_decides() {
        for kind in [
            CheckFailureKind::Failed,
            CheckFailureKind::NotRun,
            CheckFailureKind::Unknown,
        ] {
            assert!(AttemptFailure::Check(kind).is_resubmittable());
        }
        for kind in [
            TerminalFailure::SecretValueInOutput,
            TerminalFailure::Binding,
            TerminalFailure::Pin,
            TerminalFailure::Process,
            TerminalFailure::Path,
            TerminalFailure::Cancelled,
            TerminalFailure::CommitBeforeIntent,
        ] {
            assert!(!AttemptFailure::Terminal(kind).is_resubmittable());
        }
    }

    fn start(
        machine: &mut StageMachine,
        bytes: &FakeBytes,
        turn: u64,
        args: &str,
    ) -> AttemptTicket {
        let target = alpha();
        let revision = machine.revision_of(&target).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let contract = machine.contract().clone();
        proceed(machine.begin(
            &target,
            request_for(
                &contract, &target, &revision, bytes, turn, NOW, args, &output,
            ),
        ))
    }

    #[test]
    fn a_failed_check_keeps_the_target_short_of_sealed_and_can_be_resubmitted() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let ticket = start(&mut machine, &bytes, 1, "a");
        let absent = ObservedBytes::Absent;
        let mut failing = passed_checks(machine.contract(), &alpha());
        failing[0].status = CheckStatus::Failed;
        assert_eq!(
            machine.record_local_result(&ticket, &sha_of(b"bad"), failing, &absent),
            LocalDecision::Resubmit(CheckFailureKind::Failed)
        );
        assert_eq!(phase(&machine, &alpha()), Phase::Proposed);
        assert!(machine.record(&alpha()).unwrap().in_flight.is_none());
        assert_eq!(machine.cas_check(&ticket), Err(StageRefusal::Stale));
        assert_eq!(
            machine.record_intent(&ticket, None),
            Err(StageRefusal::Stale),
            "a failed attempt cannot go on to a commit"
        );
        // Another submission is another attempt.
        let second = start(&mut machine, &bytes, 2, "a2");
        assert_eq!(second.attempt, 2);
    }

    #[test]
    fn a_failed_resubmission_leaves_what_is_sealed_as_it_was() {
        let (mut machine, bytes) = both_sealed();
        let before = machine.record(&alpha()).unwrap().current_receipt.clone();
        let ticket = start(&mut machine, &bytes, 3, "other");
        let output = bytes.observe_path("out/alpha.json");
        assert_eq!(
            machine.record_local_result(&ticket, &sha_of(b"x"), Vec::new(), &output),
            LocalDecision::Resubmit(CheckFailureKind::NotRun)
        );
        assert_eq!(phase(&machine, &alpha()), Phase::Sealed);
        assert_eq!(machine.record(&alpha()).unwrap().current_receipt, before);
        assert_eq!(phase(&machine, &beta()), Phase::Sealed);
        assert!(machine.judge(&bytes).complete);
    }

    #[test]
    fn a_terminal_failure_ends_the_run_and_never_offers_a_retry() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let ticket = start(&mut machine, &bytes, 1, "a");
        assert_eq!(
            machine.record_failure(
                &ticket,
                AttemptFailure::Terminal(TerminalFailure::SecretValueInOutput)
            ),
            FailureDecision::EndRun(TerminalFailure::SecretValueInOutput)
        );
        assert_eq!(
            machine.record_failure(&ticket, AttemptFailure::Check(CheckFailureKind::Failed)),
            FailureDecision::Refused(StageRefusal::Stale)
        );
        assert_ne!(phase(&machine, &alpha()), Phase::Sealed);
    }

    #[test]
    fn the_same_report_is_a_replay_and_another_is_a_conflict() {
        let (mut machine, bytes) = both_sealed();
        let ticket = start(&mut machine, &bytes, 3, "other");
        let output = bytes.observe_path("out/alpha.json");
        let checks = passed_checks(machine.contract(), &alpha());
        let candidate = sha_of(b"alpha v2");
        assert_eq!(
            machine.record_local_result(&ticket, &candidate, checks.clone(), &output),
            LocalDecision::ReadyToCommit
        );
        let after_first = machine.clone();
        assert_eq!(
            machine.record_local_result(&ticket, &candidate, checks.clone(), &output),
            LocalDecision::Replay
        );
        assert_eq!(
            machine.record_local_result(
                &ticket,
                &sha_of(b"something else"),
                checks.clone(),
                &output
            ),
            LocalDecision::Conflict
        );
        let mut other_checks = checks;
        other_checks[0].status = CheckStatus::Failed;
        assert_eq!(
            machine.record_local_result(&ticket, &candidate, other_checks, &output),
            LocalDecision::Conflict
        );
        assert_eq!(machine.snapshot(), after_first.snapshot());
    }

    #[test]
    fn a_late_or_foreign_result_changes_nothing() {
        let (mut machine, bytes) = both_sealed();
        let first = start(&mut machine, &bytes, 3, "other");
        // Superseded by a newer attempt before it reported.
        let second = start(&mut machine, &bytes, 4, "another");
        let output = bytes.observe_path("out/alpha.json");
        let checks = passed_checks(machine.contract(), &alpha());
        let before = machine.clone();
        assert_eq!(
            machine.record_local_result(&first, &sha_of(b"late"), checks.clone(), &output),
            LocalDecision::Refused(StageRefusal::Stale)
        );
        assert_eq!(machine.cas_check(&first), Err(StageRefusal::Stale));
        assert_eq!(
            machine.record_commit(&first, CommitObservation::OutcomeUnknown),
            Err(StageRefusal::Stale)
        );

        let mut other_run = second.clone();
        other_run.run_nonce = "run-b".to_string();
        assert_eq!(
            machine.record_local_result(&other_run, &sha_of(b"x"), checks.clone(), &output),
            LocalDecision::Refused(StageRefusal::OtherRun)
        );
        assert_eq!(machine.cas_check(&other_run), Err(StageRefusal::OtherRun));
        assert_eq!(
            machine.record_commit(&other_run, CommitObservation::OutcomeUnknown),
            Err(StageRefusal::OtherRun)
        );
        let mut other_target = second.clone();
        other_target.target = "nowhere".to_string();
        assert_eq!(
            machine.record_local_result(&other_target, &sha_of(b"x"), checks.clone(), &output),
            LocalDecision::Refused(StageRefusal::UnknownTarget)
        );
        assert_eq!(
            machine.record_local_result(&second, "not a digest", checks, &output),
            LocalDecision::Refused(StageRefusal::MalformedDigest)
        );
        assert_eq!(machine.snapshot(), before.snapshot());
    }

    #[test]
    fn a_commit_cannot_skip_the_steps_before_it() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let ticket = start(&mut machine, &bytes, 1, "a");
        assert_eq!(
            machine.record_intent(&ticket, None),
            Err(StageRefusal::OutOfOrder),
            "no intent before the result"
        );
        assert_eq!(
            machine.record_commit(&ticket, CommitObservation::NotAdopted),
            Err(StageRefusal::OutOfOrder),
            "no commit before the intent"
        );
        assert!(machine.record(&alpha()).unwrap().in_flight.is_some());
    }

    #[test]
    fn the_same_bytes_on_a_sealed_target_are_neither_committed_nor_counted_as_progress() {
        let (mut machine, bytes) = both_sealed();
        let receipt = machine.record(&alpha()).unwrap().current_receipt.clone();
        let generation = machine.record(&alpha()).unwrap().generation;
        let ticket = start(&mut machine, &bytes, 3, "different arguments");
        let output = bytes.observe_path("out/alpha.json");
        let checks = passed_checks(machine.contract(), &alpha());
        assert_eq!(
            machine.record_local_result(&ticket, &sha_of(b"alpha v1"), checks, &output),
            LocalDecision::Unchanged
        );
        let record = machine.record(&alpha()).unwrap();
        assert_eq!(record.generation, generation);
        assert_eq!(record.current_receipt, receipt);
        assert_eq!(phase(&machine, &beta()), Phase::Sealed);
        assert_eq!(
            machine.record_intent(&ticket, None),
            Err(StageRefusal::Stale)
        );
    }

    #[test]
    fn the_same_bad_candidate_runs_out_at_the_limit_and_never_progresses() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let limit = machine.contract().limits().attempts_per_target;
        let absent = ObservedBytes::Absent;
        for turn in 0..u64::from(limit) {
            let ticket = start(&mut machine, &bytes, turn, "same");
            let mut checks = passed_checks(machine.contract(), &alpha());
            checks[0].status = CheckStatus::Failed;
            assert_eq!(
                machine.record_local_result(&ticket, &sha_of(b"same bad"), checks, &absent),
                LocalDecision::Resubmit(CheckFailureKind::Failed)
            );
            assert_eq!(machine.record(&alpha()).unwrap().generation, 0);
        }
        let revision = machine.revision_of(&alpha()).unwrap();
        let decision = machine.begin(
            &alpha(),
            request_for(
                machine.contract(),
                &alpha(),
                &revision,
                &bytes,
                99,
                NOW,
                "same",
                &absent,
            ),
        );
        assert_eq!(
            decision,
            BeginDecision::AttemptsExhausted(ExhaustedLimit::Target)
        );
        assert!(decision.ends_run());
        assert_eq!(machine.record(&alpha()).unwrap().attempts_used, limit);
        assert!(!machine.judge(&bytes).complete);
    }

    #[test]
    fn the_run_limit_holds_across_targets_and_revisions_give_nothing_back() {
        let declaration = StageSetDeclaration {
            stages: vec![one_target("one", "t1"), one_target("two", "t2")],
            limits: NarrowedStageLimits {
                attempts_per_target: None,
                attempts_per_run: Some(3),
                deadline_seconds: None,
            },
        };
        let tools = [facts("t1", "out/one", &[]), facts("t2", "out/two", &[])];
        let mut machine = machine(admit(&declaration, &tools).unwrap());
        let mut bytes = FakeBytes::default();
        let one = key("one", "t1");
        let two = key("two", "t2");
        // Two new versions on one target do not refill anything.
        seal(&mut machine, &one, b"v1", &mut bytes, 1, NOW, "a");
        seal(&mut machine, &one, b"v2", &mut bytes, 2, NOW, "b");
        seal(&mut machine, &one, b"v1", &mut bytes, 3, NOW, "c");
        let revision = machine.revision_of(&two).unwrap();
        let absent = ObservedBytes::Absent;
        assert_eq!(
            machine.begin(
                &two,
                request_for(
                    machine.contract(),
                    &two,
                    &revision,
                    &bytes,
                    4,
                    NOW,
                    "d",
                    &absent
                ),
            ),
            BeginDecision::AttemptsExhausted(ExhaustedLimit::Run)
        );
    }

    #[test]
    fn the_deadline_is_exact_and_a_clock_going_back_returns_no_time() {
        let mut machine = machine(two_stage_contract());
        let mut bytes = FakeBytes::default();
        bytes.put("fixed/source.json", b"fixed source");
        let deadline_ms = machine.contract().limits().deadline_seconds * 1000;
        let absent = ObservedBytes::Absent;
        let call = |machine: &mut StageMachine, now: u64, turn: u64| {
            let revision = machine.revision_of(&alpha()).unwrap();
            let contract = machine.contract().clone();
            machine.begin(
                &alpha(),
                request_for(
                    &contract,
                    &alpha(),
                    &revision,
                    &bytes,
                    turn,
                    now,
                    "a",
                    &absent,
                ),
            )
        };
        assert!(call(&mut machine, NOW + deadline_ms - 1, 1).starts_process());
        assert_eq!(
            call(&mut machine, NOW + deadline_ms, 2),
            BeginDecision::DeadlineExceeded
        );
        assert_eq!(call(&mut machine, NOW, 3), BeginDecision::DeadlineExceeded);
        assert_eq!(machine.snapshot().attempts_used, 1);
    }

    #[test]
    fn aba_on_the_upstream_does_not_bring_the_old_downstream_back() {
        let (mut machine, mut bytes) = both_sealed();
        let old_receipt = machine
            .record(&beta())
            .unwrap()
            .current_receipt
            .clone()
            .unwrap();
        seal(
            &mut machine,
            &alpha(),
            b"alpha v2",
            &mut bytes,
            3,
            NOW,
            "a2",
        );
        seal(
            &mut machine,
            &alpha(),
            b"alpha v1",
            &mut bytes,
            4,
            NOW,
            "a3",
        );
        assert_eq!(
            bytes.observe_path("out/alpha.json").sha256(),
            Some(sha_of(b"alpha v1"))
        );
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
        assert!(!machine.judge(&bytes).complete);

        seal(&mut machine, &beta(), b"beta v1", &mut bytes, 5, NOW, "b");
        let fresh = machine
            .record(&beta())
            .unwrap()
            .current_receipt
            .clone()
            .unwrap();
        assert_eq!(fresh.committed_sha256, old_receipt.committed_sha256);
        assert_ne!(fresh.sha256(), old_receipt.sha256());
        assert!(fresh.dependencies[0].generation > old_receipt.dependencies[0].generation);
        assert!(machine.judge(&bytes).complete);
    }

    #[test]
    fn the_receipt_history_is_bounded_and_no_judgment_reads_it() {
        let declaration = StageSetDeclaration {
            stages: vec![one_target("one", "t1")],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [facts("t1", "out/one", &[])];
        let mut machine = machine(admit(&declaration, &tools).unwrap());
        let one = key("one", "t1");
        let mut bytes = FakeBytes::default();
        let limit = machine.contract().limits().attempts_per_target;
        for turn in 0..u64::from(limit) {
            let content = format!("v{turn}");
            seal(
                &mut machine,
                &one,
                content.as_bytes(),
                &mut bytes,
                turn,
                NOW,
                &content,
            );
        }
        let record = machine.record(&one).unwrap();
        assert_eq!(
            record.history.len(),
            (limit as usize - 1).min(MAX_RECEIPT_HISTORY)
        );
        for _ in 0..3 {
            machine.snapshot_mut().targets[0].push_history(machine_receipt());
        }
        assert_eq!(
            machine.record(&one).unwrap().history.len(),
            MAX_RECEIPT_HISTORY
        );

        // An old receipt's bytes put back: the current digest is what is compared.
        let verdict = machine.judge(&bytes.with("out/one", b"v0"));
        assert!(!verdict.complete);
        assert_eq!(verdict.invalidated, vec![one.clone()]);
        assert_eq!(
            machine.record(&one).unwrap().unverified_reason,
            Some(UnverifiedReason::BytesChanged)
        );
    }

    fn machine_receipt() -> StageReceipt {
        use super::super::test_support::{commit_receipt, parts};
        let candidate = digest("history");
        StageReceipt::bind(parts(&candidate), commit_receipt(&candidate)).unwrap()
    }

    #[test]
    fn the_verdict_compares_bytes_and_a_change_takes_the_target_and_its_downstream() {
        let (mut machine, bytes) = both_sealed();
        assert!(machine.judge(&bytes).complete);
        let generation = machine.record(&alpha()).unwrap().generation;
        let revision = machine.revision_of(&alpha()).unwrap();

        let verdict = machine.judge(&bytes.with("out/alpha.json", b"edited with Write"));
        assert!(!verdict.complete);
        assert_eq!(verdict.invalidated, vec![alpha(), beta()]);
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
        // A call that still holds the revision from before the edit is told about the new one.
        assert_eq!(machine.record(&alpha()).unwrap().generation, generation + 1);
        let output = bytes.observe_path("out/alpha.json");
        let contract = machine.contract().clone();
        assert!(matches!(
            machine.begin(
                &alpha(),
                request_for(
                    &contract,
                    &alpha(),
                    &revision,
                    &bytes,
                    9,
                    NOW,
                    "args-a",
                    &output,
                ),
            ),
            BeginDecision::Stale { .. }
        ));

        // Putting the bytes back does not restore anything.
        assert!(!machine.judge(&bytes).complete);
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
    }

    #[test]
    fn a_change_to_the_last_stage_keeps_what_is_above_it() {
        let (mut machine, bytes) = both_sealed();
        let verdict = machine.judge(&bytes.with("out/beta.json", b"edited"));
        assert_eq!(verdict.invalidated, vec![beta()]);
        assert_eq!(phase(&machine, &alpha()), Phase::Sealed);
    }

    #[test]
    fn a_missing_or_unreadable_output_or_a_moved_input_is_not_sealed() {
        for damage in [
            FakeBytes::without as fn(&FakeBytes, &str) -> FakeBytes,
            FakeBytes::unreadable,
        ] {
            let (mut machine, bytes) = both_sealed();
            assert!(!machine.judge(&damage(&bytes, "out/beta.json")).complete);
            assert_eq!(
                machine.record(&beta()).unwrap().unverified_reason,
                Some(UnverifiedReason::BytesChanged)
            );
        }
        let (mut machine, bytes) = both_sealed();
        assert!(
            !machine
                .judge(&bytes.with("fixed/source.json", b"changed"))
                .complete
        );
        assert_eq!(
            machine.record(&alpha()).unwrap().unverified_reason,
            Some(UnverifiedReason::InputChanged)
        );
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
    }

    #[test]
    fn unrelated_bytes_and_repeated_judgments_move_nothing() {
        let (mut machine, bytes) = both_sealed();
        let before = machine.clone();
        // The same content rewritten is what an mtime-only change looks like here: the source
        // has no timestamp to offer.
        for _ in 0..5 {
            let verdict = machine.judge(&bytes.with("unrelated/file.txt", b"noise"));
            assert!(verdict.complete);
            assert!(verdict.invalidated.is_empty());
        }
        assert_eq!(machine.snapshot(), before.snapshot());
    }

    #[test]
    fn a_stage_set_with_nothing_done_is_not_complete_and_reading_changes_nothing() {
        let mut machine = machine(two_stage_contract());
        let bytes = FakeBytes::default()
            .with("out/alpha.json", b"present")
            .with("out/beta.json", b"present");
        let before = machine.clone();
        assert!(!machine.judge(&bytes).complete);
        assert_eq!(machine.snapshot(), before.snapshot());
    }

    fn at_commit(
        machine: &mut StageMachine,
        bytes: &mut FakeBytes,
        turn: u64,
    ) -> (AttemptTicket, String) {
        let ticket = start(machine, bytes, turn, "new");
        let output = bytes.observe_path("out/alpha.json");
        let candidate = sha_of(b"alpha new");
        let checks = passed_checks(machine.contract(), &alpha());
        assert_eq!(
            machine.record_local_result(&ticket, &candidate, checks, &output),
            LocalDecision::ReadyToCommit
        );
        machine.record_intent(&ticket, output.sha256()).unwrap();
        bytes.put("out/alpha.json", b"alpha new");
        (ticket, candidate)
    }

    fn in_flight_commit(
        machine: &StageMachine,
        candidate: &str,
    ) -> crate::tools::profile_tools::commit::CommitReceipt {
        let in_flight = machine.record(&alpha()).unwrap().in_flight.clone().unwrap();
        commit_for(
            machine.contract(),
            &alpha(),
            &in_flight.pins,
            &in_flight.schema_sha256,
            &in_flight.input_revision,
            candidate,
        )
    }

    #[test]
    fn a_commit_cancelled_after_the_rename_or_not_durable_moves_the_generation_without_a_seal() {
        for (change, reason) in [
            (
                (|commit| commit.cancelled_after_commit = true)
                    as fn(&mut crate::tools::profile_tools::commit::CommitReceipt),
                UnverifiedReason::CancelledAfterCommit,
            ),
            (
                |commit| commit.durable = false,
                UnverifiedReason::CommitNotDurable,
            ),
        ] {
            let (mut machine, mut bytes) = both_sealed();
            let generation = machine.record(&alpha()).unwrap().generation;
            let (ticket, candidate) = at_commit(&mut machine, &mut bytes, 3);
            let mut commit = in_flight_commit(&machine, &candidate);
            change(&mut commit);
            let decision = machine
                .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
                .unwrap();
            assert!(!decision.is_progress());
            let CommitDecision::StoodUnverified {
                reason: got,
                invalidated,
                ..
            } = decision
            else {
                panic!("unverified");
            };
            assert_eq!(got, reason);
            assert_eq!(invalidated, vec![beta()]);
            let record = machine.record(&alpha()).unwrap();
            assert_eq!(record.phase, Phase::Unverified);
            assert_eq!(record.generation, generation + 1);
            assert!(record.current_receipt.is_none());
            assert_eq!(phase(&machine, &beta()), Phase::NotReady);
            assert!(!machine.judge(&bytes).complete);
        }
    }

    #[test]
    fn a_commit_that_does_not_describe_the_attempt_seals_nothing() {
        let (mut machine, mut bytes) = both_sealed();
        let (ticket, candidate) = at_commit(&mut machine, &mut bytes, 3);
        let mut commit = in_flight_commit(&machine, &candidate);
        commit.path = "out/somewhere-else.json".to_string();
        let decision = machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
            .unwrap();
        assert!(matches!(
            decision,
            CommitDecision::StoodUnverified {
                reason: UnverifiedReason::ReceiptRefused,
                ..
            }
        ));
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
    }

    #[test]
    fn an_unknown_commit_outcome_counts_as_moved_and_a_not_adopted_one_does_not() {
        let (mut machine, mut bytes) = both_sealed();
        let generation = machine.record(&alpha()).unwrap().generation;
        let (ticket, _) = at_commit(&mut machine, &mut bytes, 3);
        let decision = machine
            .record_commit(&ticket, CommitObservation::OutcomeUnknown)
            .unwrap();
        assert!(matches!(
            decision,
            CommitDecision::StoodUnverified {
                reason: UnverifiedReason::CommitOutcomeUnknown,
                ..
            }
        ));
        assert_eq!(machine.record(&alpha()).unwrap().generation, generation + 1);
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);

        let (mut machine, mut bytes) = both_sealed();
        let attempts = machine.snapshot().attempts_used;
        let generation = machine.record(&alpha()).unwrap().generation;
        let downstream = machine.record(&beta()).unwrap().generation;
        let (ticket, _) = at_commit(&mut machine, &mut bytes, 3);
        assert_eq!(
            machine.record_commit(&ticket, CommitObservation::NotAdopted),
            Ok(CommitDecision::NotAdopted)
        );
        let record = machine.record(&alpha()).unwrap();
        assert_eq!(record.generation, generation);
        assert!(record.in_flight.is_none());
        assert_eq!(machine.record(&beta()).unwrap().generation, downstream);
        assert_eq!(record.phase, Phase::Sealed);
        assert_eq!(
            machine.snapshot().attempts_used,
            attempts + 1,
            "the attempt stays spent"
        );
        assert_eq!(phase(&machine, &beta()), Phase::Sealed);
    }

    #[test]
    fn a_commit_receipt_replayed_after_the_commit_is_stale() {
        let (mut machine, mut bytes) = both_sealed();
        let (ticket, candidate) = at_commit(&mut machine, &mut bytes, 3);
        let commit = in_flight_commit(&machine, &candidate);
        machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit.clone())))
            .unwrap();
        let after = machine.clone();
        assert_eq!(
            machine.record_commit(&ticket, CommitObservation::Stood(Box::new(commit))),
            Err(StageRefusal::Stale)
        );
        assert_eq!(machine.snapshot(), after.snapshot());
    }

    #[test]
    fn a_failure_report_cannot_end_an_attempt_whose_rename_may_have_happened() {
        let (mut machine, mut bytes) = both_sealed();
        let generation = machine.record(&alpha()).unwrap().generation;
        let (ticket, _) = at_commit(&mut machine, &mut bytes, 3);
        let before = machine.clone();
        for failure in [
            AttemptFailure::Terminal(TerminalFailure::CommitBeforeIntent),
            AttemptFailure::Terminal(TerminalFailure::Process),
            AttemptFailure::Terminal(TerminalFailure::Cancelled),
            AttemptFailure::Check(CheckFailureKind::Failed),
        ] {
            assert_eq!(
                machine.record_failure(&ticket, failure),
                FailureDecision::Refused(StageRefusal::OutOfOrder),
                "{failure:?}"
            );
            assert_eq!(machine.snapshot(), before.snapshot(), "{failure:?}");
        }
        // The way out is still there, and it moves the generation.
        machine
            .record_commit(&ticket, CommitObservation::OutcomeUnknown)
            .unwrap();
        assert_eq!(machine.record(&alpha()).unwrap().generation, generation + 1);
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
    }

    #[test]
    fn a_next_call_cannot_replace_an_attempt_whose_rename_may_have_happened() {
        let (mut machine, mut bytes) = both_sealed();
        at_commit(&mut machine, &mut bytes, 3);
        let before = machine.clone();
        let revision = machine.revision_of(&alpha()).unwrap();
        let output = bytes.observe_path("out/alpha.json");
        let contract = machine.contract().clone();
        let decision = machine.begin(
            &alpha(),
            request_for(
                &contract,
                &alpha(),
                &revision,
                &bytes,
                4,
                NOW + 1,
                "next",
                &output,
            ),
        );
        assert_eq!(decision, BeginDecision::Refused(StageRefusal::OutOfOrder));
        assert!(decision.ends_run());
        assert_eq!(machine.snapshot(), before.snapshot());
        // Another target is not held up by it.
        assert_eq!(phase(&machine, &beta()), Phase::Sealed);
    }

    #[test]
    fn an_upstream_commit_leaves_a_downstream_intent_its_way_out() {
        let (mut machine, mut bytes) = both_sealed();
        let revision = machine.revision_of(&beta()).unwrap();
        let output = bytes.observe_path("out/beta.json");
        let contract = machine.contract().clone();
        let ticket = proceed(machine.begin(
            &beta(),
            request_for(&contract, &beta(), &revision, &bytes, 3, NOW, "b2", &output),
        ));
        let candidate = sha_of(b"beta v2");
        assert_eq!(
            machine.record_local_result(
                &ticket,
                &candidate,
                passed_checks(&contract, &beta()),
                &output,
            ),
            LocalDecision::ReadyToCommit
        );
        machine.record_intent(&ticket, output.sha256()).unwrap();
        let in_flight = machine.record(&beta()).unwrap().in_flight.clone().unwrap();
        seal(
            &mut machine,
            &alpha(),
            b"alpha v2",
            &mut bytes,
            4,
            NOW,
            "a2",
        );
        assert!(machine.record(&beta()).unwrap().has_recorded_intent());

        let commit = commit_for(
            &contract,
            &beta(),
            &in_flight.pins,
            &in_flight.schema_sha256,
            &in_flight.input_revision,
            &candidate,
        );
        let decision = machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
            .unwrap();
        assert!(matches!(
            decision,
            CommitDecision::StoodUnverified {
                reason: UnverifiedReason::ChangedDuringCommit,
                ..
            }
        ));
        assert_eq!(phase(&machine, &beta()), Phase::Unverified);
    }

    #[test]
    fn a_judgment_leaves_an_intent_its_way_out_and_drops_an_earlier_attempt() {
        let (mut machine, mut bytes) = both_sealed();
        let (ticket, _) = at_commit(&mut machine, &mut bytes, 3);
        assert!(!machine.judge(&bytes).complete);
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
        assert!(machine.record(&alpha()).unwrap().has_recorded_intent());
        machine
            .record_commit(&ticket, CommitObservation::OutcomeUnknown)
            .unwrap();

        let (mut machine, bytes) = both_sealed();
        let started = start(&mut machine, &bytes, 3, "other");
        assert!(
            !machine
                .judge(&bytes.with("out/alpha.json", b"edited"))
                .complete
        );
        assert!(machine.record(&alpha()).unwrap().in_flight.is_none());
        assert_eq!(machine.cas_check(&started), Err(StageRefusal::Stale));
    }

    #[test]
    fn an_intent_still_open_keeps_the_verdict_from_complete_and_changes_no_obligation() {
        let (mut machine, mut bytes) = both_sealed();
        let before = bytes.clone();
        let (ticket, _) = at_commit(&mut machine, &mut bytes, 3);
        let held = machine.clone();

        // The bytes are still those the receipts name, and only the open intent says no.
        let verdict = machine.judge(&before);
        assert!(!verdict.complete);
        assert!(verdict.ledger_intact);
        assert!(verdict.invalidated.is_empty());
        assert!(
            verdict
                .obligations
                .iter()
                .all(|status| status.phase == Phase::Sealed && status.receipt_sha256.is_some())
        );
        assert_eq!(machine.snapshot(), held.snapshot());

        machine
            .record_commit(&ticket, CommitObservation::NotAdopted)
            .unwrap();
        assert!(machine.judge(&before).complete, "with the intent closed");
    }

    /// Admission refuses a set with no target and a saved record must name every target, so only
    /// a state built here can show this guard.
    #[test]
    fn a_machine_with_no_obligations_is_not_complete() {
        let mut machine = machine(two_stage_contract());
        machine.snapshot_mut().targets.clear();
        let verdict = machine.judge(&FakeBytes::default());
        assert!(verdict.obligations.is_empty());
        assert!(!verdict.complete);
    }

    #[test]
    fn a_sealed_record_without_a_receipt_is_lowered_and_never_complete() {
        let (mut machine, bytes) = both_sealed();
        machine.snapshot_mut().targets[1].current_receipt = None;
        let verdict = machine.judge(&bytes);
        assert!(!verdict.complete);
        assert_eq!(phase(&machine, &beta()), Phase::Unverified);
        assert_eq!(
            machine.record(&beta()).unwrap().unverified_reason,
            Some(UnverifiedReason::ReceiptRefused)
        );
    }

    #[test]
    fn a_commit_that_stands_after_its_bytes_moved_does_not_name_a_dependency() {
        let (mut machine, mut bytes) = both_sealed();
        let (ticket, candidate) = at_commit(&mut machine, &mut bytes, 3);
        let commit = in_flight_commit(&machine, &candidate);
        // The judgment moves the generation over the output's own bytes: no upstream moved.
        assert!(!machine.judge(&bytes).complete);
        assert_eq!(
            machine.record(&alpha()).unwrap().unverified_reason,
            Some(UnverifiedReason::BytesChanged)
        );
        let decision = machine
            .record_commit(&ticket, CommitObservation::Stood(Box::new(commit)))
            .unwrap();
        let CommitDecision::StoodUnverified { reason, .. } = decision else {
            panic!("unverified, got {decision:?}");
        };
        assert_eq!(reason, UnverifiedReason::ChangedDuringCommit);
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);
    }

    #[test]
    fn a_failed_check_on_an_unverified_target_ends_as_proposed_and_keeps_the_reason() {
        let (mut machine, bytes) = both_sealed();
        let edited = bytes.with("out/alpha.json", b"edited");
        assert!(!machine.judge(&edited).complete);
        assert_eq!(phase(&machine, &alpha()), Phase::Unverified);

        let ticket = start(&mut machine, &edited, 3, "again");
        let output = edited.observe_path("out/alpha.json");
        assert_eq!(
            machine.record_local_result(&ticket, &sha_of(b"bad"), Vec::new(), &output),
            LocalDecision::Resubmit(CheckFailureKind::NotRun)
        );
        let record = machine.record(&alpha()).unwrap();
        assert_eq!(record.phase, Phase::Proposed);
        assert_eq!(
            record.unverified_reason,
            Some(UnverifiedReason::BytesChanged)
        );
        assert!(!machine.judge(&edited).complete);
    }

    #[test]
    fn a_candidate_like_the_sealed_digest_still_commits_when_the_file_no_longer_holds_it() {
        let (mut machine, bytes) = both_sealed();
        let ticket = start(&mut machine, &bytes, 3, "other");
        let edited = ObservedBytes::Present(b"edited behind the host".to_vec());
        let checks = passed_checks(machine.contract(), &alpha());
        assert_eq!(
            machine.record_local_result(&ticket, &sha_of(b"alpha v1"), checks, &edited),
            LocalDecision::ReadyToCommit
        );
    }

    #[test]
    fn a_candidate_other_than_the_sealed_digest_goes_on_to_a_commit() {
        let (mut machine, bytes) = both_sealed();
        let ticket = start(&mut machine, &bytes, 3, "other");
        let output = bytes.observe_path("out/alpha.json");
        let checks = passed_checks(machine.contract(), &alpha());
        assert_eq!(
            machine.record_local_result(&ticket, &sha_of(b"alpha v2"), checks, &output),
            LocalDecision::ReadyToCommit
        );
        assert!(machine.record_intent(&ticket, output.sha256()).is_ok());
    }

    #[test]
    fn an_invalidation_during_an_attempt_makes_it_stale() {
        let (mut machine, mut bytes) = both_sealed();
        // `beta` starts, then `alpha` takes a new version before beta's result arrives.
        let revision = machine.revision_of(&beta()).unwrap();
        let output = bytes.observe_path("out/beta.json");
        let contract = machine.contract().clone();
        let late = proceed(machine.begin(
            &beta(),
            request_for(&contract, &beta(), &revision, &bytes, 3, NOW, "b2", &output),
        ));
        seal(
            &mut machine,
            &alpha(),
            b"alpha v2",
            &mut bytes,
            4,
            NOW,
            "a2",
        );
        let checks = passed_checks(machine.contract(), &beta());
        assert_eq!(
            machine.record_local_result(&late, &sha_of(b"beta late"), checks, &output),
            LocalDecision::Refused(StageRefusal::Stale)
        );
        assert_eq!(machine.cas_check(&late), Err(StageRefusal::Stale));
        assert_eq!(phase(&machine, &beta()), Phase::NotReady);
    }

    #[test]
    fn the_identity_for_a_budget_reads_without_changing_anything() {
        let (machine, _) = both_sealed();
        let before = machine.clone();
        let identity = machine.attempt_identity(&alpha()).unwrap();
        assert_eq!(identity.run_nonce, "run-a");
        assert_eq!(identity.stage, "alpha");
        assert_eq!(identity.target, "main");
        assert_eq!(identity.attempt, 1);
        assert_eq!(identity.generation, 1);
        assert!(machine.attempt_identity(&key("alpha", "nowhere")).is_none());
        assert_eq!(machine, before);

        let fresh = StageMachine::new(
            machine.contract().clone(),
            "run-c".to_string(),
            "p".to_string(),
            NOW,
        );
        assert!(fresh.attempt_identity(&alpha()).is_none());
    }

    #[test]
    fn a_two_target_stage_keeps_the_sibling_and_seals_only_when_both_do() {
        let mut stage = one_target("pair", "t1");
        stage.targets.push(TargetDeclaration {
            id: "t2".to_string(),
            tool_id: "t2".to_string(),
        });
        let declaration = StageSetDeclaration {
            stages: vec![stage, one_target("reader", "t3")],
            limits: NarrowedStageLimits::default(),
        };
        let tools = [
            facts("t1", "out/one", &[]),
            facts("t2", "out/two", &[]),
            facts("t3", "out/three", &["out/two"]),
        ];
        let mut machine = machine(admit(&declaration, &tools).unwrap());
        let (first, second, third) = (key("pair", "t1"), key("pair", "t2"), key("reader", "t3"));
        let mut bytes = FakeBytes::default();
        seal(&mut machine, &first, b"one", &mut bytes, 1, NOW, "a");

        // The reader waits on the whole upstream stage.
        let revision = machine.revision_of(&third).unwrap();
        let absent = ObservedBytes::Absent;
        let contract = machine.contract().clone();
        assert_eq!(
            machine.begin(
                &third,
                request_for(&contract, &third, &revision, &bytes, 2, NOW, "c", &absent),
            ),
            BeginDecision::NotReady {
                waiting_on: vec![second.clone()]
            }
        );
        seal(&mut machine, &second, b"two", &mut bytes, 3, NOW, "b");
        seal(&mut machine, &third, b"three", &mut bytes, 4, NOW, "c");
        assert!(machine.judge(&bytes).complete);

        // A new version of one target takes the reader, and leaves the sibling.
        seal(&mut machine, &first, b"one again", &mut bytes, 5, NOW, "a2");
        assert_eq!(phase(&machine, &second), Phase::Sealed);
        assert_eq!(phase(&machine, &third), Phase::NotReady);
    }
}
