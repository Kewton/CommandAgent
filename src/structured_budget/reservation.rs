//! The money and the state machine of one wallet. Nothing here touches a disk or an `f64`.
//!
//! | state      | next                           | the wallet                |
//! |------------|--------------------------------|---------------------------|
//! | `reserved` | `started`, `released`          | holds the reservation     |
//! | `started`  | `settled`, `unknown`           | holds the reservation     |
//! | `unknown`  | `settled`, once; latch stays   | holds the reservation     |
//! | `settled`  | none                           | has spent the amount      |
//! | `released` | none                           | holds nothing             |
//!
//! Every limit is judged in one place, [`WalletState::check_limits`], in the one call that takes
//! the reservation.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::clock::{ClockRecord, RunClock};
use super::identity::{
    AttemptBinding, LogicalClaim, LogicalId, LogicalKind, PhysicalId, SendPurpose, SubmissionSource,
};
use super::policy::{RunBudgetPolicy, Tariff, WireRoute};
use super::settlement::{SettlementRecord, UnknownReason};
use crate::planner::structured_stages::receipt::sha256_hex;
use crate::providers::BilledCostUsd;

pub(crate) const WALLET_SCHEMA: &str = "run-budget.ledger/v1";

/// Decimal places of the unit: an amount is a count of 10^-18 dollars.
const UNIT_DECIMALS: i128 = 18;
/// u128 holds up to 38 full decimal digits.
const MAX_POWER_OF_TEN: i128 = 38;
const PER_MILLION: u128 = 1_000_000;
/// An exponent past this says nothing more: it is past every digit count a token can have.
const EXPONENT_CEILING: i128 = 1 << 62;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AmountError {
    Malformed,
    Overflow,
}

/// An amount in units of 10^-18 dollars, rounded up from its decimal text, never through a float.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Money(u128);

impl Money {
    pub(crate) const ZERO: Self = Self(0);

    /// The JSON number grammar with no sign. `0`, `0.0`, and `0e5` are zero, and only those: any
    /// other digit, however far below the unit (`1e-400`), rounds up to one unit.
    pub(crate) fn parse_decimal_up(text: &str) -> Result<Self, AmountError> {
        let (mantissa, exponent) = match text.find(['e', 'E']) {
            Some(at) => (&text[..at], Some(&text[at + 1..])),
            None => (text, None),
        };
        let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let digits_only = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
        if !digits_only(integer)
            || (integer.len() > 1 && integer.starts_with('0'))
            || (mantissa.contains('.') && !digits_only(fraction))
        {
            return Err(AmountError::Malformed);
        }
        let exponent = exponent.map_or(Ok(0), parse_exponent)?;
        let digits = format!("{integer}{fraction}");
        let digits = digits.trim_start_matches('0');
        if digits.is_empty() {
            return Ok(Self::ZERO);
        }
        // The value in units is `digits * 10^shift`.
        let fraction_len = i128::try_from(fraction.len()).map_err(|_| AmountError::Overflow)?;
        let shift = exponent - fraction_len + UNIT_DECIMALS;
        if shift >= 0 {
            if shift > MAX_POWER_OF_TEN {
                return Err(AmountError::Overflow);
            }
            let scale = 10_u128.pow(u32::try_from(shift).map_err(|_| AmountError::Overflow)?);
            return parse_units(digits)?
                .checked_mul(scale)
                .map(Self)
                .ok_or(AmountError::Overflow);
        }
        let dropped = usize::try_from(-shift).unwrap_or(usize::MAX);
        if dropped >= digits.len() {
            // `digits` starts with a non-zero digit, all of it below the unit.
            return Ok(Self(1));
        }
        let (kept, below) = digits.split_at(digits.len() - dropped);
        let kept = parse_units(kept)?;
        let rounded = if below.bytes().all(|byte| byte == b'0') {
            Some(kept)
        } else {
            kept.checked_add(1)
        };
        rounded.map(Self).ok_or(AmountError::Overflow)
    }

    pub(crate) fn from_billed(billed: &BilledCostUsd) -> Result<Self, AmountError> {
        Self::parse_decimal_up(billed.as_decimal_str())
    }

    pub(crate) const fn units(self) -> u128 {
        self.0
    }

    pub(crate) fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub(super) fn from_units(units: u128) -> Self {
        Self(units)
    }
}

impl Serialize for Money {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(serde::de::Error::custom("not a count of units"));
        }
        text.parse()
            .map(Self)
            .map_err(|_| serde::de::Error::custom("not a count of units"))
    }
}

/// An optional sign and digits, held at a ceiling far past every digit count a token can have.
fn parse_exponent(text: &str) -> Result<i128, AmountError> {
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'+') => (false, &text[1..]),
        Some(b'-') => (true, &text[1..]),
        _ => (false, text),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AmountError::Malformed);
    }
    let magnitude = digits.bytes().fold(0_i128, |value, byte| {
        (value * 10 + i128::from(byte - b'0')).min(EXPONENT_CEILING)
    });
    Ok(if negative { -magnitude } else { magnitude })
}

fn parse_units(digits: &str) -> Result<u128, AmountError> {
    digits.parse().map_err(|_| AmountError::Overflow)
}

/// `numerator / 1_000_000`, rounded up, for a sum of tokens times a per-million rate.
pub(super) fn per_million_up(numerator: u128) -> Money {
    Money(numerator.div_ceil(PER_MILLION))
}

/// `tokens * rate`, refused on overflow.
pub(super) fn token_cost(tokens: u64, rate: Money) -> Result<u128, BudgetRefusal> {
    u128::from(tokens)
        .checked_mul(rate.units())
        .ok_or(BudgetRefusal::Overflow)
}

/// The most this request can cost. The split of cached and written input is not known before the
/// send, so the whole bound takes the highest input rate. Reasoning is inside the output.
pub(crate) fn reservation_amount(
    tariff: &Tariff,
    input_bound: u64,
    max_output: u64,
) -> Result<Money, BudgetRefusal> {
    let input_rate = tariff
        .input
        .max(tariff.cached_input)
        .max(tariff.cache_write);
    let numerator = token_cost(input_bound, input_rate)?
        .checked_add(token_cost(max_output, tariff.output)?)
        .ok_or(BudgetRefusal::Overflow)?;
    per_million_up(numerator)
        .checked_add(tariff.request_fee)
        .ok_or(BudgetRefusal::Overflow)
}

