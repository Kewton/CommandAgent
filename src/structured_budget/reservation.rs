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
//! the reservation or the submission. A send takes no logical count, so what stops a send with no
//! submission is the physical, token, money, and wall limits, all of them required in a policy.

use std::collections::BTreeMap;

use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::value::RawValue;

use super::clock::{ClockRecord, RunClock};
use super::identity::{
    AttemptBinding, FinalRepairClaim, FinalRepairId, LogicalId, LogicalKind, PhysicalId,
    SendPurpose, SubmissionSource,
};
use super::policy::{ROUTE_API_CHAT_COMPLETIONS, RunBudgetPolicy, Tariff, WireRoute};
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
    /// The response was settled after the run's deadline. Its cost counts; the run did not make it.
    ResponseAfterDeadline,
    /// The same for a final repair's send, after the window of the final repairs.
    ResponseAfterFinalRepairWindow,
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
    /// The bytes are not one object of the listed keys with one positive count of output tokens.
    BodyNotAdmitted,
    PurposeDiffers,
    Overflow,
    NoSuchSend,
    NoSuchLogical,
    NoSuchFinalRepair,
    SendNotSettled,
    DuplicateSubmission,
    AlreadyBound,
    BindingStageDiffers,
    BytesChanged,
    /// A body went out and the report says no prompt tokens came in, or that it cost nothing.
    UsageInconsistent,
    IllegalTransition {
        from: AttemptState,
        to: AttemptState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptRecord {
    pub(crate) physical: PhysicalId,
    pub(crate) purpose: SendPurpose,
    pub(crate) final_repair: Option<FinalRepairId>,
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

/// One send about to be made. What it costs and where it goes are read off the bytes, under the
/// policy, and nothing else can say otherwise. `purpose` and `final_repair` are the exception: the
/// bytes do not carry them, so they stay inputs, and it is the caller's gate that has to hold them.
#[derive(Debug, Clone)]
pub(crate) struct ReserveRequest<'a> {
    /// The wire bytes. Only their length and digest are kept.
    pub(crate) body: &'a [u8],
    pub(crate) purpose: SendPurpose,
    pub(crate) final_repair: Option<FinalRepairClaim>,
}

/// Every top-level key a body may have. Any other could change what a send costs (`n` does), so
/// it is refused and not skipped. `messages` and `tools` are not read: their bytes are the bound.
const WIRE_KEYS: [&str; 7] = [
    "model",
    "messages",
    "max_tokens",
    "max_completion_tokens",
    "tools",
    "reasoning_effort",
    "provider",
];

/// What the bytes of a body say about themselves.
#[derive(Debug, PartialEq, Eq)]
struct WireBody {
    route: WireRoute,
    output_tokens: u64,
}

/// The members of a JSON object, none of them read yet. A key twice is not an object anything
/// agrees on: this parser and the one upstream could each take another of the two.
struct UniqueMembers<'a>(BTreeMap<String, &'a RawValue>);

impl<'de> Deserialize<'de> for UniqueMembers<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Members;
        impl<'de> Visitor<'de> for Members {
            type Value = UniqueMembers<'de>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object with no key twice")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut members = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, &'de RawValue>()? {
                    if members.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("a key twice"));
                    }
                }
                Ok(UniqueMembers(members))
            }
        }
        deserializer.deserialize_map(Members)
    }
}

/// The route the bytes ask for. `only` and `allow_fallbacks` are all a `provider` may hold, and both
/// have to be there: another key, `order` among them, can send it elsewhere, and leaving a key out
/// leaves it to the upstream's default.
///
/// `api` is not read off the bytes, which do not say where they go: it is the one value a policy
/// can hold, so it always matches. A body of another endpoint is stopped before this, by the list
/// of keys a body may have.
fn wire_route(members: &BTreeMap<String, &RawValue>) -> Result<WireRoute, BudgetRefusal> {
    fn read<T: DeserializeOwned>(
        members: &BTreeMap<String, &RawValue>,
        key: &str,
    ) -> Result<T, BudgetRefusal> {
        let raw = members.get(key).ok_or(BudgetRefusal::RouteNotAdmitted)?;
        serde_json::from_str(raw.get()).map_err(|_| BudgetRefusal::RouteNotAdmitted)
    }
    let provider = members
        .get("provider")
        .ok_or(BudgetRefusal::RouteNotAdmitted)?;
    let UniqueMembers(provider) =
        serde_json::from_str(provider.get()).map_err(|_| BudgetRefusal::RouteNotAdmitted)?;
    if provider.len() != 2 {
        return Err(BudgetRefusal::RouteNotAdmitted);
    }
    Ok(WireRoute {
        api: ROUTE_API_CHAT_COMPLETIONS.to_string(),
        model: read(members, "model")?,
        upstream_only: read(&provider, "only")?,
        allow_fallbacks: read(&provider, "allow_fallbacks")?,
        effort: read(members, "reasoning_effort")?,
    })
}

