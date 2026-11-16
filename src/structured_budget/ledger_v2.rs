//! The ledger of a `usage_tariff` wallet (Issue #1006, design 1): `run-budget.ledger/v2`.
//!
//! It lives where a v1 wallet lives, `<ledger_dir>/<budget_id>`, under the same lock and the same
//! store (`ledger.rs`), and it is another record: a v1 file is refused here and a v2 file there,
//! each for what it is ([`LedgerRefusal::AccountingBasisChanged`]). Nothing converts one into the
//! other, so a wallet keeps the basis it was made with and what it spent stays spent.
//!
//! Each send keeps what v1 keeps (identity, digest of the bytes, bounds, reservation, state) and
//! also how its input was bounded. What it cost is split in two, and the two never mix:
//!
//! - **budget charged** ([`BudgetCharge`]): the fixed tariff applied to the validated usage. This
//!   is what the limits are judged on, and the only thing a response identity is issued from.
//! - **provider billed** ([`ProviderBilled`]): what the provider says it billed. Under this basis
//!   it is always [`ProviderBilled::Unreported`]: no `KnownBilling`, no zero, nothing written as
//!   `provider_billed_cost_usd`.
//!
//! The state machine is v1's: reserved, started, settled or unknown, released only before it
//! left; an unknown send keeps its reservation and sets the latch for good; a process that ended
//! with a send started leaves it unknown as `InterruptedAfterStart`; a late valid usage settles the
//! same send once and the latch stays.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::clock::{BudgetClock, ClockRecord, RunClock};
use super::identity::{
    AttemptBinding, FinalRepairClaim, FinalRepairId, LogicalId, LogicalKind, PhysicalId,
    SendPurpose, SubmissionSource,
};
use super::ledger::{
    CommandWrites, HeldStore, LedgerEntry, LedgerFailure, LedgerRefusal, Store, canonical_profile,
    decode_body, encode_body, hold_store_at, pinned_profile_agrees, schema_of,
};
use super::policy_v2::{
    ACCOUNTING_BASIS_USAGE_TARIFF, CacheWriteMode, InputBound, RunBudgetPolicyV2,
};
use super::reservation::{
    AttemptState, BudgetRefusal, Latch, LatchReason, LogicalRecord, Money, within_cap,
};
use super::settlement::UnknownReason;
use super::usage_tariff::{ChargeStop, charge, reservation_bound};
use crate::providers::usage_observation::{ResponsesUsage, UsageStop};

pub(crate) const WALLET_V2_SCHEMA: &str = "run-budget.ledger/v2";

/// How a send's input was bounded before it left (Issue #1006, design 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputBoundMethod {
    /// Text, calls, call outputs and tool schemas only: the body's bytes and the overhead.
    BodyBytesPlusOverhead,
    /// An opaque item (a reasoning item) is in it: the provider's declared most input.
    ProviderMaxInputTokens,
}

/// What the provider reported it billed. Under `usage_tariff` there is no report to read, and it
/// stays unreported: never a zero, never the tariff's figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderBilled {
    Unreported,
}

/// Why a response's top level is not one this wallet's route answers with. Fixed words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResponseStop {
    /// `model` or `service_tier` comes twice.
    DuplicateKey,
    ModelMissing,
    /// Not one of the policy's `response_models`, by exact match.
    ModelNotDeclared,
    ServiceTierMissing,
    /// `null`, not a string, or a tier other than `default`.
    ServiceTierNotDefault,
}

/// Why a started send has no budget charge. Fixed words, in three families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum UnknownCause {
    /// The send ended with no response to read: timeout, cancel, HTTP error, cut body, a process
    /// that ended with it started.
    Transport { reason: UnknownReason },
    /// The response's usage did not pass the reader (`providers::usage_observation`).
    Usage { stop: UsageStop },
    /// The response's top level is not this route's.
    Response { stop: ResponseStop },
    /// The usage passed the reader and the tariff cannot charge it.
    Charge { stop: ChargeStop },
}

/// A validated usage as the ledger keeps it: the reader's counts, nothing filled in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UsageCounts {
    pub(crate) input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) cached_tokens: u64,
    pub(crate) reasoning_tokens: Option<u64>,
    pub(crate) total_tokens: Option<u64>,
    pub(crate) cache_write_tokens: Option<u64>,
    pub(crate) cache_write_30m_tokens: Option<u64>,
    pub(crate) cache_write_12h_tokens: Option<u64>,
}