/// The one comparison every budget goes through: exactly at the cap passes.
pub(crate) fn within_cap(
    spent: u128,
    held: u128,
    request: u128,
    cap: u128,
) -> Result<bool, BudgetRefusal> {
    spent
        .checked_add(held)
        .and_then(|sum| sum.checked_add(request))
        .map(|total| total <= cap)
        .ok_or(BudgetRefusal::Overflow)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttemptState {
    Reserved,
    Started,
    Settled,
    Unknown,
    Released,
}

impl AttemptState {
    /// Still has a claim on the wallet, and blocks the next send.
    const fn holds(self) -> bool {
        matches!(self, Self::Reserved | Self::Started | Self::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LatchReason {
    UnknownBilling,
    OverReservation,
    InputOverBound,
    OutputOverBound,
    InterruptedAfterStart,
}

/// The first reason the wallet stopped. There is no way to clear it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Latch {
    pub(crate) reason: LatchReason,
    pub(crate) physical: PhysicalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BudgetRefusal {
    Latched,
    Unresolved,
    CostExceeded,
    TokensExceeded,
    PhysicalExceeded,
    LogicalGlobalExceeded,
    LogicalStageExceeded,
    WallExceeded,
    FinalRepairAttemptsExceeded,
    FinalRepairWallExceeded,
    InputBoundExceeded,
    OutputBoundExceeded,
    RouteNotAdmitted,
    PurposeDiffers,
    Overflow,
    NoSuchSend,
    NoSuchLogical,
    SendNotSettled,
    DuplicateSubmission,
    AlreadyBound,
    BytesChanged,
    IllegalTransition {
        from: AttemptState,
        to: AttemptState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptRecord {
    pub(crate) physical: PhysicalId,
    pub(crate) logical: LogicalId,
    pub(crate) purpose: SendPurpose,
    pub(crate) body_sha256: String,
    pub(crate) input_bound: u64,
    pub(crate) output_bound: u64,
    pub(crate) reserved: Money,
    pub(crate) state: AttemptState,
    pub(crate) unknown_reason: Option<UnknownReason>,
    pub(crate) settlement: Option<SettlementRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogicalRecord {
    pub(crate) id: LogicalId,
    pub(crate) kind: LogicalKind,
    pub(crate) source: Option<SubmissionSource>,
    pub(crate) attempt: Option<AttemptBinding>,
}

/// One send about to be made.
#[derive(Debug, Clone)]
pub(crate) struct ReserveRequest<'a> {
    pub(crate) route: &'a WireRoute,
    /// The wire bytes. Only their length and digest are kept.
    pub(crate) body: &'a [u8],
    pub(crate) max_output_tokens: u64,
    pub(crate) purpose: SendPurpose,
    pub(crate) logical: LogicalClaim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reservation {
    pub(crate) physical: PhysicalId,
    pub(crate) logical: LogicalId,
    pub(crate) amount: Money,
}

/// What the limits are asked about.
struct Demand<'a> {
    kind: &'a LogicalKind,
    opens_logical: bool,
    /// `None` for a submission, which only the host looked at: no send, no money, no tokens.
    send: Option<(Money, u64)>,
}

/// Everything a wallet is, as the ledger stores it. The counts are not stored: they are read off
/// the records, so they cannot disagree with them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WalletState {
    pub(crate) schema: String,
    pub(crate) budget_id: String,
    pub(crate) policy_sha256: String,
    pub(crate) workspace_sha256: String,
    pub(crate) profile: Option<String>,
    pub(crate) clock: ClockRecord,
    pub(crate) attempts: Vec<AttemptRecord>,
    pub(crate) logicals: Vec<LogicalRecord>,
    pub(crate) latch: Option<Latch>,
}

impl WalletState {
    pub(crate) fn new(
        policy: &RunBudgetPolicy,
        workspace_sha256: String,
        profile: Option<String>,
    ) -> Self {
        Self {
            schema: WALLET_SCHEMA.to_string(),
            budget_id: policy.budget_id.clone(),
            policy_sha256: policy.sha256.clone(),
            workspace_sha256,
            profile,
            clock: ClockRecord::default(),
            attempts: Vec::new(),
            logicals: Vec::new(),
            latch: None,
        }
    }

    pub(crate) fn attempt(&self, physical: PhysicalId) -> Option<&AttemptRecord> {
        self.attempts
            .iter()
            .find(|attempt| attempt.physical == physical)
    }

    pub(super) fn attempt_mut(&mut self, physical: PhysicalId) -> Option<&mut AttemptRecord> {
        self.attempts
            .iter_mut()
            .find(|attempt| attempt.physical == physical)
    }

    pub(crate) fn logical(&self, id: LogicalId) -> Option<&LogicalRecord> {
        self.logicals.iter().find(|logical| logical.id == id)
    }

    pub(crate) fn latch(&self) -> Option<&Latch> {
        self.latch.as_ref()
    }

    /// Set once. A later reason does not replace the first, and nothing clears it.
    pub(super) fn set_latch(&mut self, reason: LatchReason, physical: PhysicalId) {
        self.latch.get_or_insert(Latch { reason, physical });
    }

    pub(crate) fn spent(&self) -> Result<Money, BudgetRefusal> {
        self.attempts
            .iter()
            .filter_map(|attempt| attempt.settlement.as_ref())
            .try_fold(Money::ZERO, |sum, record| sum.checked_add(record.consumed))
            .ok_or(BudgetRefusal::Overflow)
    }

    pub(crate) fn held(&self) -> Result<Money, BudgetRefusal> {
        self.attempts
            .iter()
            .filter(|attempt| attempt.state.holds())
            .try_fold(Money::ZERO, |sum, attempt| {
                sum.checked_add(attempt.reserved)
            })
            .ok_or(BudgetRefusal::Overflow)
    }

    pub(crate) fn spent_tokens(&self) -> u128 {
        self.attempts
            .iter()
            .filter_map(|attempt| attempt.settlement.as_ref())
            .map(|record| u128::from(record.prompt_tokens) + u128::from(record.completion_tokens))
            .sum()
    }

    pub(crate) fn held_tokens(&self) -> u128 {
        self.attempts
            .iter()
            .filter(|attempt| attempt.state.holds())
            .map(|attempt| u128::from(attempt.input_bound) + u128::from(attempt.output_bound))
            .sum()
    }

    /// Every send but one released before it left.
    pub(crate) fn physical_used(&self) -> u64 {
        self.attempts
            .iter()
            .filter(|attempt| attempt.state != AttemptState::Released)
            .count() as u64
    }

    pub(crate) fn logical_global_used(&self) -> u64 {
        self.logicals.len() as u64
    }

    pub(crate) fn logical_stage_used(&self, stage: &str) -> u64 {
        self.logicals
            .iter()
            .filter(|logical| {
                matches!(&logical.kind, LogicalKind::Stage { stage: held } if held == stage)
            })
            .count() as u64
    }

    pub(crate) fn final_repair_used(&self) -> u64 {
        self.logicals
            .iter()
            .filter(|logical| logical.kind == LogicalKind::FinalRepair)
            .count() as u64
    }

    /// A latch or an unresolved send stops a send and a success alike.
    pub(crate) fn success_gate(&self) -> Result<(), BudgetRefusal> {
        if self.latch.is_some() {
            return Err(BudgetRefusal::Latched);
        }
        if self.attempts.iter().any(|attempt| attempt.state.holds()) {
            return Err(BudgetRefusal::Unresolved);
        }
        Ok(())
    }

    /// Every limit, for one send or one submission. Exactly at a limit passes.
    fn check_limits(
        &self,
        policy: &RunBudgetPolicy,
        demand: &Demand<'_>,
        elapsed_ms: u64,
    ) -> Result<(), BudgetRefusal> {
        let limits = &policy.limits;
        if elapsed_ms > limits.wall_ms {
            return Err(BudgetRefusal::WallExceeded);
        }
        if *demand.kind == LogicalKind::FinalRepair {
            if demand.opens_logical && self.final_repair_used() >= limits.final_repair_attempts {
                return Err(BudgetRefusal::FinalRepairAttemptsExceeded);
            }
            let began = self.clock.final_repair_started_ms.unwrap_or(elapsed_ms);
            if elapsed_ms.saturating_sub(began) > limits.final_repair_ms {
                return Err(BudgetRefusal::FinalRepairWallExceeded);
            }
        }
        if demand.opens_logical {
            if self.logical_global_used() >= limits.logical_global {
                return Err(BudgetRefusal::LogicalGlobalExceeded);
            }
            if let LogicalKind::Stage { stage } = demand.kind
                && self.logical_stage_used(stage) >= limits.logical_per_stage
            {
                return Err(BudgetRefusal::LogicalStageExceeded);
            }
        }
        let Some((amount, tokens)) = demand.send else {
            return Ok(());
        };
        if self.physical_used() >= limits.physical_sends {
            return Err(BudgetRefusal::PhysicalExceeded);
        }
        if !within_cap(
            self.spent_tokens(),
            self.held_tokens(),
            u128::from(tokens),
            u128::from(limits.tokens),
        )? {
            return Err(BudgetRefusal::TokensExceeded);
        }
        if !within_cap(
            self.spent()?.units(),
            self.held()?.units(),
            amount.units(),
            limits.cost.units(),
        )? {
            return Err(BudgetRefusal::CostExceeded);
        }
        Ok(())
    }

    /// Opens a logical request of this kind. The caller has judged the limits.
    fn open_logical(
        &mut self,
        kind: LogicalKind,
        source: Option<SubmissionSource>,
        elapsed_ms: u64,
    ) -> LogicalId {
        let id = LogicalId(self.logicals.len() as u64 + 1);
        if kind == LogicalKind::FinalRepair {
            self.clock.final_repair_started_ms.get_or_insert(elapsed_ms);
        }
        self.logicals.push(LogicalRecord {
            id,
            kind,
            source,
            attempt: None,
        });
        id
    }

    /// A reservation of the request's bytes, on the record before anything is sent.
    pub(crate) fn reserve(
        &mut self,
        policy: &RunBudgetPolicy,
        request: &ReserveRequest<'_>,
        clock: &mut RunClock,
    ) -> Result<Reservation, BudgetRefusal> {
        self.success_gate()?;
        if !policy.route.admits(request.route) {
            return Err(BudgetRefusal::RouteNotAdmitted);
        }
        let input_bound = u64::try_from(request.body.len())
            .ok()
            .and_then(|bytes| bytes.checked_add(policy.input_overhead))
            .ok_or(BudgetRefusal::Overflow)?;
        if input_bound > policy.max_input_tokens {
            return Err(BudgetRefusal::InputBoundExceeded);
        }
        if request.max_output_tokens > policy.max_output_tokens {
            return Err(BudgetRefusal::OutputBoundExceeded);
        }
        let tokens = input_bound
            .checked_add(request.max_output_tokens)
            .ok_or(BudgetRefusal::Overflow)?;
        let amount = reservation_amount(&policy.tariff, input_bound, request.max_output_tokens)?;
        let (kind, existing) = match &request.logical {
            LogicalClaim::New(kind) => (kind.clone(), None),
            LogicalClaim::Continue(id) => {
                let logical = self.logical(*id).ok_or(BudgetRefusal::NoSuchLogical)?;
                (logical.kind.clone(), Some(*id))
            }
        };
        if (kind == LogicalKind::FinalRepair) != (request.purpose == SendPurpose::FinalRepair) {
            return Err(BudgetRefusal::PurposeDiffers);
        }
        let elapsed_ms = clock.observe(&mut self.clock);
        self.check_limits(
            policy,
            &Demand {
                kind: &kind,
                opens_logical: existing.is_none(),
                send: Some((amount, tokens)),
            },
            elapsed_ms,
        )?;
        clock.begin(&mut self.clock);
        let logical = match existing {
            Some(id) => id,
            None => self.open_logical(kind, None, elapsed_ms),
        };
        let physical = PhysicalId(self.attempts.len() as u64 + 1);
        self.attempts.push(AttemptRecord {
            physical,
            logical,
            purpose: request.purpose,
            body_sha256: sha256_hex(request.body),
            input_bound,
            output_bound: request.max_output_tokens,
            reserved: amount,
            state: AttemptState::Reserved,
            unknown_reason: None,
            settlement: None,
        });
        Ok(Reservation {
            physical,
            logical,
            amount,
        })
    }

    /// The send in `from`, or the refusal to take it to `to`.
    fn attempt_in(
        &mut self,
        physical: PhysicalId,
        from: AttemptState,
        to: AttemptState,
    ) -> Result<&mut AttemptRecord, BudgetRefusal> {
        let attempt = self
            .attempt_mut(physical)
            .ok_or(BudgetRefusal::NoSuchSend)?;
        if attempt.state != from {
            return Err(BudgetRefusal::IllegalTransition {
                from: attempt.state,
                to,
            });
        }
        Ok(attempt)
    }

    /// `reserved` to `started`, for the same bytes that were reserved.
    pub(crate) fn start(&mut self, physical: PhysicalId, body: &[u8]) -> Result<(), BudgetRefusal> {
        let attempt = self.attempt_in(physical, AttemptState::Reserved, AttemptState::Started)?;
        if attempt.body_sha256 != sha256_hex(body) {
            return Err(BudgetRefusal::BytesChanged);
        }
        attempt.state = AttemptState::Started;
        Ok(())
    }

    /// `reserved` to `released`: the one way a reservation goes back, and nothing has left.
    pub(crate) fn release(&mut self, physical: PhysicalId) -> Result<(), BudgetRefusal> {
        let attempt = self.attempt_in(physical, AttemptState::Reserved, AttemptState::Released)?;
        attempt.state = AttemptState::Released;
        Ok(())
    }

    /// `started` to `unknown`. The reservation stays held and the wallet stops.
    pub(crate) fn mark_unknown(
        &mut self,
        physical: PhysicalId,
        reason: UnknownReason,
    ) -> Result<(), BudgetRefusal> {
        let attempt = self.attempt_in(physical, AttemptState::Started, AttemptState::Unknown)?;
        attempt.state = AttemptState::Unknown;
        attempt.unknown_reason = Some(reason);
        self.set_latch(LatchReason::UnknownBilling, physical);
        Ok(())
    }

    /// One submission the host looked at. It costs no send, and it does not escape the counts or
    /// the clock. Its `source`, when it came from a response, must be a settled one.
    pub(crate) fn record_submission(
        &mut self,
        policy: &RunBudgetPolicy,
        kind: LogicalKind,
        source: Option<SubmissionSource>,
        clock: &mut RunClock,
    ) -> Result<LogicalId, BudgetRefusal> {
        self.success_gate()?;
        if let Some(source) = source {
            let attempt = self
                .attempt(source.physical)
                .ok_or(BudgetRefusal::NoSuchSend)?;
            if attempt.state != AttemptState::Settled {
                return Err(BudgetRefusal::SendNotSettled);
            }
            if self
                .logicals
                .iter()
                .any(|logical| logical.source == Some(source))
            {
                return Err(BudgetRefusal::DuplicateSubmission);
            }
        }
        let elapsed_ms = clock.observe(&mut self.clock);
        self.check_limits(
            policy,
            &Demand {
                kind: &kind,
                opens_logical: true,
                send: None,
            },
            elapsed_ms,
        )?;
        Ok(self.open_logical(kind, source, elapsed_ms))
    }

    /// Ties G4's attempt to a submission after the fact, once. The same value again is no change.
    pub(crate) fn bind_attempt(
        &mut self,
        logical: LogicalId,
        binding: AttemptBinding,
    ) -> Result<(), BudgetRefusal> {
        let logical = self
            .logicals
            .iter_mut()
            .find(|record| record.id == logical)
            .ok_or(BudgetRefusal::NoSuchLogical)?;
        match &logical.attempt {
            Some(held) if *held != binding => Err(BudgetRefusal::AlreadyBound),
            _ => {
                logical.attempt = Some(binding);
                Ok(())
            }
        }
    }

    /// What a process finds of one that ended: a reservation that never left goes back, and a
    /// started send may have left, so it is unknown and the wallet stops.
    pub(crate) fn recover_after_process_end(&mut self) {
        let mut interrupted = None;
        for attempt in &mut self.attempts {
            match attempt.state {
                AttemptState::Reserved => attempt.state = AttemptState::Released,
                AttemptState::Started => {
                    attempt.state = AttemptState::Unknown;
                    attempt.unknown_reason = Some(UnknownReason::InterruptedAfterStart);
                    interrupted = Some(attempt.physical);
                }
                AttemptState::Settled | AttemptState::Unknown | AttemptState::Released => {}
            }
        }
        if let Some(physical) = interrupted {
            self.set_latch(LatchReason::InterruptedAfterStart, physical);
        }
    }

    /// Whether these records can be what this code wrote under this policy. Finds damage: amounts
    /// are computed again, and the states have to agree with what they carry.
    pub(crate) fn is_consistent(&self, policy: &RunBudgetPolicy) -> bool {
        self.schema == WALLET_SCHEMA
            && self.logicals_are_consistent()
            && self.clock_is_consistent()
            && self
                .attempts
                .iter()
                .enumerate()
                .all(|(index, attempt)| self.attempt_is_consistent(policy, index, attempt))
            && self
                .attempts
                .iter()
                .filter(|attempt| attempt.state.holds())
                .count()
                <= 1
            && self.spent().is_ok()
            && self.held().is_ok()
    }

    fn logicals_are_consistent(&self) -> bool {
        self.logicals.iter().enumerate().all(|(index, logical)| {
            logical.id.0 == index as u64 + 1
                && logical.source.is_none_or(|source| {
                    self.attempt(source.physical)
                        .is_some_and(|attempt| attempt.state == AttemptState::Settled)
                })
        })
    }

    fn clock_is_consistent(&self) -> bool {
        self.clock.started_wall_ms.is_some() != self.attempts.is_empty()
            && self.clock.final_repair_started_ms.is_some() == (self.final_repair_used() > 0)
    }

    fn attempt_is_consistent(
        &self,
        policy: &RunBudgetPolicy,
        index: usize,
        attempt: &AttemptRecord,
    ) -> bool {
        let identified = attempt.physical.0 == index as u64 + 1
            && self.logical(attempt.logical).is_some()
            && attempt.input_bound <= policy.max_input_tokens
            && attempt.output_bound <= policy.max_output_tokens
            && reservation_amount(&policy.tariff, attempt.input_bound, attempt.output_bound)
                == Ok(attempt.reserved);
        identified
            && match (attempt.state, &attempt.settlement) {
                (AttemptState::Reserved | AttemptState::Started | AttemptState::Released, None) => {
                    attempt.unknown_reason.is_none()
                }
                (AttemptState::Unknown, None) => {
                    attempt.unknown_reason.is_some() && self.latch.is_some()
                }
                (AttemptState::Settled, Some(record)) => {
                    // Only a late settlement carries the reason it was unknown.
                    record.late == attempt.unknown_reason.is_some()
                        && (!record.late || self.latch.is_some())
                        && Self::settlement_is_consistent(policy, record)
                        && (self.latch.is_some()
                            || (record.consumed <= attempt.reserved
                                && record.prompt_tokens <= attempt.input_bound
                                && record.completion_tokens <= attempt.output_bound))
                }
                _ => false,
            }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::clock::RunClock;
    use super::super::identity::{
        AttemptBinding, LogicalClaim, LogicalKind, PhysicalId, ResponseIdentity, SendPurpose,
    };
    use super::super::policy::RunBudgetPolicy;
    use super::super::settlement::{KnownBilling, ReportedUsage, UnknownReason};
    use super::super::test_support::{
        BODY, FakeClock, general, policy, policy_with, request, route, stage,
    };
    use super::{
        AmountError, AttemptState, BudgetRefusal, LatchReason, Money, Reservation, WalletState,
        reservation_amount, within_cap,
    };
    use crate::providers::BilledCostUsd;

    const UNIT: u128 = 1_000_000_000_000_000_000;

    fn money(text: &str) -> Money {
        Money::parse_decimal_up(text).unwrap()
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

    struct World {
        policy: RunBudgetPolicy,
        source: std::sync::Arc<FakeClock>,
        clock: RunClock,
        wallet: WalletState,
    }

    impl World {
        fn new(policy: RunBudgetPolicy) -> Self {
            let source = FakeClock::at(5_000);
            let clock = RunClock::new(source.clone());
            let wallet = WalletState::new(&policy, "workspace".to_string(), None);
            Self {
                policy,
                source,
                clock,
                wallet,
            }
        }

        fn reserve(&mut self, claim: LogicalClaim) -> Result<Reservation, BudgetRefusal> {
            let wire = route();
            let body = request(&wire, claim);
            self.wallet.reserve(&self.policy, &body, &mut self.clock)
        }

        fn reserve_purpose(
            &mut self,
            claim: LogicalClaim,
            purpose: SendPurpose,
        ) -> Result<Reservation, BudgetRefusal> {
            let wire = route();
            let mut body = request(&wire, claim);
            body.purpose = purpose;
            self.wallet.reserve(&self.policy, &body, &mut self.clock)
        }

        /// A whole send, settled well inside its reservation.
        fn send(&mut self, claim: LogicalClaim) -> Result<Reservation, BudgetRefusal> {
            let reservation = self.reserve(claim)?;
            self.wallet.start(reservation.physical, BODY).unwrap();
            self.wallet
                .settle(&self.policy, reservation.physical, &known(100, 40))
                .unwrap();
            Ok(reservation)
        }

        fn in_state(&mut self, state: AttemptState) -> PhysicalId {
            let id = self.reserve(general()).unwrap().physical;
            match state {
                AttemptState::Reserved => {}
                AttemptState::Started => self.wallet.start(id, BODY).unwrap(),
                AttemptState::Released => self.wallet.release(id).unwrap(),
                AttemptState::Unknown => {
                    self.wallet.start(id, BODY).unwrap();
                    self.wallet
                        .mark_unknown(id, UnknownReason::Timeout)
                        .unwrap();
                }
                AttemptState::Settled => {
                    self.wallet.start(id, BODY).unwrap();
                    self.wallet
                        .settle(&self.policy, id, &known(100, 40))
                        .unwrap();
                }
            }
            id
        }
    }

    fn final_repair() -> LogicalClaim {
        LogicalClaim::New(LogicalKind::FinalRepair)
    }

    #[test]
    fn zero_is_only_a_written_zero() {
        for zero in ["0", "0.0", "0e5", "0.000", "0E+9", "0e-400"] {
            assert_eq!(money(zero), Money::ZERO, "{zero}");
        }
    }

    #[test]
    fn an_amount_below_the_unit_rounds_up_and_is_never_zero() {
        for tiny in [
            "1e-400",
            "1e-19",
            "0.0000000000000000001",
            "1E-99999999999999999999",
        ] {
            assert_eq!(money(tiny).units(), 1, "{tiny}");
        }
    }

    #[test]
    fn amounts_are_exact_in_units_and_round_up_only_past_the_last_place() {
        assert_eq!(money("1").units(), UNIT);
        assert_eq!(money("1.5").units(), UNIT + UNIT / 2);
        assert_eq!(money("1e2").units(), 100 * UNIT);
        assert_eq!(money("15e-1").units(), UNIT + UNIT / 2);
        assert_eq!(money("0.000000000000000001").units(), 1);
        assert_eq!(money("0.0000000000000000011").units(), 2);
        assert_eq!(money("0.0000000000000000010").units(), 1);
        assert_eq!(money("0.1").units(), UNIT / 10);
        // A float would round this one.
        assert_eq!(money("0.100000000000000006").units(), UNIT / 10 + 6);
    }

    #[test]
    fn an_amount_past_the_range_is_refused() {
        for huge in [
            "1e21",
            "340282366920938463464",
            "1e400",
            "1e99999999999999999999",
        ] {
            assert_eq!(
                Money::parse_decimal_up(huge),
                Err(AmountError::Overflow),
                "{huge}"
            );
        }
        // The largest that fits, and one unit more.
        assert_eq!(
            money("340282366920938463463.374607431768211455").units(),
            u128::MAX
        );
        assert_eq!(
            Money::parse_decimal_up("340282366920938463463.3746074317682114551"),
            Err(AmountError::Overflow)
        );
    }

    #[test]
    fn anything_but_the_unsigned_grammar_is_refused() {
        for bad in [
            "", " 1", "1 ", "-1", "+1", "-0", "01", ".5", "1.", "1e", "1e+", "e5", "1.2.3", "NaN",
            "inf", "0x1", "1_000",
        ] {
            assert_eq!(
                Money::parse_decimal_up(bad),
                Err(AmountError::Malformed),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_billed_amount_goes_in_as_its_decimal_text() {
        let billed = BilledCostUsd::from_json_token("1e-400").unwrap();
        assert_eq!(Money::from_billed(&billed).unwrap().units(), 1);
        let billed = BilledCostUsd::from_json_token("0e5").unwrap();
        assert_eq!(Money::from_billed(&billed).unwrap(), Money::ZERO);
        let largest = BilledCostUsd::from_json_token(BilledCostUsd::MAX).unwrap();
        assert_eq!(
            Money::from_billed(&largest).unwrap().units(),
            9_007_199_254_740_991 * UNIT
        );
    }

    #[test]
    fn money_survives_the_ledger_and_refuses_anything_but_digits() {
        let text = serde_json::to_string(&Money::from_units(u128::MAX)).unwrap();
        assert_eq!(text, format!("\"{}\"", u128::MAX));
        assert_eq!(
            serde_json::from_str::<Money>(&text).unwrap().units(),
            u128::MAX
        );
        for bad in [
            "\"-1\"",
            "\"1.5\"",
            "\"\"",
            "1",
            "\"340282366920938463463374607431768211456\"",
        ] {
            assert!(serde_json::from_str::<Money>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_reservation_formula_is_exact() {
        let tariff = policy().tariff;
        // (110 * 2 + 50 * 10) per million.
        assert_eq!(
            reservation_amount(&tariff, 110, 50).unwrap(),
            money("0.00072")
        );
    }

    #[test]
    fn the_reservation_takes_the_highest_input_rate() {
        let cached = policy_with(|v| v["tariff"]["cached_input_per_million"] = json!("3")).tariff;
        assert_eq!(
            reservation_amount(&cached, 100, 0).unwrap(),
            money("0.0003")
        );
        let write = policy_with(|v| v["tariff"]["cache_write_per_million"] = json!("4")).tariff;
        assert_eq!(reservation_amount(&write, 100, 0).unwrap(), money("0.0004"));
        let input = policy_with(|v| v["tariff"]["input_per_million"] = json!("5")).tariff;
        assert_eq!(reservation_amount(&input, 100, 0).unwrap(), money("0.0005"));
    }

    #[test]
    fn the_reservation_adds_the_fee_and_rounds_up() {
        let fee = policy_with(|v| v["tariff"]["request_fee"] = json!("0.25")).tariff;
        assert_eq!(reservation_amount(&fee, 1, 0).unwrap(), money("0.250002"));
        let tiny = policy_with(|v| {
            v["tariff"]["input_per_million"] = json!("0.0000000000001");
            v["tariff"]["cached_input_per_million"] = json!("0");
            v["tariff"]["cache_write_per_million"] = json!("0");
        })
        .tariff;
        assert_eq!(reservation_amount(&tiny, 1, 0).unwrap().units(), 1);
    }

    #[test]
    fn a_reservation_that_overflows_is_refused() {
        let tariff = policy_with(|v| v["tariff"]["output_per_million"] = json!("1000")).tariff;
        assert_eq!(
            reservation_amount(&tariff, 0, u64::MAX),
            Err(BudgetRefusal::Overflow)
        );
    }

    #[test]
    fn the_cap_counts_spent_and_held_and_request_alike() {
        assert_eq!(within_cap(1, 2, 3, 6), Ok(true));
        assert_eq!(within_cap(1, 2, 3, 5), Ok(false));
        assert_eq!(within_cap(2, 2, 3, 6), Ok(false));
        assert_eq!(within_cap(1, 3, 3, 6), Ok(false));
        assert_eq!(within_cap(1, 2, 4, 6), Ok(false));
        assert_eq!(
            within_cap(u128::MAX, 1, 0, u128::MAX),
            Err(BudgetRefusal::Overflow)
        );
    }

    #[test]
    fn a_cost_cap_passes_exactly_and_refuses_one_unit_short() {
        let mut exact = World::new(policy_with(|v| v["limits"]["cost_usd"] = json!("0.00072")));
        assert!(exact.reserve(general()).is_ok());
        let mut short = World::new(policy_with(|v| {
            v["limits"]["cost_usd"] = json!("0.000719999999999999");
        }));
        assert_eq!(short.reserve(general()), Err(BudgetRefusal::CostExceeded));
        assert!(short.wallet.attempts.is_empty());
    }

    #[test]
    fn what_was_spent_counts_against_the_next_reservation() {
        // The first send consumes 0.0006; the second needs 0.00072.
        let mut exact = World::new(policy_with(|v| v["limits"]["cost_usd"] = json!("0.00132")));
        exact.send(general()).unwrap();
        assert!(exact.reserve(general()).is_ok());
        let mut short = World::new(policy_with(|v| {
            v["limits"]["cost_usd"] = json!("0.001319999999999999");
        }));
        short.send(general()).unwrap();
        assert_eq!(short.reserve(general()), Err(BudgetRefusal::CostExceeded));
    }

    #[test]
    fn a_token_cap_passes_exactly_and_refuses_one_short() {
        // 100 + 10 + 50.
        let mut exact = World::new(policy_with(|v| v["limits"]["tokens"] = json!(160)));
        assert!(exact.reserve(general()).is_ok());
        let mut short = World::new(policy_with(|v| v["limits"]["tokens"] = json!(159)));
        assert_eq!(short.reserve(general()), Err(BudgetRefusal::TokensExceeded));
    }

    #[test]
    fn tokens_spent_count_against_the_next_reservation() {
        // The first send used 140 tokens.
        let mut exact = World::new(policy_with(|v| v["limits"]["tokens"] = json!(300)));
        exact.send(general()).unwrap();
        assert!(exact.reserve(general()).is_ok());
        let mut short = World::new(policy_with(|v| v["limits"]["tokens"] = json!(299)));
        short.send(general()).unwrap();
        assert_eq!(short.reserve(general()), Err(BudgetRefusal::TokensExceeded));
    }

    #[test]
    fn a_physical_cap_passes_exactly_and_refuses_one_past() {
        let mut world = World::new(policy_with(|v| v["limits"]["physical_sends"] = json!(2)));
        world.send(general()).unwrap();
        world.send(general()).unwrap();
        assert_eq!(
            world.reserve(general()),
            Err(BudgetRefusal::PhysicalExceeded)
        );
        assert_eq!(world.wallet.physical_used(), 2);
    }

    #[test]
    fn a_global_logical_cap_passes_exactly_and_refuses_one_past() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_global"] = json!(2)));
        world.send(stage("one")).unwrap();
        world.send(general()).unwrap();
        assert_eq!(
            world.reserve(stage("two")),
            Err(BudgetRefusal::LogicalGlobalExceeded)
        );
        assert_eq!(world.wallet.logical_global_used(), 2);
    }

    #[test]
    fn a_stage_cap_passes_exactly_refuses_one_past_and_leaves_other_stages_alone() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_per_stage"] = json!(2)));
        world.send(stage("one")).unwrap();
        world.send(stage("one")).unwrap();
        assert_eq!(
            world.reserve(stage("one")),
            Err(BudgetRefusal::LogicalStageExceeded)
        );
        assert_eq!(world.wallet.logical_stage_used("one"), 2);
        assert_eq!(world.wallet.logical_stage_used("two"), 0);
        world.send(stage("two")).unwrap();
        // A send with no stage is not in a stage's count.
        world.send(general()).unwrap();
        assert_eq!(world.wallet.logical_stage_used("two"), 1);
    }

    #[test]
    fn continuing_a_logical_request_counts_no_new_one() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_global"] = json!(1)));
        let first = world.send(general()).unwrap();
        world.send(LogicalClaim::Continue(first.logical)).unwrap();
        assert_eq!(world.wallet.logical_global_used(), 1);
        assert_eq!(world.wallet.physical_used(), 2);
    }

    #[test]
    fn continuing_a_logical_request_that_does_not_exist_is_refused() {
        let mut world = World::new(policy());
        let first = world.send(general()).unwrap();
        let other = super::LogicalId(first.logical.0 + 1);
        assert_eq!(
            world.reserve(LogicalClaim::Continue(other)),
            Err(BudgetRefusal::NoSuchLogical)
        );
    }

    #[test]
    fn the_wall_limit_passes_exactly_and_refuses_one_past() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.send(general()).unwrap();
        world.source.advance(1_000);
        world.send(general()).unwrap();
        world.source.advance(1);
        assert_eq!(world.reserve(general()), Err(BudgetRefusal::WallExceeded));
    }

    #[test]
    fn the_run_clock_starts_at_the_first_reservation_not_before() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.source.advance(50_000);
        world.send(general()).unwrap();
        assert_eq!(world.wallet.clock.started_wall_ms, Some(55_000));
        world.source.advance(1_000);
        world.send(general()).unwrap();
        assert_eq!(world.wallet.clock.started_wall_ms, Some(55_000));
    }

    #[test]
    fn a_refused_first_reservation_does_not_start_the_clock() {
        let mut world = World::new(policy_with(|v| v["limits"]["tokens"] = json!(1)));
        assert!(world.reserve(general()).is_err());
        assert_eq!(world.wallet.clock.started_wall_ms, None);
    }

    #[test]
    fn final_repair_attempts_pass_exactly_and_refuse_one_past() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(2)
        }));
        world
            .reserve_purpose(final_repair(), SendPurpose::FinalRepair)
            .and_then(|r| world.wallet.start(r.physical, BODY).map(|()| r))
            .map(|r| {
                world
                    .wallet
                    .settle(&world.policy, r.physical, &known(1, 1))
                    .unwrap();
            })
            .unwrap();
        world
            .reserve_purpose(final_repair(), SendPurpose::FinalRepair)
            .and_then(|r| world.wallet.start(r.physical, BODY).map(|()| r))
            .map(|r| {
                world
                    .wallet
                    .settle(&world.policy, r.physical, &known(1, 1))
                    .unwrap();
            })
            .unwrap();
        assert_eq!(
            world.reserve_purpose(final_repair(), SendPurpose::FinalRepair),
            Err(BudgetRefusal::FinalRepairAttemptsExceeded)
        );
        assert_eq!(world.wallet.final_repair_used(), 2);
    }

    #[test]
    fn the_final_repair_window_passes_exactly_and_refuses_one_past_from_its_own_start() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_seconds"] = json!(1)
        }));
        // Time before the first repair is not in its window.
        world.send(general()).unwrap();
        world.source.advance(50_000);
        let repair = world
            .reserve_purpose(final_repair(), SendPurpose::FinalRepair)
            .unwrap();
        world.wallet.start(repair.physical, BODY).unwrap();
        world
            .wallet
            .settle(&world.policy, repair.physical, &known(1, 1))
            .unwrap();
        world.source.advance(1_000);
        let again = world
            .reserve_purpose(
                LogicalClaim::Continue(repair.logical),
                SendPurpose::FinalRepair,
            )
            .unwrap();
        world.wallet.start(again.physical, BODY).unwrap();
        world
            .wallet
            .settle(&world.policy, again.physical, &known(1, 1))
            .unwrap();
        world.source.advance(1);
        assert_eq!(
            world.reserve_purpose(
                LogicalClaim::Continue(repair.logical),
                SendPurpose::FinalRepair
            ),
            Err(BudgetRefusal::FinalRepairWallExceeded)
        );
        assert_eq!(
            world.reserve_purpose(final_repair(), SendPurpose::FinalRepair),
            Err(BudgetRefusal::FinalRepairWallExceeded)
        );
    }

    #[test]
    fn a_global_limit_that_runs_out_first_stops_a_final_repair() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(5);
            v["limits"]["physical_sends"] = json!(1);
        }));
        world.send(general()).unwrap();
        assert_eq!(
            world.reserve_purpose(final_repair(), SendPurpose::FinalRepair),
            Err(BudgetRefusal::PhysicalExceeded)
        );
    }

    #[test]
    fn a_final_repair_has_to_say_so_both_ways() {
        let mut world = World::new(policy());
        assert_eq!(
            world.reserve_purpose(final_repair(), SendPurpose::Executor),
            Err(BudgetRefusal::PurposeDiffers)
        );
        assert_eq!(
            world.reserve_purpose(general(), SendPurpose::FinalRepair),
            Err(BudgetRefusal::PurposeDiffers)
        );
        let general_one = world.send(general()).unwrap();
        assert_eq!(
            world.reserve_purpose(
                LogicalClaim::Continue(general_one.logical),
                SendPurpose::FinalRepair
            ),
            Err(BudgetRefusal::PurposeDiffers)
        );
    }

    #[test]
    fn a_route_the_policy_does_not_declare_is_refused_before_any_record() {
        let mut world = World::new(policy());
        let mut wire = route();
        wire.model = "another".to_string();
        let body = request(&wire, general());
        assert_eq!(
            world.wallet.reserve(&world.policy, &body, &mut world.clock),
            Err(BudgetRefusal::RouteNotAdmitted)
        );
        assert!(world.wallet.attempts.is_empty());
        assert!(world.wallet.logicals.is_empty());
    }

    #[test]
    fn the_input_bound_passes_exactly_and_refuses_one_past() {
        let mut exact = World::new(policy_with(|v| v["max_input_tokens"] = json!(110)));
        assert!(exact.reserve(general()).is_ok());
        let mut short = World::new(policy_with(|v| v["max_input_tokens"] = json!(109)));
        assert_eq!(
            short.reserve(general()),
            Err(BudgetRefusal::InputBoundExceeded)
        );
    }

    #[test]
    fn an_output_above_the_policy_is_refused_and_a_lower_one_reserves_less() {
        let mut world = World::new(policy());
        let wire = route();
        let mut body = request(&wire, general());
        body.max_output_tokens = 51;
        assert_eq!(
            world.wallet.reserve(&world.policy, &body, &mut world.clock),
            Err(BudgetRefusal::OutputBoundExceeded)
        );
        body.max_output_tokens = 10;
        let reservation = world
            .wallet
            .reserve(&world.policy, &body, &mut world.clock)
            .unwrap();
        // (110 * 2 + 10 * 10) per million.
        assert_eq!(reservation.amount, money("0.00032"));
    }

    #[test]
    fn the_state_table_holds_and_nothing_else_moves() {
        use AttemptState::{Released, Reserved, Settled, Started, Unknown};
        let cases: [(AttemptState, [bool; 4]); 5] = [
            // start, release, unknown, settle
            (Reserved, [true, true, false, false]),
            (Started, [false, false, true, true]),
            (Unknown, [false, false, false, true]),
            (Settled, [false; 4]),
            (Released, [false; 4]),
        ];
        for (state, allowed) in cases {
            for (op, allow) in allowed.into_iter().enumerate() {
                let mut world = World::new(policy());
                let id = world.in_state(state);
                let before = world.wallet.clone();
                let target = [Started, Released, Unknown, Settled][op];
                let result = match op {
                    0 => world.wallet.start(id, BODY),
                    1 => world.wallet.release(id),
                    2 => world.wallet.mark_unknown(id, UnknownReason::Timeout),
                    _ => world
                        .wallet
                        .settle(&world.policy, id, &known(100, 40))
                        .map(|_| ()),
                };
                if allow {
                    assert_eq!(result, Ok(()), "{state:?} to {target:?}");
                    assert_eq!(world.wallet.attempt(id).unwrap().state, target);
                } else {
                    assert_eq!(
                        result,
                        Err(BudgetRefusal::IllegalTransition {
                            from: state,
                            to: target,
                        }),
                        "{state:?} to {target:?}"
                    );
                    assert_eq!(world.wallet, before, "{state:?} to {target:?}");
                }
            }
        }
    }

    #[test]
    fn an_operation_on_a_send_that_does_not_exist_is_refused() {
        let mut world = World::new(policy());
        let id = PhysicalId(99);
        assert_eq!(world.wallet.start(id, BODY), Err(BudgetRefusal::NoSuchSend));
        assert_eq!(world.wallet.release(id), Err(BudgetRefusal::NoSuchSend));
        assert_eq!(
            world.wallet.mark_unknown(id, UnknownReason::Timeout),
            Err(BudgetRefusal::NoSuchSend)
        );
        assert_eq!(
            world
                .wallet
                .settle(&world.policy, id, &known(1, 1))
                .map(|_| ()),
            Err(BudgetRefusal::NoSuchSend)
        );
    }

    #[test]
    fn a_start_with_other_bytes_is_refused() {
        let mut world = World::new(policy());
        let id = world.reserve(general()).unwrap().physical;
        let before = world.wallet.clone();
        let other = [b'y'; 100];
        assert_eq!(
            world.wallet.start(id, &other),
            Err(BudgetRefusal::BytesChanged)
        );
        assert_eq!(world.wallet, before);
        assert_eq!(world.wallet.start(id, BODY), Ok(()));
    }

    #[test]
    fn a_reservation_holds_and_a_release_gives_back_money_tokens_and_the_send() {
        let mut world = World::new(policy());
        let id = world.reserve(general()).unwrap().physical;
        assert_eq!(world.wallet.held().unwrap(), money("0.00072"));
        assert_eq!(world.wallet.held_tokens(), 160);
        assert_eq!(world.wallet.physical_used(), 1);
        world.wallet.release(id).unwrap();
        assert_eq!(world.wallet.held().unwrap(), Money::ZERO);
        assert_eq!(world.wallet.held_tokens(), 0);
        assert_eq!(world.wallet.physical_used(), 0);
        assert_eq!(world.wallet.spent().unwrap(), Money::ZERO);
        assert_eq!(world.wallet.success_gate(), Ok(()));
    }

    #[test]
    fn a_release_does_not_give_back_the_logical_count_or_a_final_repair() {
        let mut world = World::new(policy());
        let id = world
            .reserve_purpose(final_repair(), SendPurpose::FinalRepair)
            .unwrap()
            .physical;
        world.wallet.release(id).unwrap();
        assert_eq!(world.wallet.logical_global_used(), 1);
        assert_eq!(world.wallet.final_repair_used(), 1);
    }

    #[test]
    fn an_unknown_send_holds_its_reservation_and_stops_the_wallet() {
        for reason in [
            UnknownReason::CostMissing,
            UnknownReason::UsageMissing,
            UnknownReason::UsageInconsistent,
            UnknownReason::BodyTruncated,
            UnknownReason::ParseFailed,
            UnknownReason::HttpError,
            UnknownReason::Timeout,
            UnknownReason::Cancelled,
            UnknownReason::SendError,
        ] {
            let mut world = World::new(policy());
            let id = world.reserve(general()).unwrap().physical;
            world.wallet.start(id, BODY).unwrap();
            world.wallet.mark_unknown(id, reason).unwrap();
            assert_eq!(world.wallet.held().unwrap(), money("0.00072"), "{reason:?}");
            assert_eq!(world.wallet.spent().unwrap(), Money::ZERO, "{reason:?}");
            assert_eq!(
                world.wallet.attempt(id).unwrap().unknown_reason,
                Some(reason)
            );
            let latch = world.wallet.latch().unwrap();
            assert_eq!(latch.reason, LatchReason::UnknownBilling);
            assert_eq!(latch.physical, id);
            assert_eq!(world.reserve(general()), Err(BudgetRefusal::Latched));
            assert_eq!(world.wallet.success_gate(), Err(BudgetRefusal::Latched));
        }
    }

    #[test]
    fn an_unresolved_send_blocks_the_next_send_and_success() {
        for state in [AttemptState::Reserved, AttemptState::Started] {
            let mut world = World::new(policy());
            world.in_state(state);
            assert_eq!(
                world.reserve(general()),
                Err(BudgetRefusal::Unresolved),
                "{state:?}"
            );
            assert_eq!(world.wallet.success_gate(), Err(BudgetRefusal::Unresolved));
        }
    }

    #[test]
    fn only_a_settled_or_released_wallet_can_succeed() {
        let mut world = World::new(policy());
        assert_eq!(world.wallet.success_gate(), Ok(()));
        world.in_state(AttemptState::Settled);
        assert_eq!(world.wallet.success_gate(), Ok(()));
        world.in_state(AttemptState::Released);
        assert_eq!(world.wallet.success_gate(), Ok(()));
    }

    #[test]
    fn the_first_latch_stays_and_nothing_clears_it() {
        let mut world = World::new(policy());
        let id = world.in_state(AttemptState::Unknown);
        let first = *world.wallet.latch().unwrap();
        // Well past the prompt bound, which would latch by itself.
        world
            .wallet
            .settle(&world.policy, id, &known(500, 40))
            .unwrap();
        assert_eq!(*world.wallet.latch().unwrap(), first);
        world.source.advance(10_000_000);
        assert_eq!(world.reserve(general()), Err(BudgetRefusal::Latched));
        assert_eq!(world.wallet.success_gate(), Err(BudgetRefusal::Latched));
    }

    #[test]
    fn host_submissions_count_without_a_send_and_stop_at_the_caps() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["logical_per_stage"] = json!(2);
        }));
        let kind = LogicalKind::Stage {
            stage: "one".to_string(),
        };
        for _ in 0..2 {
            world
                .wallet
                .record_submission(&world.policy, kind.clone(), None, &mut world.clock)
                .unwrap();
        }
        assert_eq!(
            world
                .wallet
                .record_submission(&world.policy, kind, None, &mut world.clock),
            Err(BudgetRefusal::LogicalStageExceeded)
        );
        assert_eq!(world.wallet.physical_used(), 0);
        assert_eq!(world.wallet.spent().unwrap(), Money::ZERO);
    }

    #[test]
    fn a_host_submission_does_not_escape_the_clock_or_the_latch() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.send(general()).unwrap();
        world.source.advance(1_001);
        assert_eq!(
            world.wallet.record_submission(
                &world.policy,
                LogicalKind::General,
                None,
                &mut world.clock
            ),
            Err(BudgetRefusal::WallExceeded)
        );

        let mut world = World::new(policy());
        world.in_state(AttemptState::Unknown);
        assert_eq!(
            world.wallet.record_submission(
                &world.policy,
                LogicalKind::General,
                None,
                &mut world.clock
            ),
            Err(BudgetRefusal::Latched)
        );
    }

    #[test]
    fn a_host_submission_is_free_of_money_even_with_the_cap_used_up() {
        let mut world = World::new(policy_with(|v| v["limits"]["cost_usd"] = json!("0.00072")));
        world.send(general()).unwrap();
        assert!(world.reserve(general()).is_err());
        assert!(
            world
                .wallet
                .record_submission(&world.policy, LogicalKind::General, None, &mut world.clock)
                .is_ok()
        );
    }

    #[test]
    fn many_tool_calls_of_one_response_are_one_send_settled_once() {
        let mut world = World::new(policy());
        let reservation = world.reserve(stage("one")).unwrap();
        let id = reservation.physical;
        world.wallet.start(id, BODY).unwrap();
        let settled = world
            .wallet
            .settle(&world.policy, id, &known(100, 40))
            .unwrap();
        let response: ResponseIdentity = settled.response;
        assert_eq!(response.physical(), id);
        for index in 0..3 {
            world
                .wallet
                .record_submission(
                    &world.policy,
                    LogicalKind::Stage {
                        stage: "one".to_string(),
                    },
                    Some(response.call(index)),
                    &mut world.clock,
                )
                .unwrap();
        }
        assert_eq!(world.wallet.physical_used(), 1);
        assert_eq!(world.wallet.attempts.len(), 1);
        assert_eq!(world.wallet.spent().unwrap(), money("0.0006"));
        // The send's own logical request and the three calls.
        assert_eq!(world.wallet.logical_global_used(), 4);
        assert_eq!(
            world.wallet.record_submission(
                &world.policy,
                LogicalKind::General,
                Some(response.call(1)),
                &mut world.clock
            ),
            Err(BudgetRefusal::DuplicateSubmission)
        );
    }

    #[test]
    fn a_submission_cannot_name_a_send_that_is_not_settled() {
        let mut world = World::new(policy());
        let id = world.in_state(AttemptState::Settled);
        let response = ResponseIdentity::of_settled(id);
        let other = world.reserve(general()).unwrap().physical;
        world.wallet.release(other).unwrap();
        let not_settled = ResponseIdentity::of_settled(other);
        assert_eq!(
            world.wallet.record_submission(
                &world.policy,
                LogicalKind::General,
                Some(not_settled.call(0)),
                &mut world.clock
            ),
            Err(BudgetRefusal::SendNotSettled)
        );
        let missing = ResponseIdentity::of_settled(PhysicalId(99));
        assert_eq!(
            world.wallet.record_submission(
                &world.policy,
                LogicalKind::General,
                Some(missing.call(0)),
                &mut world.clock
            ),
            Err(BudgetRefusal::NoSuchSend)
        );
        assert!(
            world
                .wallet
                .record_submission(
                    &world.policy,
                    LogicalKind::General,
                    Some(response.call(0)),
                    &mut world.clock
                )
                .is_ok()
        );
    }

    fn binding(attempt: u32) -> AttemptBinding {
        AttemptBinding {
            run_nonce: "nonce-synthetic".to_string(),
            stage: "one".to_string(),
            target: "target".to_string(),
            attempt,
            generation: 1,
        }
    }

    #[test]
    fn an_attempt_is_bound_after_the_fact_once() {
        let mut world = World::new(policy());
        let logical = world.send(stage("one")).unwrap().logical;
        assert_eq!(world.wallet.logical(logical).unwrap().attempt, None);
        world.wallet.bind_attempt(logical, binding(1)).unwrap();
        assert_eq!(
            world.wallet.logical(logical).unwrap().attempt,
            Some(binding(1))
        );
        assert_eq!(world.wallet.bind_attempt(logical, binding(1)), Ok(()));
        assert_eq!(
            world.wallet.bind_attempt(logical, binding(2)),
            Err(BudgetRefusal::AlreadyBound)
        );
        assert_eq!(
            world.wallet.logical(logical).unwrap().attempt,
            Some(binding(1))
        );
        let missing = super::LogicalId(99);
        assert_eq!(
            world.wallet.bind_attempt(missing, binding(1)),
            Err(BudgetRefusal::NoSuchLogical)
        );
    }

    #[test]
    fn recovery_releases_what_never_left_and_stops_on_what_may_have() {
        let mut world = World::new(policy());
        let id = world.in_state(AttemptState::Reserved);
        world.wallet.recover_after_process_end();
        assert_eq!(
            world.wallet.attempt(id).unwrap().state,
            AttemptState::Released
        );
        assert!(world.wallet.latch().is_none());

        let mut world = World::new(policy());
        let id = world.in_state(AttemptState::Started);
        world.wallet.recover_after_process_end();
        let attempt = world.wallet.attempt(id).unwrap();
        assert_eq!(attempt.state, AttemptState::Unknown);
        assert_eq!(
            attempt.unknown_reason,
            Some(UnknownReason::InterruptedAfterStart)
        );
        assert_eq!(
            world.wallet.latch().unwrap().reason,
            LatchReason::InterruptedAfterStart
        );
        assert_eq!(world.wallet.held().unwrap(), money("0.00072"));
    }

    #[test]
    fn recovery_leaves_settled_unknown_and_released_alone() {
        for state in [
            AttemptState::Settled,
            AttemptState::Unknown,
            AttemptState::Released,
        ] {
            let mut world = World::new(policy());
            world.in_state(state);
            let before = world.wallet.clone();
            world.wallet.recover_after_process_end();
            assert_eq!(world.wallet, before, "{state:?}");
        }
    }

    #[test]
    fn records_this_code_wrote_are_consistent_in_every_state() {
        for state in [
            AttemptState::Reserved,
            AttemptState::Started,
            AttemptState::Settled,
            AttemptState::Unknown,
            AttemptState::Released,
        ] {
            let mut world = World::new(policy());
            world.in_state(state);
            assert!(world.wallet.is_consistent(&world.policy), "{state:?}");
        }
        let mut world = World::new(policy());
        assert!(world.wallet.is_consistent(&world.policy));
        let id = world.in_state(AttemptState::Unknown);
        world
            .wallet
            .settle(&world.policy, id, &known(100, 40))
            .unwrap();
        assert!(world.wallet.is_consistent(&world.policy));
        let reservation = World::new(policy());
        assert!(reservation.wallet.is_consistent(&reservation.policy));
    }

    /// Each change is one a file could carry past its digest: every one has to be seen.
    #[test]
    fn records_that_disagree_are_not_consistent() {
        fn settled() -> World {
            let mut world = World::new(policy());
            world.in_state(AttemptState::Settled);
            world
        }
        type Change = fn(&mut WalletState);
        let changes: Vec<(&str, Change)> = vec![
            ("schema", |w| w.schema = "other".to_string()),
            ("id gap", |w| w.attempts[0].physical = PhysicalId(5)),
            ("logical gap", |w| w.logicals[0].id = super::LogicalId(5)),
            ("dangling logical", |w| {
                w.attempts[0].logical = super::LogicalId(9)
            }),
            ("reservation shrunk", |w| {
                w.attempts[0].reserved = Money::ZERO
            }),
            ("output bound over policy", |w| {
                w.attempts[0].output_bound = 51
            }),
            ("input bound over policy", |w| {
                w.attempts[0].input_bound = 1001
            }),
            ("settled without record", |w| {
                w.attempts[0].settlement = None
            }),
            ("consumed lowered", |w| {
                w.attempts[0].settlement.as_mut().unwrap().consumed = Money::ZERO;
            }),
            ("computed lowered", |w| {
                w.attempts[0].settlement.as_mut().unwrap().computed = Money::ZERO;
            }),
            ("billed differs from its text", |w| {
                w.attempts[0].settlement.as_mut().unwrap().billed_usd = "5".to_string();
            }),
            ("tokens lowered", |w| {
                w.attempts[0].settlement.as_mut().unwrap().prompt_tokens = 0;
            }),
            ("not settled with a record", |w| {
                w.attempts[0].state = AttemptState::Started
            }),
            ("released with a record", |w| {
                w.attempts[0].state = AttemptState::Released
            }),
            ("clock erased", |w| w.clock.started_wall_ms = None),
            ("over the reservation with no latch", |w| {
                let record = w.attempts[0].settlement.as_mut().unwrap();
                record.billed_usd = "5".to_string();
                record.billed = money("5");
                record.consumed = money("5");
            }),
            ("a reason on a send that was never unknown", |w| {
                w.attempts[0].unknown_reason = Some(UnknownReason::Timeout);
            }),
            ("reasoning larger than the completion", |w| {
                w.attempts[0].settlement.as_mut().unwrap().reasoning_tokens = Some(41);
            }),
            ("cached larger than the prompt", |w| {
                w.attempts[0].settlement.as_mut().unwrap().cached_tokens = Some(101);
            }),
            ("late with no reason", |w| {
                w.attempts[0].settlement.as_mut().unwrap().late = true;
            }),
            ("a final repair clock with no final repair", |w| {
                w.clock.final_repair_started_ms = Some(0);
            }),
            ("a source that names no send", |w| {
                w.logicals[0].source = Some(ResponseIdentity::of_settled(PhysicalId(9)).call(0));
            }),
        ];
        for (name, change) in changes {
            let mut world = settled();
            change(&mut world.wallet);
            assert!(!world.wallet.is_consistent(&world.policy), "{name}");
        }
    }

    #[test]
    fn an_unknown_without_its_latch_or_reason_is_not_consistent() {
        let build = || {
            let mut world = World::new(policy());
            world.in_state(AttemptState::Unknown);
            world
        };
        let mut world = build();
        world.wallet.latch = None;
        assert!(!world.wallet.is_consistent(&world.policy));
        let mut world = build();
        world.wallet.attempts[0].unknown_reason = None;
        assert!(!world.wallet.is_consistent(&world.policy));
    }

    #[test]
    fn a_late_settlement_without_its_latch_or_a_clock_with_no_send_is_not_consistent() {
        let mut world = World::new(policy());
        let id = world.in_state(AttemptState::Unknown);
        world
            .wallet
            .settle(&world.policy, id, &known(100, 40))
            .unwrap();
        world.wallet.latch = None;
        assert!(!world.wallet.is_consistent(&world.policy));

        let mut world = World::new(policy());
        world.wallet.clock.started_wall_ms = Some(1);
        assert!(!world.wallet.is_consistent(&world.policy));
    }

    #[test]
    fn two_unresolved_sends_or_a_final_repair_with_no_start_are_not_consistent() {
        let mut world = World::new(policy());
        world.in_state(AttemptState::Reserved);
        let second = world.wallet.attempts[0].clone();
        world.wallet.attempts.push(super::AttemptRecord {
            physical: PhysicalId(2),
            ..second
        });
        assert!(!world.wallet.is_consistent(&world.policy));

        let mut world = World::new(policy());
        world
            .reserve_purpose(final_repair(), SendPurpose::FinalRepair)
            .unwrap();
        world.wallet.clock.final_repair_started_ms = None;
        assert!(!world.wallet.is_consistent(&world.policy));
    }
}