/// A body is one JSON object of the listed keys, with one positive count of output tokens. A count
/// that is not there is not read as zero, and zero is refused: a send always reserves something.
fn read_wire_body(bytes: &[u8]) -> Result<WireBody, BudgetRefusal> {
    let UniqueMembers(members) =
        serde_json::from_slice(bytes).map_err(|_| BudgetRefusal::BodyNotAdmitted)?;
    if members.keys().any(|key| !WIRE_KEYS.contains(&key.as_str())) {
        return Err(BudgetRefusal::BodyNotAdmitted);
    }
    let count = match (
        members.get("max_tokens"),
        members.get("max_completion_tokens"),
    ) {
        (Some(count), None) | (None, Some(count)) => count,
        _ => return Err(BudgetRefusal::BodyNotAdmitted),
    };
    let output_tokens = serde_json::from_str::<u64>(count.get())
        .ok()
        .filter(|count| *count > 0)
        .ok_or(BudgetRefusal::BodyNotAdmitted)?;
    Ok(WireBody {
        route: wire_route(&members)?,
        output_tokens,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reservation {
    pub(crate) physical: PhysicalId,
    pub(crate) final_repair: Option<FinalRepairId>,
    pub(crate) amount: Money,
}

/// What the limits are asked about.
enum Demand<'a> {
    Send {
        amount: Money,
        tokens: u64,
        final_repair: Option<FinalRepairClaim>,
    },
    /// Only the host looked at it: no send, no money, no tokens.
    Submission { kind: &'a LogicalKind },
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
    /// The first explicit profile, canonical. A profile that was only inferred is not pinned.
    pub(crate) profile: Option<String>,
    pub(crate) clock: ClockRecord,
    pub(crate) attempts: Vec<AttemptRecord>,
    pub(crate) logicals: Vec<LogicalRecord>,
    pub(crate) latch: Option<Latch>,
}

impl WalletState {
    pub(super) fn new(
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

    /// The final repairs opened. A release gives none back: the IDs run from one, in order, so the
    /// highest is the count.
    pub(crate) fn final_repair_used(&self) -> u64 {
        self.attempts
            .iter()
            .filter_map(|attempt| attempt.final_repair)
            .map(|id| id.0)
            .max()
            .unwrap_or(0)
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
        match demand {
            Demand::Submission { kind } => {
                if self.logical_global_used() >= limits.logical_global {
                    return Err(BudgetRefusal::LogicalGlobalExceeded);
                }
                if let LogicalKind::Stage { stage } = kind
                    && self.logical_stage_used(stage) >= limits.logical_per_stage
                {
                    return Err(BudgetRefusal::LogicalStageExceeded);
                }
                Ok(())
            }
            Demand::Send {
                amount,
                tokens,
                final_repair,
            } => {
                if let Some(claim) = final_repair {
                    if *claim == FinalRepairClaim::New
                        && self.final_repair_used() >= limits.final_repair_attempts
                    {
                        return Err(BudgetRefusal::FinalRepairAttemptsExceeded);
                    }
                    let began = self.clock.final_repair_started_ms.unwrap_or(elapsed_ms);
                    if elapsed_ms.saturating_sub(began) > limits.final_repair_ms {
                        return Err(BudgetRefusal::FinalRepairWallExceeded);
                    }
                }
                if self.physical_used() >= limits.physical_sends {
                    return Err(BudgetRefusal::PhysicalExceeded);
                }
                if !within_cap(
                    self.spent_tokens(),
                    self.held_tokens(),
                    u128::from(*tokens),
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
        }
    }

    /// A claim of a final repair has to go with that purpose, both ways, and to continue one that
    /// exists.
    fn refuse_final_repair_claim(&self, request: &ReserveRequest<'_>) -> Result<(), BudgetRefusal> {
        if request.final_repair.is_some() != (request.purpose == SendPurpose::FinalRepair) {
            return Err(BudgetRefusal::PurposeDiffers);
        }
        if let Some(FinalRepairClaim::Continue(id)) = request.final_repair
            && (id.0 == 0 || id.0 > self.final_repair_used())
        {
            return Err(BudgetRefusal::NoSuchFinalRepair);
        }
        Ok(())
    }

    /// A reservation of the request's bytes, on the record before anything is sent.
    pub(super) fn reserve(
        &mut self,
        policy: &RunBudgetPolicy,
        request: &ReserveRequest<'_>,
        clock: &mut RunClock,
    ) -> Result<Reservation, BudgetRefusal> {
        self.success_gate()?;
        let wire = read_wire_body(request.body)?;
        if !policy.route.admits(&wire.route) {
            return Err(BudgetRefusal::RouteNotAdmitted);
        }
        let input_bound = u64::try_from(request.body.len())
            .ok()
            .and_then(|bytes| bytes.checked_add(policy.input_overhead))
            .ok_or(BudgetRefusal::Overflow)?;
        if input_bound > policy.max_input_tokens {
            return Err(BudgetRefusal::InputBoundExceeded);
        }
        if wire.output_tokens > policy.max_output_tokens {
            return Err(BudgetRefusal::OutputBoundExceeded);
        }
        let tokens = input_bound
            .checked_add(wire.output_tokens)
            .ok_or(BudgetRefusal::Overflow)?;
        let amount = reservation_amount(&policy.tariff, input_bound, wire.output_tokens)?;
        self.refuse_final_repair_claim(request)?;
        let elapsed_ms = clock.observe(&mut self.clock);
        self.check_limits(
            policy,
            &Demand::Send {
                amount,
                tokens,
                final_repair: request.final_repair,
            },
            elapsed_ms,
        )?;
        clock.begin(&mut self.clock);
        let final_repair = match request.final_repair {
            None => None,
            Some(FinalRepairClaim::Continue(id)) => Some(id),
            Some(FinalRepairClaim::New) => {
                self.clock.final_repair_started_ms.get_or_insert(elapsed_ms);
                Some(FinalRepairId(self.final_repair_used() + 1))
            }
        };
        let physical = PhysicalId(self.attempts.len() as u64 + 1);
        self.attempts.push(AttemptRecord {
            physical,
            purpose: request.purpose,
            final_repair,
            body_sha256: sha256_hex(request.body),
            input_bound,
            output_bound: wire.output_tokens,
            reserved: amount,
            state: AttemptState::Reserved,
            unknown_reason: None,
            settlement: None,
        });
        Ok(Reservation {
            physical,
            final_repair,
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

    /// Whether the time the ledger last observed is past the run's deadline.
    pub(super) fn past_the_deadline(&self, policy: &RunBudgetPolicy) -> bool {
        self.clock.high_water_elapsed_ms > policy.limits.wall_ms
    }

    /// Whether the time the ledger last observed is past the window of the final repairs, which
    /// runs from the first of them. It takes no send: a caller asks it only for a final repair's.
    pub(super) fn past_the_final_repair_window(&self, policy: &RunBudgetPolicy) -> bool {
        self.clock.final_repair_started_ms.is_some_and(|began| {
            self.clock.high_water_elapsed_ms.saturating_sub(began) > policy.limits.final_repair_ms
        })
    }

    /// `reserved` to `started`, for the same bytes that were reserved and inside the deadline as
    /// the ledger last observed it, and a final repair's send inside its window too. It gives out
    /// nothing: only `BudgetLedger::start`, with the start on disk, gives out what a POST needs.
    pub(super) fn start(
        &mut self,
        policy: &RunBudgetPolicy,
        physical: PhysicalId,
        body: &[u8],
    ) -> Result<(), BudgetRefusal> {
        let past_deadline = self.past_the_deadline(policy);
        let past_window = self.past_the_final_repair_window(policy);
        let attempt = self.attempt_in(physical, AttemptState::Reserved, AttemptState::Started)?;
        if attempt.body_sha256 != sha256_hex(body) {
            return Err(BudgetRefusal::BytesChanged);
        }
        if past_deadline {
            return Err(BudgetRefusal::WallExceeded);
        }
        if past_window && attempt.final_repair.is_some() {
            return Err(BudgetRefusal::FinalRepairWallExceeded);
        }
        attempt.state = AttemptState::Started;
        Ok(())
    }

    /// `reserved` to `released`: the one way a reservation goes back, and nothing has left.
    pub(super) fn release(&mut self, physical: PhysicalId) -> Result<(), BudgetRefusal> {
        let attempt = self.attempt_in(physical, AttemptState::Reserved, AttemptState::Released)?;
        attempt.state = AttemptState::Released;
        Ok(())
    }

    /// `started` to `unknown`. The reservation stays held and the wallet stops.
    pub(super) fn mark_unknown(
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
    /// the clock, which it starts if nothing has yet. Its `source`, when it came from a response,
    /// must be a settled one.
    pub(super) fn record_submission(
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
        self.check_limits(policy, &Demand::Submission { kind: &kind }, elapsed_ms)?;
        clock.begin(&mut self.clock);
        let id = LogicalId(self.logicals.len() as u64 + 1);
        self.logicals.push(LogicalRecord {
            id,
            kind,
            source,
            attempt: None,
        });
        Ok(id)
    }

    /// Ties G4's attempt to a submission of its stage after the fact, once. The same value again
    /// is no change.
    pub(super) fn bind_attempt(
        &mut self,
        logical: LogicalId,
        binding: AttemptBinding,
    ) -> Result<(), BudgetRefusal> {
        let logical = self
            .logicals
            .iter_mut()
            .find(|record| record.id == logical)
            .ok_or(BudgetRefusal::NoSuchLogical)?;
        if !matches!(&logical.kind, LogicalKind::Stage { stage } if *stage == binding.stage) {
            return Err(BudgetRefusal::BindingStageDiffers);
        }
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
    pub(super) fn recover_after_process_end(&mut self) {
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

    /// Whether these records can be what this code wrote under this policy. Finds damage, not an
    /// adversary: the digest has no key. Amounts are computed again, and the states have to agree
    /// with what they carry.
    pub(crate) fn is_consistent(&self, policy: &RunBudgetPolicy) -> bool {
        self.schema == WALLET_SCHEMA
            && self.logicals_are_consistent()
            && self.clock_is_consistent()
            && self.final_repairs_are_consistent()
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
            && self
                .latch
                .is_none_or(|latch| self.attempt(latch.physical).is_some())
            && (self.latch.is_some() || self.is_within_limits(policy))
    }

    /// The limits a record can show. Time is not one: it passes with nothing on the record.
    fn is_within_limits(&self, policy: &RunBudgetPolicy) -> bool {
        let limits = &policy.limits;
        let within_money = match (self.spent(), self.held()) {
            (Ok(spent), Ok(held)) => {
                within_cap(spent.units(), held.units(), 0, limits.cost.units()).unwrap_or(false)
            }
            _ => false,
        };
        within_money
            && self.physical_used() <= limits.physical_sends
            && self.logical_global_used() <= limits.logical_global
            && self.final_repair_used() <= limits.final_repair_attempts
            && self.spent_tokens() + self.held_tokens() <= u128::from(limits.tokens)
            && self.logicals.iter().all(|logical| match &logical.kind {
                LogicalKind::General => true,
                LogicalKind::Stage { stage } => {
                    self.logical_stage_used(stage) <= limits.logical_per_stage
                }
            })
    }

    fn logicals_are_consistent(&self) -> bool {
        self.logicals.iter().enumerate().all(|(index, logical)| {
            logical.id.0 == index as u64 + 1
                && logical.source.is_none_or(|source| {
                    self.attempt(source.physical)
                        .is_some_and(|attempt| attempt.state == AttemptState::Settled)
                        && self.logicals[..index]
                            .iter()
                            .all(|earlier| earlier.source != Some(source))
                })
        })
    }

    fn clock_is_consistent(&self) -> bool {
        let anything_recorded = !self.attempts.is_empty() || !self.logicals.is_empty();
        self.clock.started_wall_ms.is_some() == anything_recorded
            && self.clock.final_repair_started_ms.is_some() == (self.final_repair_used() > 0)
    }

    /// The IDs first appear in order from one, and each goes with its purpose.
    fn final_repairs_are_consistent(&self) -> bool {
        let mut highest = 0;
        self.attempts.iter().all(|attempt| {
            let matched =
                attempt.final_repair.is_some() == (attempt.purpose == SendPurpose::FinalRepair);
            let in_order = attempt.final_repair.is_none_or(|id| {
                let fits = id.0 >= 1 && id.0 <= highest + 1;
                highest = highest.max(id.0);
                fits
            });
            matched && in_order
        })
    }

    /// A record is what `reserve` wrote for it: its bounds inside the policy, bytes past the
    /// overhead and some output, since `reserve` takes no body with no bytes or no output, and the
    /// amount the formula gives. Then its state agrees with what it carries.
    fn attempt_is_consistent(
        &self,
        policy: &RunBudgetPolicy,
        index: usize,
        attempt: &AttemptRecord,
    ) -> bool {
        let identified = attempt.physical.0 == index as u64 + 1
            && attempt.input_bound > policy.input_overhead
            && attempt.output_bound > 0
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
                        && !Self::report_hides_the_body(
                            policy,
                            attempt.input_bound,
                            record.prompt_tokens,
                            record.consumed,
                        )
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
        AttemptBinding, FinalRepairClaim, FinalRepairId, LogicalId, LogicalKind, PhysicalId,
        ResponseIdentity, SendPurpose,
    };
    use super::super::policy::RunBudgetPolicy;
    use super::super::settlement::{KnownBilling, ReportedUsage, UnknownReason};
    use super::super::test_support::{
        BODY, FakeClock, Rng, body_asking_for, policy, policy_value, policy_with, request, route,
        stage_kind,
    };
    use super::{
        AmountError, AttemptRecord, AttemptState, BudgetRefusal, LatchReason, Money, Reservation,
        ReserveRequest, WalletState, read_wire_body, reservation_amount, within_cap,
    };
    use crate::providers::BilledCostUsd;

    const UNIT: u128 = 1_000_000_000_000_000_000;

    fn money(text: &str) -> Money {
        Money::parse_decimal_up(text).unwrap()
    }

    fn billing(token: &str, prompt: u64, completion: u64) -> KnownBilling {
        let cost = BilledCostUsd::from_json_token(token).unwrap();
        KnownBilling::from_reported(&ReportedUsage {
            cost: Some(&cost),
            prompt_tokens: Some(prompt),
            completion_tokens: Some(completion),
            ..ReportedUsage::default()
        })
        .unwrap()
    }

    fn known(prompt: u64, completion: u64) -> KnownBilling {
        billing("0", prompt, completion)
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

        fn reserve(&mut self) -> Result<Reservation, BudgetRefusal> {
            self.wallet
                .reserve(&self.policy, &request(), &mut self.clock)
        }

        /// A reservation of exactly these bytes.
        fn reserve_bytes(&mut self, bytes: &[u8]) -> Result<Reservation, BudgetRefusal> {
            let request = ReserveRequest {
                body: bytes,
                ..request()
            };
            self.wallet.reserve(&self.policy, &request, &mut self.clock)
        }

        fn reserve_as(
            &mut self,
            purpose: SendPurpose,
            final_repair: Option<FinalRepairClaim>,
        ) -> Result<Reservation, BudgetRefusal> {
            let mut body = request();
            body.purpose = purpose;
            body.final_repair = final_repair;
            self.wallet.reserve(&self.policy, &body, &mut self.clock)
        }

        fn reserve_repair(
            &mut self,
            claim: FinalRepairClaim,
        ) -> Result<Reservation, BudgetRefusal> {
            self.reserve_as(SendPurpose::FinalRepair, Some(claim))
        }

        fn start(&mut self, id: PhysicalId) -> Result<(), BudgetRefusal> {
            self.wallet.start(&self.policy, id, BODY)
        }

        fn complete(&mut self, id: PhysicalId) -> ResponseIdentity {
            self.start(id).unwrap();
            self.wallet
                .settle(&self.policy, id, &known(100, 40))
                .unwrap()
                .response
        }

        /// A whole send, settled well inside its reservation.
        fn send(&mut self) -> Result<ResponseIdentity, BudgetRefusal> {
            let id = self.reserve()?.physical;
            Ok(self.complete(id))
        }

        fn send_repair(&mut self, claim: FinalRepairClaim) -> Result<Reservation, BudgetRefusal> {
            let reservation = self.reserve_repair(claim)?;
            self.complete(reservation.physical);
            Ok(reservation)
        }

        fn submit(&mut self, kind: LogicalKind) -> Result<LogicalId, BudgetRefusal> {
            self.wallet
                .record_submission(&self.policy, kind, None, &mut self.clock)
        }

        fn in_state(&mut self, state: AttemptState) -> PhysicalId {
            let id = self.reserve().unwrap().physical;
            match state {
                AttemptState::Reserved => {}
                AttemptState::Started => self.start(id).unwrap(),
                AttemptState::Released => self.wallet.release(id).unwrap(),
                AttemptState::Unknown => {
                    self.start(id).unwrap();
                    self.wallet
                        .mark_unknown(id, UnknownReason::Timeout)
                        .unwrap();
                }
                AttemptState::Settled => {
                    self.complete(id);
                }
            }
            id
        }
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
    fn the_largest_power_of_ten_that_fits_is_accepted() {
        assert_eq!(money("1e20").units(), 10_u128.pow(38));
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
            "\"+5\"",
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
        assert!(exact.reserve().is_ok());
        let mut short = World::new(policy_with(|v| {
            v["limits"]["cost_usd"] = json!("0.000719999999999999");
        }));
        assert_eq!(short.reserve(), Err(BudgetRefusal::CostExceeded));
        assert!(short.wallet.attempts.is_empty());
    }

    #[test]
    fn what_was_spent_counts_against_the_next_reservation() {
        // The first send consumes 0.0006; the second needs 0.00072.
        let mut exact = World::new(policy_with(|v| v["limits"]["cost_usd"] = json!("0.00132")));
        exact.send().unwrap();
        assert!(exact.reserve().is_ok());
        let mut short = World::new(policy_with(|v| {
            v["limits"]["cost_usd"] = json!("0.001319999999999999");
        }));
        short.send().unwrap();
        assert_eq!(short.reserve(), Err(BudgetRefusal::CostExceeded));
    }

    #[test]
    fn a_token_cap_passes_exactly_and_refuses_one_short() {
        // 100 + 10 + 50.
        let mut exact = World::new(policy_with(|v| v["limits"]["tokens"] = json!(160)));
        assert!(exact.reserve().is_ok());
        let mut short = World::new(policy_with(|v| v["limits"]["tokens"] = json!(159)));
        assert_eq!(short.reserve(), Err(BudgetRefusal::TokensExceeded));
    }

    #[test]
    fn tokens_spent_count_against_the_next_reservation() {
        // The first send used 140 tokens.
        let mut exact = World::new(policy_with(|v| v["limits"]["tokens"] = json!(300)));
        exact.send().unwrap();
        assert!(exact.reserve().is_ok());
        let mut short = World::new(policy_with(|v| v["limits"]["tokens"] = json!(299)));
        short.send().unwrap();
        assert_eq!(short.reserve(), Err(BudgetRefusal::TokensExceeded));
    }

    #[test]
    fn a_physical_cap_passes_exactly_and_refuses_one_past() {
        let mut world = World::new(policy_with(|v| v["limits"]["physical_sends"] = json!(2)));
        world.send().unwrap();
        world.send().unwrap();
        assert_eq!(world.reserve(), Err(BudgetRefusal::PhysicalExceeded));
        assert_eq!(world.wallet.physical_used(), 2);
    }

    #[test]
    fn a_send_opens_no_logical_request_and_goes_on_with_none_left() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["logical_global"] = json!(0);
            v["limits"]["logical_per_stage"] = json!(0);
        }));
        world.send().unwrap();
        world.send().unwrap();
        assert_eq!(world.wallet.logical_global_used(), 0);
        assert_eq!(world.wallet.physical_used(), 2);
    }

    /// With no submission at all, the physical cap is what ends it, and a policy cannot leave it
    /// out (`every_missing_field_is_refused`).
    #[test]
    fn sends_with_no_submission_stop_at_the_physical_cap() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["logical_global"] = json!(0);
            v["limits"]["physical_sends"] = json!(3);
        }));
        for _ in 0..3 {
            world.send().unwrap();
        }
        assert_eq!(world.reserve(), Err(BudgetRefusal::PhysicalExceeded));
    }

    /// The provider reports zero here, and the tariff still uses the cap up, because this report
    /// has uncached prompt tokens and the input rate cannot be zero. The cached rate can: a report
    /// that comes to nothing that way is `a_body_that_comes_to_nothing_is_not_known...`.
    #[test]
    fn sends_with_no_submission_stop_at_the_cost_cap_when_the_provider_bills_nothing() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["logical_global"] = json!(0);
            v["limits"]["cost_usd"] = json!("0.0014");
        }));
        world.send().unwrap();
        world.send().unwrap();
        assert_eq!(world.reserve(), Err(BudgetRefusal::CostExceeded));
        assert_eq!(world.wallet.spent().unwrap(), money("0.0012"));
    }

    #[test]
    fn three_calls_of_one_response_are_three_submissions_of_one_send() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_per_stage"] = json!(3)));
        let response = world.send().unwrap();
        for index in 0..3 {
            world
                .wallet
                .record_submission(
                    &world.policy,
                    stage_kind("one"),
                    Some(response.call(index)),
                    &mut world.clock,
                )
                .unwrap();
        }
        assert_eq!(world.wallet.logical_stage_used("one"), 3);
        assert_eq!(world.wallet.logical_global_used(), 3);
        assert_eq!(world.wallet.physical_used(), 1);
        assert_eq!(world.wallet.attempts.len(), 1);
        assert_eq!(world.wallet.spent().unwrap(), money("0.0006"));
        // A fourth is past the cap, and a repeat of a call is not a new one.
        assert_eq!(
            world.wallet.record_submission(
                &world.policy,
                stage_kind("one"),
                Some(response.call(3)),
                &mut world.clock
            ),
            Err(BudgetRefusal::LogicalStageExceeded)
        );
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
    fn a_global_logical_cap_passes_exactly_and_refuses_one_past() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_global"] = json!(2)));
        world.submit(stage_kind("one")).unwrap();
        world.submit(LogicalKind::General).unwrap();
        assert_eq!(
            world.submit(stage_kind("two")),
            Err(BudgetRefusal::LogicalGlobalExceeded)
        );
        assert_eq!(world.wallet.logical_global_used(), 2);
    }

    #[test]
    fn a_stage_cap_passes_exactly_refuses_one_past_and_leaves_other_stages_alone() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_per_stage"] = json!(2)));
        world.submit(stage_kind("one")).unwrap();
        world.submit(stage_kind("one")).unwrap();
        assert_eq!(
            world.submit(stage_kind("one")),
            Err(BudgetRefusal::LogicalStageExceeded)
        );
        assert_eq!(world.wallet.logical_stage_used("one"), 2);
        assert_eq!(world.wallet.logical_stage_used("two"), 0);
        world.submit(stage_kind("two")).unwrap();
        // A submission with no stage is not in a stage's count.
        world.submit(LogicalKind::General).unwrap();
        assert_eq!(world.wallet.logical_stage_used("two"), 1);
    }

    #[test]
    fn the_wall_limit_passes_exactly_and_refuses_one_past() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.send().unwrap();
        world.source.advance(1_000);
        world.send().unwrap();
        world.source.advance(1);
        assert_eq!(world.reserve(), Err(BudgetRefusal::WallExceeded));
    }

    #[test]
    fn the_run_clock_starts_at_the_first_reservation_not_before() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.source.advance(50_000);
        world.send().unwrap();
        assert_eq!(world.wallet.clock.started_wall_ms, Some(55_000));
        world.source.advance(1_000);
        world.send().unwrap();
        assert_eq!(world.wallet.clock.started_wall_ms, Some(55_000));
    }

    #[test]
    fn a_refused_first_reservation_does_not_start_the_clock() {
        let mut world = World::new(policy_with(|v| v["limits"]["tokens"] = json!(1)));
        assert!(world.reserve().is_err());
        assert_eq!(world.wallet.clock.started_wall_ms, None);
    }

    #[test]
    fn a_submission_with_no_send_starts_the_clock_and_the_limit_holds_it() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.source.advance(50_000);
        world.submit(LogicalKind::General).unwrap();
        assert_eq!(world.wallet.clock.started_wall_ms, Some(55_000));
        assert!(world.wallet.is_consistent(&world.policy));
        world.source.advance(1_000);
        world.submit(LogicalKind::General).unwrap();
        world.source.advance(1);
        assert_eq!(
            world.submit(LogicalKind::General),
            Err(BudgetRefusal::WallExceeded)
        );
        // A send after those submissions is held to the same clock.
        assert_eq!(world.reserve(), Err(BudgetRefusal::WallExceeded));
    }

    #[test]
    fn a_refused_first_submission_does_not_start_the_clock() {
        let mut world = World::new(policy_with(|v| v["limits"]["logical_global"] = json!(0)));
        assert!(world.submit(LogicalKind::General).is_err());
        assert_eq!(world.wallet.clock.started_wall_ms, None);
    }

    #[test]
    fn final_repair_attempts_pass_exactly_and_refuse_one_past() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(2)
        }));
        let first = world.send_repair(FinalRepairClaim::New).unwrap();
        let second = world.send_repair(FinalRepairClaim::New).unwrap();
        assert_eq!(first.final_repair, Some(FinalRepairId(1)));
        assert_eq!(second.final_repair, Some(FinalRepairId(2)));
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::New),
            Err(BudgetRefusal::FinalRepairAttemptsExceeded)
        );
        assert_eq!(world.wallet.final_repair_used(), 2);
    }

    #[test]
    fn a_final_repair_that_returns_only_text_takes_its_attempt() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(1)
        }));
        world.send_repair(FinalRepairClaim::New).unwrap();
        assert_eq!(world.wallet.logical_global_used(), 0);
        assert_eq!(world.wallet.final_repair_used(), 1);
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::New),
            Err(BudgetRefusal::FinalRepairAttemptsExceeded)
        );
    }

    #[test]
    fn the_tool_calls_and_later_sends_of_one_final_repair_take_no_more_attempts() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(1)
        }));
        let repair = world.reserve_repair(FinalRepairClaim::New).unwrap();
        let response = world.complete(repair.physical);
        for index in 0..2 {
            world
                .wallet
                .record_submission(
                    &world.policy,
                    LogicalKind::General,
                    Some(response.call(index)),
                    &mut world.clock,
                )
                .unwrap();
        }
        let id = repair.final_repair.unwrap();
        world.send_repair(FinalRepairClaim::Continue(id)).unwrap();
        assert_eq!(world.wallet.final_repair_used(), 1);
        assert_eq!(world.wallet.physical_used(), 2);
    }

    #[test]
    fn continuing_a_final_repair_that_does_not_exist_is_refused() {
        let mut world = World::new(policy());
        for missing in [0, 1] {
            assert_eq!(
                world.reserve_repair(FinalRepairClaim::Continue(FinalRepairId(missing))),
                Err(BudgetRefusal::NoSuchFinalRepair),
                "{missing}"
            );
        }
        let repair = world.send_repair(FinalRepairClaim::New).unwrap();
        assert_eq!(repair.final_repair, Some(FinalRepairId(1)));
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::Continue(FinalRepairId(2))),
            Err(BudgetRefusal::NoSuchFinalRepair)
        );
    }

    #[test]
    fn the_final_repair_window_passes_exactly_and_refuses_one_past_from_its_own_start() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_seconds"] = json!(1)
        }));
        // Time before the first repair is not in its window.
        world.send().unwrap();
        world.source.advance(50_000);
        let repair = world.send_repair(FinalRepairClaim::New).unwrap();
        let id = repair.final_repair.unwrap();
        world.source.advance(1_000);
        world.send_repair(FinalRepairClaim::Continue(id)).unwrap();
        world.source.advance(1);
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::Continue(id)),
            Err(BudgetRefusal::FinalRepairWallExceeded)
        );
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::New),
            Err(BudgetRefusal::FinalRepairWallExceeded)
        );
        // Nothing else is in its window.
        assert!(world.reserve().is_ok());
    }

    #[test]
    fn a_global_limit_that_runs_out_first_stops_a_final_repair() {
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(5);
            v["limits"]["physical_sends"] = json!(1);
        }));
        world.send().unwrap();
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::New),
            Err(BudgetRefusal::PhysicalExceeded)
        );
    }

    #[test]
    fn a_final_repair_has_to_say_so_both_ways() {
        let mut world = World::new(policy());
        assert_eq!(
            world.reserve_as(SendPurpose::Executor, Some(FinalRepairClaim::New)),
            Err(BudgetRefusal::PurposeDiffers)
        );
        assert_eq!(
            world.reserve_as(SendPurpose::FinalRepair, None),
            Err(BudgetRefusal::PurposeDiffers)
        );
        assert!(world.wallet.attempts.is_empty());
        assert!(world.reserve_as(SendPurpose::Executor, None).is_ok());
    }

    /// One row per limit in a policy, and the table has to cover them all. The money, token, and
    /// physical limits govern every send. The logical limits govern submissions, which a send is
    /// not. The two limits in seconds cannot be zero at all (`policy.rs`).
    #[test]
    fn every_limit_at_zero_allows_nothing_it_governs() {
        let refused_sends = [
            ("cost_usd", json!("0"), BudgetRefusal::CostExceeded),
            ("tokens", json!(0), BudgetRefusal::TokensExceeded),
            ("physical_sends", json!(0), BudgetRefusal::PhysicalExceeded),
        ];
        for (key, zero, refusal) in refused_sends {
            let mut world = World::new(policy_with(|v| v["limits"][key] = zero.clone()));
            assert_eq!(world.reserve(), Err(refusal), "{key}");
            assert_eq!(
                world.reserve_repair(FinalRepairClaim::New),
                Err(refusal),
                "{key}"
            );
            assert!(world.wallet.attempts.is_empty(), "{key}");
        }

        let mut world = World::new(policy_with(|v| v["limits"]["logical_global"] = json!(0)));
        assert_eq!(
            world.submit(LogicalKind::General),
            Err(BudgetRefusal::LogicalGlobalExceeded)
        );
        let mut world = World::new(policy_with(|v| v["limits"]["logical_per_stage"] = json!(0)));
        assert_eq!(
            world.submit(stage_kind("one")),
            Err(BudgetRefusal::LogicalStageExceeded)
        );
        let mut world = World::new(policy_with(|v| {
            v["limits"]["final_repair_attempts"] = json!(0);
        }));
        assert_eq!(
            world.reserve_repair(FinalRepairClaim::New),
            Err(BudgetRefusal::FinalRepairAttemptsExceeded)
        );
        assert!(world.wallet.attempts.is_empty());

        let covered = [
            "cost_usd",
            "tokens",
            "physical_sends",
            "logical_global",
            "logical_per_stage",
            "final_repair_attempts",
            "wall_seconds",
            "final_repair_seconds",
        ];
        let value = policy_value();
        let limits = value["limits"].as_object().unwrap();
        assert_eq!(limits.len(), covered.len());
        for key in covered {
            assert!(limits.contains_key(key), "{key}");
        }
    }

    #[test]
    fn a_route_the_policy_does_not_declare_is_refused_before_any_record() {
        let mut world = World::new(policy());
        let body = String::from_utf8(BODY.to_vec())
            .unwrap()
            .replace("m-1", "another-976");
        assert_eq!(
            world.reserve_bytes(body.as_bytes()),
            Err(BudgetRefusal::RouteNotAdmitted)
        );
        assert!(world.wallet.attempts.is_empty());
    }

    /// The fixture's length is what the amounts in this file are worked from.
    #[test]
    fn the_fixture_reserves_its_input_bound_its_tokens_and_its_amount() {
        assert_eq!(BODY.len(), 106);
        let mut world = World::new(policy());
        assert_eq!(world.policy.input_overhead, 4);
        let reservation = world.reserve().unwrap();
        assert_eq!(reservation.amount, money("0.00072"));
        let attempt = world.wallet.attempt(reservation.physical).unwrap();
        assert_eq!(attempt.input_bound, 110);
        assert_eq!(attempt.output_bound, 50);
        assert_eq!(world.wallet.held_tokens(), 160);
    }

    /// The invariant itself. Two wallets that were told nothing else about the same bytes hold the
    /// same record, and the control is that other bytes hold another.
    #[test]
    fn the_same_bytes_and_policy_are_the_same_reservation() {
        let record = |bytes: &[u8]| {
            let mut world = World::new(policy());
            let id = world.reserve_bytes(bytes).unwrap().physical;
            world.wallet.attempt(id).unwrap().clone()
        };
        assert_eq!(record(BODY), record(BODY));
        let asked_for_less = body_asking_for(10);
        let other = record(&asked_for_less);
        assert_eq!(other.output_bound, 10);
        assert_ne!(other, record(BODY));
    }

    #[test]
    fn a_body_is_read_for_what_it_says_and_nothing_it_does_not() {
        use BudgetRefusal::{BodyNotAdmitted as Body, RouteNotAdmitted as Route};
        const REST: &str =
            r#""reasoning_effort":"low","provider":{"only":["u"],"allow_fallbacks":false}"#;
        const HEAD: &str = r#""model":"m-1","max_tokens":50"#;
        let rows: Vec<(&str, String, Result<u64, BudgetRefusal>)> = vec![
            (
                "the fixture",
                String::from_utf8(BODY.to_vec()).unwrap(),
                Ok(50),
            ),
            (
                "max_completion_tokens alone",
                format!(r#"{{"model":"m-1","max_completion_tokens":7,{REST}}}"#),
                Ok(7),
            ),
            (
                "messages and tools",
                format!(r#"{{{HEAD},"messages":[{{"a":1}}],"tools":[],{REST}}}"#),
                Ok(50),
            ),
            ("nothing at all", String::new(), Err(Body)),
            ("not JSON", "not json 976".to_string(), Err(Body)),
            ("an array", "[1]".to_string(), Err(Body)),
            ("two values", format!("{{{HEAD},{REST}}} {{}}"), Err(Body)),
            (
                "a key not listed",
                format!(r#"{{{HEAD},"n":9,{REST}}}"#),
                Err(Body),
            ),
            (
                "stream",
                format!(r#"{{{HEAD},"stream":true,{REST}}}"#),
                Err(Body),
            ),
            (
                "no count",
                format!(r#"{{"model":"m-1",{REST}}}"#),
                Err(Body),
            ),
            (
                "both counts",
                format!(r#"{{{HEAD},"max_completion_tokens":50,{REST}}}"#),
                Err(Body),
            ),
            (
                "a count of zero",
                format!(r#"{{"model":"m-1","max_tokens":0,{REST}}}"#),
                Err(Body),
            ),
            (
                "a count of zero in the other field",
                format!(r#"{{"model":"m-1","max_completion_tokens":0,{REST}}}"#),
                Err(Body),
            ),
            (
                "a count in text",
                format!(r#"{{"model":"m-1","max_tokens":"50",{REST}}}"#),
                Err(Body),
            ),
            (
                "a count with a fraction",
                format!(r#"{{"model":"m-1","max_tokens":50.0,{REST}}}"#),
                Err(Body),
            ),
            (
                "a negative count",
                format!(r#"{{"model":"m-1","max_tokens":-50,{REST}}}"#),
                Err(Body),
            ),
            (
                "a key twice",
                format!(r#"{{{HEAD},"max_tokens":1,{REST}}}"#),
                Err(Body),
            ),
            (
                "a key twice under another spelling",
                format!(r#"{{{HEAD},"max_tokens":1,{REST}}}"#),
                Err(Body),
            ),
            (
                "a key once under another spelling",
                format!(r#"{{"model":"m-1","max_tokens":50,{REST}}}"#),
                Ok(50),
            ),
            (
                "no model",
                format!(r#"{{"max_tokens":50,{REST}}}"#),
                Err(Route),
            ),
            (
                "no effort",
                format!(r#"{{{HEAD},"provider":{{"only":["u"],"allow_fallbacks":false}}}}"#),
                Err(Route),
            ),
            (
                "another effort",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"high","provider":{{"only":["u"],"allow_fallbacks":false}}}}"#
                ),
                Err(Route),
            ),
            (
                "no provider",
                format!(r#"{{{HEAD},"reasoning_effort":"low"}}"#),
                Err(Route),
            ),
            (
                "a provider that is no object",
                format!(r#"{{{HEAD},"reasoning_effort":"low","provider":"u"}}"#),
                Err(Route),
            ),
            (
                "order beside the two",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["u"],"order":["v"],"allow_fallbacks":false}}}}"#
                ),
                Err(Route),
            ),
            (
                "order in place of only",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"order":["u"],"allow_fallbacks":false}}}}"#
                ),
                Err(Route),
            ),
            (
                "another key beside the two",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["u"],"ignore":[],"allow_fallbacks":false}}}}"#
                ),
                Err(Route),
            ),
            (
                "no only",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"allow_fallbacks":false}}}}"#
                ),
                Err(Route),
            ),
            (
                "another only",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["v"],"allow_fallbacks":false}}}}"#
                ),
                Err(Route),
            ),
            (
                "fallbacks allowed",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["u"],"allow_fallbacks":true}}}}"#
                ),
                Err(Route),
            ),
            (
                "no word on fallbacks",
                format!(r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["u"]}}}}"#),
                Err(Route),
            ),
            (
                "fallbacks not a boolean",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["u"],"allow_fallbacks":"false"}}}}"#
                ),
                Err(Route),
            ),
            (
                "a provider key twice",
                format!(
                    r#"{{{HEAD},"reasoning_effort":"low","provider":{{"only":["u"],"only":["u"]}}}}"#
                ),
                Err(Route),
            ),
        ];
        let mut wrong = Vec::new();
        for (name, body, expected) in rows {
            let mut world = World::new(policy());
            let got = world.reserve_bytes(body.as_bytes()).map(|reservation| {
                world
                    .wallet
                    .attempt(reservation.physical)
                    .unwrap()
                    .output_bound
            });
            let recorded = world.wallet.attempts.len();
            // What `reserve` writes is what a file may hold: bytes past the overhead, and some
            // output.
            let replayable = world.wallet.attempts.iter().all(|attempt| {
                attempt.input_bound > world.policy.input_overhead && attempt.output_bound > 0
            }) && world.wallet.is_consistent(&world.policy);
            if got != expected || recorded != usize::from(expected.is_ok()) || !replayable {
                wrong.push(format!(
                    "{name}: {got:?}, {recorded} on the record, replayable {replayable}"
                ));
            }
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// The reader alone answers for the two parts of what it reads. The route's own comparison is
    /// `Route::admits`, whose words are tested with the policy.
    #[test]
    fn what_a_body_says_is_read_exactly() {
        let read = read_wire_body(BODY).unwrap();
        assert_eq!(read.route, route());
        assert_eq!(read.output_tokens, 50);
    }

    /// The invariant itself: one send reserves more than nothing, with every other source of an
    /// amount at its least. The rates are the smallest a policy can write, the overhead is zero,
    /// and the one way left to nothing, a body with no bytes or no output, is refused.
    #[test]
    fn a_send_reserves_more_than_nothing_with_no_overhead_and_the_least_rates() {
        let least = policy_with(|v| {
            v["input_overhead"] = json!(0);
            v["tariff"]["input_per_million"] = json!("1e-400");
            v["tariff"]["cached_input_per_million"] = json!("0");
            v["tariff"]["cache_write_per_million"] = json!("0");
            v["tariff"]["output_per_million"] = json!("1e-400");
            v["tariff"]["request_fee"] = json!("0");
        });
        let mut world = World::new(least);
        let one_token = body_asking_for(1);
        let reservation = world.reserve_bytes(&one_token).unwrap();
        assert!(reservation.amount > Money::ZERO, "{reservation:?}");
        let attempt = world.wallet.attempt(reservation.physical).unwrap();
        assert!(attempt.input_bound > 0 && attempt.output_bound > 0);
        assert!(world.wallet.is_consistent(&world.policy));

        let mut world = World::new(policy_with(|v| v["input_overhead"] = json!(0)));
        assert_eq!(
            world.reserve_bytes(b""),
            Err(BudgetRefusal::BodyNotAdmitted)
        );
        let no_output = body_asking_for(0);
        assert_eq!(
            world.reserve_bytes(&no_output),
            Err(BudgetRefusal::BodyNotAdmitted)
        );
        assert!(world.wallet.attempts.is_empty());

        // So a cost limit of nothing stops every send there can be.
        let mut world = World::new(policy_with(|v| {
            v["input_overhead"] = json!(0);
            v["limits"]["cost_usd"] = json!("0");
        }));
        assert_eq!(world.reserve(), Err(BudgetRefusal::CostExceeded));
        assert_eq!(
            world.reserve_bytes(b""),
            Err(BudgetRefusal::BodyNotAdmitted)
        );
    }

    /// A body with no `messages` is short, not free: it reserves for what it is, inside its limit.
    #[test]
    fn a_body_with_no_messages_reserves_more_than_nothing_inside_its_limit() {
        assert!(!String::from_utf8_lossy(BODY).contains("messages"));
        let mut world = World::new(policy_with(|v| v["limits"]["cost_usd"] = json!("0.00072")));
        let reservation = world.reserve().unwrap();
        assert!(reservation.amount > Money::ZERO);
        assert!(reservation.amount <= world.policy.limits.cost);
    }

    #[test]
    fn the_input_bound_passes_exactly_and_refuses_one_past() {
        let mut exact = World::new(policy_with(|v| v["max_input_tokens"] = json!(110)));
        assert!(exact.reserve().is_ok());
        let mut short = World::new(policy_with(|v| v["max_input_tokens"] = json!(109)));
        assert_eq!(short.reserve(), Err(BudgetRefusal::InputBoundExceeded));
    }

    #[test]
    fn an_output_above_the_policy_is_refused_and_a_lower_one_reserves_less() {
        let mut world = World::new(policy());
        assert_eq!(
            world.reserve_bytes(&body_asking_for(51)),
            Err(BudgetRefusal::OutputBoundExceeded)
        );
        let reservation = world.reserve_bytes(&body_asking_for(10)).unwrap();
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
                    0 => world.start(id),
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
        assert_eq!(world.start(id), Err(BudgetRefusal::NoSuchSend));
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
        let id = world.reserve().unwrap().physical;
        let before = world.wallet.clone();
        let other = [b'y'; 100];
        assert_eq!(
            world.wallet.start(&world.policy, id, &other),
            Err(BudgetRefusal::BytesChanged)
        );
        assert_eq!(world.wallet, before);
        assert_eq!(world.start(id), Ok(()));
    }

    /// The ledger observes the clock before it calls the wallet; here the test does.
    #[test]
    fn a_start_past_the_deadline_is_refused_and_exactly_at_it_passes() {
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        let id = world.reserve().unwrap().physical;
        world.source.advance(1_000);
        world.clock.observe(&mut world.wallet.clock);
        assert_eq!(world.start(id), Ok(()));

        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        let id = world.reserve().unwrap().physical;
        world.source.advance(1_001);
        world.clock.observe(&mut world.wallet.clock);
        assert_eq!(world.start(id), Err(BudgetRefusal::WallExceeded));
        assert_eq!(
            world.wallet.attempt(id).unwrap().state,
            AttemptState::Reserved
        );
        // Nothing left, so it goes back.
        world.wallet.release(id).unwrap();
        assert_eq!(world.wallet.held().unwrap(), Money::ZERO);
    }

    #[test]
    fn a_reservation_holds_and_a_release_gives_back_money_tokens_and_the_send() {
        let mut world = World::new(policy());
        let id = world.reserve().unwrap().physical;
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
    fn a_release_does_not_give_back_a_final_repair() {
        let mut world = World::new(policy());
        let id = world
            .reserve_repair(FinalRepairClaim::New)
            .unwrap()
            .physical;
        world.wallet.release(id).unwrap();
        assert_eq!(world.wallet.final_repair_used(), 1);
        let next = world.reserve_repair(FinalRepairClaim::New).unwrap();
        assert_eq!(next.final_repair, Some(FinalRepairId(2)));
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
            let id = world.reserve().unwrap().physical;
            world.start(id).unwrap();
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
            assert_eq!(world.reserve(), Err(BudgetRefusal::Latched));
            assert_eq!(world.wallet.success_gate(), Err(BudgetRefusal::Latched));
        }
    }

    #[test]
    fn an_unresolved_send_blocks_the_next_send_and_success() {
        for state in [AttemptState::Reserved, AttemptState::Started] {
            let mut world = World::new(policy());
            world.in_state(state);
            assert_eq!(world.reserve(), Err(BudgetRefusal::Unresolved), "{state:?}");
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
        assert_eq!(world.reserve(), Err(BudgetRefusal::Latched));
        assert_eq!(world.wallet.success_gate(), Err(BudgetRefusal::Latched));
    }

    #[test]
    fn host_submissions_count_without_a_send() {
        let mut world = World::new(policy());
        world.submit(stage_kind("one")).unwrap();
        assert_eq!(world.wallet.physical_used(), 0);
        assert_eq!(world.wallet.spent().unwrap(), Money::ZERO);
        assert_eq!(world.wallet.spent_tokens(), 0);
    }

    #[test]
    fn a_host_submission_does_not_escape_the_clock_or_the_latch() {
        // After a send, and (`a_submission_with_no_send_starts_the_clock...`) with none.
        let mut world = World::new(policy_with(|v| v["limits"]["wall_seconds"] = json!(1)));
        world.send().unwrap();
        world.source.advance(1_001);
        assert_eq!(
            world.submit(LogicalKind::General),
            Err(BudgetRefusal::WallExceeded)
        );

        let mut world = World::new(policy());
        world.in_state(AttemptState::Unknown);
        assert_eq!(
            world.submit(LogicalKind::General),
            Err(BudgetRefusal::Latched)
        );
    }

    #[test]
    fn a_host_submission_is_free_of_money_even_with_the_cap_used_up() {
        let mut world = World::new(policy_with(|v| v["limits"]["cost_usd"] = json!("0.00072")));
        world.send().unwrap();
        assert!(world.reserve().is_err());
        assert!(world.submit(LogicalKind::General).is_ok());
    }

    #[test]
    fn a_submission_cannot_name_a_send_that_is_not_settled() {
        let mut world = World::new(policy());
        let id = world.in_state(AttemptState::Settled);
        let response = ResponseIdentity::of_settled(id);
        let other = world.reserve().unwrap().physical;
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
        let logical = world.submit(stage_kind("one")).unwrap();
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
        assert_eq!(
            world.wallet.bind_attempt(LogicalId(99), binding(1)),
            Err(BudgetRefusal::NoSuchLogical)
        );
    }

    #[test]
    fn a_binding_of_another_stage_or_of_no_stage_is_refused() {
        let mut world = World::new(policy());
        let other = world.submit(stage_kind("another")).unwrap();
        assert_eq!(
            world.wallet.bind_attempt(other, binding(1)),
            Err(BudgetRefusal::BindingStageDiffers)
        );
        let general = world.submit(LogicalKind::General).unwrap();
        assert_eq!(
            world.wallet.bind_attempt(general, binding(1)),
            Err(BudgetRefusal::BindingStageDiffers)
        );
        assert_eq!(world.wallet.logical(other).unwrap().attempt, None);
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
    }

    /// Each change is one a file could carry past its digest: every one has to be seen.
    #[test]
    fn records_that_disagree_are_not_consistent() {
        fn settled() -> World {
            let mut world = World::new(policy());
            let id = world.in_state(AttemptState::Settled);
            let response = ResponseIdentity::of_settled(id);
            world
                .wallet
                .record_submission(
                    &world.policy,
                    stage_kind("one"),
                    Some(response.call(0)),
                    &mut world.clock,
                )
                .unwrap();
            world
        }
        type Change = fn(&mut WalletState);
        let changes: Vec<(&str, Change)> = vec![
            ("schema", |w| w.schema = "other".to_string()),
            ("id gap", |w| w.attempts[0].physical = PhysicalId(5)),
            ("logical gap", |w| w.logicals[0].id = LogicalId(5)),
            ("reservation shrunk", |w| {
                w.attempts[0].reserved = Money::ZERO;
            }),
            ("output bound over policy", |w| {
                w.attempts[0].output_bound = 51;
            }),
            ("input bound over policy", |w| {
                w.attempts[0].input_bound = 1001;
            }),
            ("settled without record", |w| {
                w.attempts[0].settlement = None;
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
                w.attempts[0].state = AttemptState::Started;
            }),
            ("released with a record", |w| {
                w.attempts[0].state = AttemptState::Released;
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
            ("a latch that names no send", |w| {
                w.latch = Some(super::Latch {
                    reason: LatchReason::OverReservation,
                    physical: PhysicalId(99),
                });
            }),
            ("a purpose of final repair with no claim", |w| {
                w.attempts[0].purpose = SendPurpose::FinalRepair;
            }),
            ("a claim with no purpose of final repair", |w| {
                w.attempts[0].final_repair = Some(FinalRepairId(1));
                w.clock.final_repair_started_ms = Some(0);
            }),
        ];
        for (name, change) in changes {
            let mut world = settled();
            assert!(world.wallet.is_consistent(&world.policy), "before {name}");
            change(&mut world.wallet);
            assert!(!world.wallet.is_consistent(&world.policy), "{name}");
        }
    }

    #[test]
    fn two_logicals_of_one_source_are_not_consistent() {
        let mut world = World::new(policy());
        let response = world.send().unwrap();
        world
            .wallet
            .record_submission(
                &world.policy,
                LogicalKind::General,
                Some(response.call(0)),
                &mut world.clock,
            )
            .unwrap();
        world.submit(LogicalKind::General).unwrap();
        assert!(world.wallet.is_consistent(&world.policy));
        world.wallet.logicals[1].source = world.wallet.logicals[0].source;
        assert!(!world.wallet.is_consistent(&world.policy));
    }

    #[test]
    fn final_repair_ids_out_of_order_are_not_consistent() {
        let build = || {
            let mut world = World::new(policy());
            world.send_repair(FinalRepairClaim::New).unwrap();
            world
        };
        let mut world = build();
        assert!(world.wallet.is_consistent(&world.policy));
        world.wallet.attempts[0].final_repair = Some(FinalRepairId(2));
        assert!(!world.wallet.is_consistent(&world.policy));
        let mut world = build();
        world.wallet.attempts[0].final_repair = Some(FinalRepairId(0));
        assert!(!world.wallet.is_consistent(&world.policy));
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
    fn a_late_settlement_without_its_latch_or_a_clock_with_nothing_recorded_is_not_consistent() {
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
        world.wallet.attempts.push(AttemptRecord {
            physical: PhysicalId(2),
            ..second
        });
        assert!(!world.wallet.is_consistent(&world.policy));

        let mut world = World::new(policy());
        world.reserve_repair(FinalRepairClaim::New).unwrap();
        world.wallet.clock.final_repair_started_ms = None;
        assert!(!world.wallet.is_consistent(&world.policy));
    }

    /// One row per limit: a record past it with no latch is damage, and a latch explains it.
    #[test]
    fn past_a_limit_with_no_latch_is_not_consistent_and_with_a_latch_it_is() {
        let mut world = World::new(policy());
        world.send().unwrap();
        world.send().unwrap();
        world.submit(stage_kind("one")).unwrap();
        world.submit(stage_kind("one")).unwrap();
        world.send_repair(FinalRepairClaim::New).unwrap();
        assert!(world.wallet.is_consistent(&world.policy));
        let narrower = [
            ("physical_sends", json!(2)),
            ("cost_usd", json!("0.0017")),
            ("tokens", json!(419)),
            ("logical_global", json!(1)),
            ("logical_per_stage", json!(1)),
            ("final_repair_attempts", json!(0)),
        ];
        for (key, value) in narrower {
            let narrow = policy_with(|v| v["limits"][key] = value.clone());
            assert!(!world.wallet.is_consistent(&narrow), "{key}");
            let mut latched = world.wallet.clone();
            latched.set_latch(LatchReason::OverReservation, PhysicalId(1));
            assert!(latched.is_consistent(&narrow), "{key}");
            // The exact value of each one passes, so the row measures that limit.
        }
        let exact = policy_with(|v| {
            v["limits"]["physical_sends"] = json!(3);
            v["limits"]["cost_usd"] = json!("0.0018");
            v["limits"]["tokens"] = json!(420);
            v["limits"]["logical_global"] = json!(2);
            v["limits"]["logical_per_stage"] = json!(2);
            v["limits"]["final_repair_attempts"] = json!(1);
        });
        assert!(world.wallet.is_consistent(&exact));
    }

    fn walk_step(
        world: &mut World,
        rng: &mut Rng,
        last_response: &mut Option<ResponseIdentity>,
        last_logical: &mut Option<LogicalId>,
        last_repair: &mut Option<FinalRepairId>,
    ) -> Result<(), BudgetRefusal> {
        let held = world
            .wallet
            .attempts
            .iter()
            .find(|attempt| attempt.state.holds())
            .map(|attempt| (attempt.physical, attempt.state));
        match (rng.next() % 10, held) {
            (0 | 1, _) => world.reserve().map(|_| ()),
            (2, _) => world
                .reserve_repair(FinalRepairClaim::New)
                .map(|r| *last_repair = r.final_repair),
            (3, _) => match *last_repair {
                Some(id) => world
                    .reserve_repair(FinalRepairClaim::Continue(id))
                    .map(|_| ()),
                None => Ok(()),
            },
            (4 | 5, Some((id, AttemptState::Reserved))) => {
                if rng.next().is_multiple_of(4) {
                    world.wallet.release(id)
                } else {
                    world.start(id)
                }
            }
            (4 | 5, Some((id, AttemptState::Started))) => {
                let (token, prompt, completion) = match rng.next() % 13 {
                    0 => {
                        return world.wallet.mark_unknown(id, UnknownReason::Timeout);
                    }
                    1 => ("5", 100, 40),
                    2 => ("0", 111, 40),
                    3 => ("0", 100, 51),
                    4 => ("0", 0, 0),
                    n => ("0.0001", n * 9, n * 4),
                };
                let cost = BilledCostUsd::from_json_token(token).unwrap();
                world
                    .wallet
                    .settle_reported(
                        &world.policy,
                        id,
                        &ReportedUsage {
                            cost: Some(&cost),
                            prompt_tokens: Some(prompt),
                            completion_tokens: Some(completion),
                            ..ReportedUsage::default()
                        },
                    )
                    .map(|report| {
                        if let super::super::settlement::SettleReport::Settled(outcome) = report {
                            *last_response = Some(outcome.response);
                        }
                    })
            }
            (4 | 5, Some((id, AttemptState::Unknown))) => world
                .wallet
                .settle(&world.policy, id, &billing("0.0001", 10, 10))
                .map(|_| ()),
            (6, _) => {
                let kind = if rng.next().is_multiple_of(2) {
                    LogicalKind::General
                } else {
                    stage_kind("a")
                };
                let source = if rng.next().is_multiple_of(2) {
                    last_response.map(|response| response.call((rng.next() % 3) as u32))
                } else {
                    None
                };
                world
                    .wallet
                    .record_submission(&world.policy, kind, source, &mut world.clock)
                    .map(|id| *last_logical = Some(id))
            }
            (7, _) => {
                world.source.advance(rng.next() % 3_000);
                world.clock.observe(&mut world.wallet.clock);
                Ok(())
            }
            (8, _) => match *last_logical {
                Some(id) => world.wallet.bind_attempt(
                    id,
                    AttemptBinding {
                        run_nonce: "nonce-walk".to_string(),
                        stage: "a".to_string(),
                        target: "target-walk".to_string(),
                        attempt: 1,
                        generation: 1,
                    },
                ),
                None => Ok(()),
            },
            (9, _) if rng.next().is_multiple_of(8) => {
                world.wallet.recover_after_process_end();
                world.clock = RunClock::new(world.source.clone());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Every operation in random order, on a fixed seed. After every step what the code wrote
    /// passes the check, survives the file, and, with no latch, is inside every limit, stated here
    /// without going through the check. Reports past their reservations latch most walks before
    /// they reach a limit: what reaches them all is `a_walk_through_the_file...` in `ledger.rs`.
    #[test]
    fn a_seeded_walk_of_every_operation_stays_consistent_and_inside_the_limits() {
        let (mut latched, mut done, mut refused) = (0_u64, 0_u64, 0_u64);
        for seed in 1..=400_u64 {
            let mut rng = Rng::seeded(seed);
            let mut world = World::new(policy_with(|v| {
                v["limits"]["physical_sends"] = json!(6);
                v["limits"]["logical_global"] = json!(9);
                v["limits"]["logical_per_stage"] = json!(3);
                v["limits"]["cost_usd"] = json!("0.003");
                v["limits"]["tokens"] = json!(800);
                v["limits"]["final_repair_attempts"] = json!(2);
                v["limits"]["final_repair_seconds"] = json!(5);
                v["limits"]["wall_seconds"] = json!(40);
            }));
            let (mut response, mut logical, mut repair) = (None, None, None);
            for _ in 0..50 {
                let result = walk_step(
                    &mut world,
                    &mut rng,
                    &mut response,
                    &mut logical,
                    &mut repair,
                );
                if result.is_ok() {
                    done += 1;
                } else {
                    refused += 1;
                }
                let wallet = &world.wallet;
                assert!(wallet.is_consistent(&world.policy), "seed {seed}");
                let text = serde_json::to_string(wallet).unwrap();
                let back: WalletState = serde_json::from_str(&text).unwrap();
                assert_eq!(&back, wallet, "seed {seed}");
                let mut recovered = wallet.clone();
                recovered.recover_after_process_end();
                assert!(recovered.is_consistent(&world.policy), "seed {seed}");
                if wallet.latch().is_none() {
                    let limits = &world.policy.limits;
                    assert!(wallet.physical_used() <= limits.physical_sends, "{seed}");
                    assert!(
                        wallet.logical_global_used() <= limits.logical_global,
                        "{seed}"
                    );
                    assert!(
                        wallet.logical_stage_used("a") <= limits.logical_per_stage,
                        "{seed}"
                    );
                    assert!(
                        wallet.final_repair_used() <= limits.final_repair_attempts,
                        "{seed}"
                    );
                    let money = wallet.spent().unwrap().units() + wallet.held().unwrap().units();
                    assert!(money <= limits.cost.units(), "{seed}");
                    let tokens = wallet.spent_tokens() + wallet.held_tokens();
                    assert!(tokens <= u128::from(limits.tokens), "{seed}");
                }
            }
            if world.wallet.latch().is_some() {
                latched += 1;
            }
        }
        // The walk has to have reached both sides for any of the above to mean something.
        assert!(done > 1_000, "{done}");
        assert!(refused > 1_000, "{refused}");
        assert!(latched > 50, "{latched}");
    }
}