impl UsageCounts {
    pub(crate) fn of(usage: &ResponsesUsage) -> Self {
        Self {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cached_tokens: usage.cached_tokens,
            reasoning_tokens: usage.reasoning_tokens,
            total_tokens: usage.total_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            cache_write_30m_tokens: usage.cache_write_30m_tokens,
            cache_write_12h_tokens: usage.cache_write_12h_tokens,
        }
    }

    pub(crate) fn usage(&self) -> ResponsesUsage {
        ResponsesUsage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cached_tokens: self.cached_tokens,
            reasoning_tokens: self.reasoning_tokens,
            total_tokens: self.total_tokens,
            cache_write_tokens: self.cache_write_tokens,
            cache_write_30m_tokens: self.cache_write_30m_tokens,
            cache_write_12h_tokens: self.cache_write_12h_tokens,
        }
    }

    /// The reader's rules that do not depend on the mode, again, for a record read back.
    fn is_valid(&self) -> bool {
        let written = self.cache_write_tokens.unwrap_or(0);
        let split = match (
            self.cache_write_tokens,
            self.cache_write_30m_tokens,
            self.cache_write_12h_tokens,
        ) {
            (_, None, None) => true,
            (None, _, _) => false,
            (Some(total), Some(a), Some(b)) => a.checked_add(b) == Some(total),
            (Some(total), Some(one), None) | (Some(total), None, Some(one)) => one <= total,
        };
        let sum = self.input_tokens.checked_add(self.output_tokens);
        split
            && self
                .cached_tokens
                .checked_add(written)
                .is_some_and(|both| both <= self.input_tokens)
            && self
                .reasoning_tokens
                .is_none_or(|reasoning| reasoning <= self.output_tokens)
            && sum.is_some()
            && self.total_tokens.is_none_or(|total| Some(total) == sum)
    }
}

/// What the fixed tariff charged one response, with the usage it charged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BudgetCharge {
    pub(crate) usage: UsageCounts,
    /// The input-length tier that priced the whole request.
    pub(crate) tier: u64,
    /// From the fixed tariff. Never a billed amount.
    pub(crate) charged: Money,
    /// It arrived after the send was already `unknown`.
    pub(crate) late: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttemptRecordV2 {
    pub(crate) physical: PhysicalId,
    pub(crate) purpose: SendPurpose,
    pub(crate) final_repair: Option<FinalRepairId>,
    /// Of the exact bytes reserved and sent.
    pub(crate) request_sha256: String,
    pub(crate) input_bound_method: InputBoundMethod,
    pub(crate) input_bound: u64,
    pub(crate) output_bound: u64,
    pub(crate) reserved: Money,
    pub(crate) state: AttemptState,
    pub(crate) unknown: Option<UnknownCause>,
    /// A valid usage of a response that could not be charged, kept as observed.
    pub(crate) observed: Option<UsageCounts>,
    pub(crate) charged: Option<BudgetCharge>,
    pub(crate) provider_billed: ProviderBilled,
}

/// Everything a `usage_tariff` wallet is, as the ledger stores it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WalletStateV2 {
    pub(crate) schema: String,
    pub(crate) accounting_basis: String,
    pub(crate) budget_id: String,
    /// Of the policy's full bytes.
    pub(crate) policy_sha256: String,
    pub(crate) workspace_sha256: String,
    pub(crate) profile: Option<String>,
    pub(crate) clock: ClockRecord,
    pub(crate) attempts: Vec<AttemptRecordV2>,
    pub(crate) logicals: Vec<LogicalRecord>,
    pub(crate) latch: Option<Latch>,
}

/// What the limits are asked about.
pub(super) enum Demand<'a> {
    Send {
        amount: Money,
        tokens: u64,
        final_repair: Option<FinalRepairClaim>,
    },
    Submission {
        kind: &'a LogicalKind,
    },
}

impl WalletStateV2 {
    pub(super) fn new(
        policy: &RunBudgetPolicyV2,
        workspace_sha256: String,
        profile: Option<String>,
    ) -> Self {
        Self {
            schema: WALLET_V2_SCHEMA.to_string(),
            accounting_basis: ACCOUNTING_BASIS_USAGE_TARIFF.to_string(),
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

    pub(crate) fn attempt(&self, physical: PhysicalId) -> Option<&AttemptRecordV2> {
        self.attempts
            .iter()
            .find(|attempt| attempt.physical == physical)
    }

    pub(super) fn attempt_mut(&mut self, physical: PhysicalId) -> Option<&mut AttemptRecordV2> {
        self.attempts
            .iter_mut()
            .find(|attempt| attempt.physical == physical)
    }

    pub(crate) fn latch(&self) -> Option<&Latch> {
        self.latch.as_ref()
    }

    /// Set once; nothing clears it.
    pub(super) fn set_latch(&mut self, reason: LatchReason, physical: PhysicalId) {
        self.latch.get_or_insert(Latch { reason, physical });
    }

    /// What the budget has charged. Never a billed amount.
    pub(crate) fn spent(&self) -> Result<Money, BudgetRefusal> {
        self.attempts
            .iter()
            .filter_map(|attempt| attempt.charged.as_ref())
            .try_fold(Money::ZERO, |sum, record| sum.checked_add(record.charged))
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
            .filter_map(|attempt| attempt.charged.as_ref())
            .map(|record| {
                u128::from(record.usage.input_tokens) + u128::from(record.usage.output_tokens)
            })
            .sum()
    }

    pub(crate) fn held_tokens(&self) -> u128 {
        self.attempts
            .iter()
            .filter(|attempt| attempt.state.holds())
            .map(|attempt| u128::from(attempt.input_bound) + u128::from(attempt.output_bound))
            .sum()
    }

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
        self.attempts
            .iter()
            .filter_map(|attempt| attempt.final_repair)
            .map(|id| id.0)
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn success_gate(&self) -> Result<(), BudgetRefusal> {
        if self.latch.is_some() {
            return Err(BudgetRefusal::Latched);
        }
        if self.attempts.iter().any(|attempt| attempt.state.holds()) {
            return Err(BudgetRefusal::Unresolved);
        }
        Ok(())
    }

    /// Every limit, for one send or one submission, as v1 judges them. Exactly at a limit passes.
    pub(super) fn check_limits(
        &self,
        policy: &RunBudgetPolicyV2,
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

    /// A claim of a final repair goes with that purpose, both ways, and continues one that exists.
    pub(super) fn refuse_final_repair_claim(
        &self,
        purpose: SendPurpose,
        claim: Option<FinalRepairClaim>,
    ) -> Result<(), BudgetRefusal> {
        if claim.is_some() != (purpose == SendPurpose::FinalRepair) {
            return Err(BudgetRefusal::PurposeDiffers);
        }
        if let Some(FinalRepairClaim::Continue(id)) = claim
            && (id.0 == 0 || id.0 > self.final_repair_used())
        {
            return Err(BudgetRefusal::NoSuchFinalRepair);
        }
        Ok(())
    }

    pub(super) fn past_the_deadline(&self, policy: &RunBudgetPolicyV2) -> bool {
        self.clock.high_water_elapsed_ms > policy.limits.wall_ms
    }

    pub(super) fn past_the_final_repair_window(&self, policy: &RunBudgetPolicyV2) -> bool {
        self.clock.final_repair_started_ms.is_some_and(|began| {
            self.clock.high_water_elapsed_ms.saturating_sub(began) > policy.limits.final_repair_ms
        })
    }

    pub(super) fn attempt_in(
        &mut self,
        physical: PhysicalId,
        from: AttemptState,
        to: AttemptState,
    ) -> Result<&mut AttemptRecordV2, BudgetRefusal> {
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

    /// `reserved` to `released`: nothing has left.
    pub(super) fn release(&mut self, physical: PhysicalId) -> Result<(), BudgetRefusal> {
        let attempt = self.attempt_in(physical, AttemptState::Reserved, AttemptState::Released)?;
        attempt.state = AttemptState::Released;
        Ok(())
    }

    /// `started` to `unknown`, keeping what was observed. The reservation stays held and the
    /// wallet stops.
    pub(super) fn mark_unknown(
        &mut self,
        physical: PhysicalId,
        cause: UnknownCause,
        observed: Option<UsageCounts>,
    ) -> Result<(), BudgetRefusal> {
        let attempt = self.attempt_in(physical, AttemptState::Started, AttemptState::Unknown)?;
        attempt.state = AttemptState::Unknown;
        attempt.unknown = Some(cause);
        attempt.observed = observed;
        self.set_latch(LatchReason::UnknownBilling, physical);
        Ok(())
    }

    /// One submission the host looked at, as v1 records it: its `source` must be a send the budget
    /// charged, once.
    pub(super) fn record_submission(
        &mut self,
        policy: &RunBudgetPolicyV2,
        kind: LogicalKind,
        source: Option<SubmissionSource>,
        clock: &mut RunClock,
    ) -> Result<LogicalId, BudgetRefusal> {
        self.success_gate()?;
        if let Some(source) = source {
            let attempt = self
                .attempt(source.physical)
                .ok_or(BudgetRefusal::NoSuchSend)?;
            if attempt.state != AttemptState::Settled || attempt.charged.is_none() {
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
    /// started send may have left, so it is unknown, keeps its reservation, and stops the wallet.
    pub(super) fn recover_after_process_end(&mut self) {
        let mut interrupted = None;
        for attempt in &mut self.attempts {
            match attempt.state {
                AttemptState::Reserved => attempt.state = AttemptState::Released,
                AttemptState::Started => {
                    attempt.state = AttemptState::Unknown;
                    attempt.unknown = Some(UnknownCause::Transport {
                        reason: UnknownReason::InterruptedAfterStart,
                    });
                    interrupted = Some(attempt.physical);
                }
                AttemptState::Settled | AttemptState::Unknown | AttemptState::Released => {}
            }
        }
        if let Some(physical) = interrupted {
            self.set_latch(LatchReason::InterruptedAfterStart, physical);
        }
    }

    /// Whether these records can be what this code wrote under this policy: amounts and charges
    /// computed again, states agreeing with what they carry.
    pub(crate) fn is_consistent(&self, policy: &RunBudgetPolicyV2) -> bool {
        self.schema == WALLET_V2_SCHEMA
            && self.accounting_basis == ACCOUNTING_BASIS_USAGE_TARIFF
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

    fn is_within_limits(&self, policy: &RunBudgetPolicyV2) -> bool {
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
                    self.attempt(source.physical).is_some_and(|attempt| {
                        attempt.state == AttemptState::Settled && attempt.charged.is_some()
                    }) && self.logicals[..index]
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

    /// The bound a method gives is one `reserve` could have chosen under this policy.
    fn bound_is_consistent(input: &InputBound, method: InputBoundMethod, bound: u64) -> bool {
        match method {
            InputBoundMethod::BodyBytesPlusOverhead => bound > input.input_overhead,
            InputBoundMethod::ProviderMaxInputTokens => {
                input.provider_max_input_tokens == Some(bound)
            }
        }
    }

    fn attempt_is_consistent(
        &self,
        policy: &RunBudgetPolicyV2,
        index: usize,
        attempt: &AttemptRecordV2,
    ) -> bool {
        let identified = attempt.physical.0 == index as u64 + 1
            && Self::bound_is_consistent(
                &policy.input_bound,
                attempt.input_bound_method,
                attempt.input_bound,
            )
            && attempt.output_bound > 0
            && attempt.input_bound <= policy.max_input_tokens
            && attempt.output_bound <= policy.max_output_tokens
            && attempt.provider_billed == ProviderBilled::Unreported
            && reservation_bound(&policy.tariff, attempt.input_bound, attempt.output_bound)
                == Ok(attempt.reserved);
        identified
            && match (attempt.state, &attempt.charged) {
                (AttemptState::Reserved | AttemptState::Started | AttemptState::Released, None) => {
                    attempt.unknown.is_none() && attempt.observed.is_none()
                }
                (AttemptState::Unknown, None) => {
                    attempt.unknown.is_some()
                        && self.latch.is_some()
                        && attempt.observed.is_none_or(|observed| observed.is_valid())
                }
                (AttemptState::Settled, Some(record)) => {
                    record.late == attempt.unknown.is_some()
                        && (!record.late || self.latch.is_some())
                        && (attempt.observed.is_none() || record.late)
                        && Self::charge_is_consistent(policy, record)
                        && (self.latch.is_some()
                            || (record.charged <= attempt.reserved
                                && record.usage.input_tokens <= attempt.input_bound
                                && record.usage.output_tokens <= attempt.output_bound))
                }
                _ => false,
            }
    }

    /// What the tariff gives for the usage kept is the tier and the amount kept.
    fn charge_is_consistent(policy: &RunBudgetPolicyV2, record: &BudgetCharge) -> bool {
        record.usage.is_valid()
            && charge(&policy.tariff, &record.usage.usage()).is_ok_and(|charged| {
                charged.charge == record.charged && charged.tier as u64 == record.tier
            })
    }
}

/// How a `usage_tariff` wallet is opened: v1's [`super::ledger::LedgerOpen`] with this basis's
/// declaration.
pub(crate) struct LedgerOpenV2<'a> {
    pub(crate) policy: Arc<RunBudgetPolicyV2>,
    pub(crate) workspace: &'a Path,
    pub(crate) explicit_profile: Option<&'a str>,
    pub(crate) entry: LedgerEntry,
    pub(crate) clock: Arc<dyn BudgetClock>,
    pub(crate) command_writes: Option<CommandWrites>,
}

/// What a POST of this basis needs: the bytes that were reserved, from a start on disk. Only
/// [`BudgetLedgerV2::start`] makes one.
#[must_use = "a permit is one POST, and a start with no POST leaves its send unresolved"]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ResponsesPermit<'a> {
    physical: PhysicalId,
    body: &'a [u8],
}

impl<'a> ResponsesPermit<'a> {
    pub(crate) fn physical(&self) -> PhysicalId {
        self.physical
    }

    pub(crate) fn into_body(self) -> &'a [u8] {
        self.body
    }
}

/// One `usage_tariff` wallet, its lock held for as long as this value lives.
#[derive(Debug)]
pub(crate) struct BudgetLedgerV2 {
    policy: Arc<RunBudgetPolicyV2>,
    wallet: WalletStateV2,
    store: Store,
    clock: RunClock,
    failed: Option<LedgerRefusal>,
    made_here: bool,
}

/// The wallet already on disk, held to the declaration and the run that opened it. A record of
/// another version or basis is refused for that, before it is read as this one.
fn continue_wallet(
    bytes: &[u8],
    policy: &RunBudgetPolicyV2,
    workspace_sha256: &str,
    explicit_profile: Option<&str>,
) -> Result<WalletStateV2, LedgerRefusal> {
    if schema_of(bytes)? != WALLET_V2_SCHEMA {
        return Err(LedgerRefusal::AccountingBasisChanged);
    }
    let wallet: WalletStateV2 = decode_body(bytes)?;
    if wallet.accounting_basis != ACCOUNTING_BASIS_USAGE_TARIFF {
        return Err(LedgerRefusal::AccountingBasisChanged);
    }
    if wallet.budget_id != policy.budget_id {
        return Err(LedgerRefusal::Corrupt);
    }
    if wallet.policy_sha256 != policy.sha256 {
        return Err(LedgerRefusal::PolicyChanged);
    }
    if wallet.workspace_sha256 != workspace_sha256 {
        return Err(LedgerRefusal::WorkspaceChanged);
    }
    if !pinned_profile_agrees(wallet.profile.as_deref(), explicit_profile) {
        return Err(LedgerRefusal::ProfileChanged);
    }
    if !wallet.is_consistent(policy) {
        return Err(LedgerRefusal::Corrupt);
    }
    Ok(wallet)
}

impl BudgetLedgerV2 {
    pub(crate) fn open(request: LedgerOpenV2<'_>) -> Result<Self, LedgerRefusal> {
        let policy = request.policy;
        let HeldStore {
            mut store,
            workspace_sha256,
            explicit_profile,
            found,
        } = hold_store_at(
            &policy.ledger_dir,
            &policy.budget_id,
            request.workspace,
            request.explicit_profile,
            request.command_writes.as_ref(),
        )?;
        let mut made_here = false;
        let wallet = match found {
            Some(bytes) => {
                let found = continue_wallet(
                    &bytes,
                    &policy,
                    &workspace_sha256,
                    explicit_profile.as_deref(),
                )?;
                store.expect(&bytes);
                let mut wallet = found.clone();
                wallet.recover_after_process_end();
                if wallet.profile.is_none() {
                    wallet.profile = explicit_profile;
                }
                if wallet != found {
                    store.write_encoded(&encode_body(&wallet)?)?;
                }
                wallet
            }
            None if request.entry == LedgerEntry::Resume => return Err(LedgerRefusal::Missing),
            None => {
                let wallet = WalletStateV2::new(&policy, workspace_sha256, explicit_profile);
                store.write_encoded(&encode_body(&wallet)?)?;
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

    pub(crate) fn policy(&self) -> &RunBudgetPolicyV2 {
        &self.policy
    }

    pub(crate) fn wallet(&self) -> &WalletStateV2 {
        &self.wallet
    }

    /// As v1's [`super::ledger::BudgetLedger::observed`]: the wallet, only while the file is what
    /// this handle last wrote, reading and changing nothing (Issue #1015).
    pub(crate) fn observed(&self) -> Result<&WalletStateV2, LedgerRefusal> {
        if let Some(refusal) = self.failed {
            return Err(refusal);
        }
        self.store.verify_unchanged()?;
        Ok(&self.wallet)
    }

    /// As v1: one this open made has no run before it to go on with.
    pub(crate) fn refuse_resume(&mut self) -> Result<(), LedgerRefusal> {
        if self.made_here {
            return Err(LedgerRefusal::Missing);
        }
        self.verified()
    }

    pub(crate) fn pin_profile(&mut self, explicit: &str) -> Result<(), LedgerRefusal> {
        let named = canonical_profile(explicit)?;
        self.verified()?;
        match self.wallet.profile.as_deref() {
            Some(pinned) if pinned == named => Ok(()),
            Some(_) => Err(LedgerRefusal::ProfileChanged),
            None => {
                let mut next = self.wallet.clone();
                next.profile = Some(named);
                let written = encode_body(&next).and_then(|bytes| self.store.write_encoded(&bytes));
                if let Err(refusal) = written {
                    self.failed = Some(refusal);
                    return Err(refusal);
                }
                self.wallet = next;
                Ok(())
            }
        }
    }

    fn verified(&mut self) -> Result<(), LedgerRefusal> {
        if let Some(refusal) = self.failed {
            return Err(refusal);
        }
        self.store.verify_unchanged().inspect_err(|refusal| {
            self.failed = Some(*refusal);
        })
    }

    /// The change on a copy, the copy on disk, and only then the change here. A change that
    /// cannot reach the disk takes no effect, and every change after it is refused.
    pub(super) fn apply<R>(
        &mut self,
        change: impl FnOnce(
            &mut WalletStateV2,
            &RunBudgetPolicyV2,
            &mut RunClock,
        ) -> Result<R, BudgetRefusal>,
    ) -> Result<R, LedgerFailure> {
        self.verified()?;
        let mut next = self.wallet.clone();
        self.clock.observe(&mut next.clock);
        let result = change(&mut next, &self.policy, &mut self.clock)?;
        let written = encode_body(&next).and_then(|bytes| self.store.write_encoded(&bytes));
        if let Err(refusal) = written {
            self.failed = Some(refusal);
            return Err(refusal.into());
        }
        self.wallet = next;
        Ok(result)
    }

    /// The start is on disk when this returns, and the permit is what a POST takes its bytes from.
    pub(crate) fn start<'a>(
        &mut self,
        physical: PhysicalId,
        body: &'a [u8],
    ) -> Result<ResponsesPermit<'a>, LedgerFailure> {
        self.apply(|wallet, policy, _| wallet.start(policy, physical, body))?;
        Ok(ResponsesPermit { physical, body })
    }

    pub(crate) fn release(&mut self, physical: PhysicalId) -> Result<(), LedgerFailure> {
        self.apply(|wallet, _, _| wallet.release(physical))
    }

    pub(crate) fn mark_unknown(
        &mut self,
        physical: PhysicalId,
        reason: UnknownReason,
    ) -> Result<(), LedgerFailure> {
        self.apply(|wallet, _, _| {
            wallet.mark_unknown(physical, UnknownCause::Transport { reason }, None)
        })
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

    pub(crate) fn success_gate(&mut self) -> Result<(), LedgerFailure> {
        self.verified()?;
        self.wallet.success_gate().map_err(LedgerFailure::from)
    }

    /// The most output a body of this wallet may ask for.
    pub(crate) fn max_output_tokens(&self) -> u64 {
        self.policy.max_output_tokens
    }

    /// Whether the policy prices writes, for a caller that shows what it records.
    pub(crate) fn cache_write_mode(&self) -> CacheWriteMode {
        self.policy.tariff.cache_write_mode
    }
}
