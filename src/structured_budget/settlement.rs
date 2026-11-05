//! What a provider reported, and the only way a send gets settled.
//!
//! Settling takes a [`KnownBilling`], and the only way to one is a reported cost with both token
//! counts. An explicit zero is a reported cost; an absent one has no value of this type, so there
//! is no path that settles a send at zero because something was missing.
//!
//! A report of no prompt tokens for a body that went out is not known either, whatever it says it
//! cost: a proxy that fills in zeros is the way an unobserved bill would be used up as nothing. The
//! same goes for a report of a body that comes to nothing at all, which only a cached rate of zero
//! can price. [`WalletState::report_hides_the_body`] is the one statement of both, for a report and
//! for a record. The price of it: a real response that was all cached at a free rate, with no
//! output and nothing billed, stops the wallet.
//!
//! [`WalletState::settle_reported`] is the one call that takes a report and leaves the send either
//! settled or unknown, a report whose amount is past counting included. Its only refusals are for
//! a send that was not out to be reported on.
//!
//! What is consumed is the larger of the billed amount and the tariff's figure. The fields stay
//! apart, and the billed one is never rewritten. `consumed` is what the limits are judged on;
//! a comparison of what a run cost reads `billed`.

use serde::{Deserialize, Serialize};

use super::identity::{PhysicalId, ResponseIdentity};
use super::policy::{RunBudgetPolicy, Tariff};
use super::reservation::{
    AttemptState, BudgetRefusal, LatchReason, Money, WalletState, per_million_up, token_cost,
};
use crate::providers::BilledCostUsd;

/// Why a started send has no known billing. Fixed words only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnknownReason {
    CostMissing,
    UsageMissing,
    UsageInconsistent,
    BodyTruncated,
    ParseFailed,
    HttpError,
    Timeout,
    Cancelled,
    SendError,
    /// A process ended with the send started; found by the next one to open the ledger.
    InterruptedAfterStart,
}

/// The usage of one response, as parsed, with nothing filled in.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ReportedUsage<'a> {
    pub(crate) cost: Option<&'a BilledCostUsd>,
    pub(crate) prompt_tokens: Option<u64>,
    pub(crate) completion_tokens: Option<u64>,
    pub(crate) cached_tokens: Option<u64>,
    pub(crate) reasoning_tokens: Option<u64>,
}

/// A response whose billing is known. Its fields cannot be set from outside this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KnownBilling {
    billed_text: String,
    billed: Money,
    prompt_tokens: u64,
    completion_tokens: u64,
    cached_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
}

impl KnownBilling {
    pub(crate) fn from_reported(reported: &ReportedUsage<'_>) -> Result<Self, UnknownReason> {
        let cost = reported.cost.ok_or(UnknownReason::CostMissing)?;
        let (Some(prompt_tokens), Some(completion_tokens)) =
            (reported.prompt_tokens, reported.completion_tokens)
        else {
            return Err(UnknownReason::UsageMissing);
        };
        if reported
            .cached_tokens
            .is_some_and(|cached| cached > prompt_tokens)
            || reported
                .reasoning_tokens
                .is_some_and(|reasoning| reasoning > completion_tokens)
            || prompt_tokens.checked_add(completion_tokens).is_none()
        {
            return Err(UnknownReason::UsageInconsistent);
        }
        let billed = Money::from_billed(cost).map_err(|_| UnknownReason::UsageInconsistent)?;
        Ok(Self {
            billed_text: cost.as_decimal_str().to_string(),
            billed,
            prompt_tokens,
            completion_tokens,
            cached_tokens: reported.cached_tokens,
            reasoning_tokens: reported.reasoning_tokens,
        })
    }
}

/// One settled send, as the ledger keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettlementRecord {
    /// The provider's text, as it sent it.
    pub(crate) billed_usd: String,
    pub(crate) billed: Money,
    /// From the fixed tariff. Never a billed amount.
    pub(crate) computed: Money,
    pub(crate) consumed: Money,
    pub(crate) prompt_tokens: u64,
    pub(crate) completion_tokens: u64,
    pub(crate) cached_tokens: Option<u64>,
    pub(crate) reasoning_tokens: Option<u64>,
    /// It arrived after the send was already `unknown`.
    pub(crate) late: bool,
}

/// What the tariff says one response costs, rounded up. Cached tokens are inside the prompt and
/// reasoning tokens inside the completion, so neither is added on top. The usage does not say
/// which prompt tokens wrote to a cache, so the uncached ones take the higher of the two rates.
pub(crate) fn computed_charge(
    tariff: &Tariff,
    prompt_tokens: u64,
    completion_tokens: u64,
    cached_tokens: Option<u64>,
) -> Result<Money, BudgetRefusal> {
    let cached = cached_tokens.unwrap_or(0);
    let uncached = prompt_tokens
        .checked_sub(cached)
        .ok_or(BudgetRefusal::Overflow)?;
    let uncached_rate = tariff.input.max(tariff.cache_write);
    let numerator = token_cost(uncached, uncached_rate)?
        .checked_add(token_cost(cached, tariff.cached_input)?)
        .and_then(|sum| sum.checked_add(token_cost(completion_tokens, tariff.output).ok()?))
        .ok_or(BudgetRefusal::Overflow)?;
    per_million_up(numerator)
        .checked_add(tariff.request_fee)
        .ok_or(BudgetRefusal::Overflow)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettleOutcome {
    pub(crate) response: ResponseIdentity,
    pub(crate) consumed: Money,
    pub(crate) latched: bool,
}

/// How a report left a send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SettleReport {
    Settled(SettleOutcome),
    Unknown(UnknownReason),
}

impl WalletState {
    /// A send that carried a body cannot have read no prompt tokens or have cost nothing. The bound
    /// is the body's bytes and the overhead, so past the overhead there were bytes.
    pub(super) fn report_hides_the_body(
        policy: &RunBudgetPolicy,
        input_bound: u64,
        prompt_tokens: u64,
        consumed: Money,
    ) -> bool {
        input_bound > policy.input_overhead && (prompt_tokens == 0 || consumed == Money::ZERO)
    }

    /// Takes one response's report. What is not known, which includes what [`Self::settle`] cannot
    /// take as known, leaves the send `unknown`, holding its reservation, and stops the wallet. A
    /// late report that is not known changes nothing.
    pub(super) fn settle_reported(
        &mut self,
        policy: &RunBudgetPolicy,
        physical: PhysicalId,
        reported: &ReportedUsage<'_>,
    ) -> Result<SettleReport, BudgetRefusal> {
        let attempt = self.attempt(physical).ok_or(BudgetRefusal::NoSuchSend)?;
        let from = attempt.state;
        if !matches!(from, AttemptState::Started | AttemptState::Unknown) {
            return Err(BudgetRefusal::IllegalTransition {
                from,
                to: AttemptState::Settled,
            });
        }
        let reason = match KnownBilling::from_reported(reported) {
            Err(reason) => reason,
            Ok(billing) => match self.settle(policy, physical, &billing) {
                Ok(outcome) => return Ok(SettleReport::Settled(outcome)),
                // `settle` refuses before it changes anything, so the send is as it was.
                Err(BudgetRefusal::UsageInconsistent | BudgetRefusal::Overflow) => {
                    UnknownReason::UsageInconsistent
                }
                Err(other) => return Err(other),
            },
        };
        if from == AttemptState::Started {
            self.mark_unknown(physical, reason)?;
        }
        Ok(SettleReport::Unknown(reason))
    }

    /// `started` or `unknown` to `settled`, once. A late settlement leaves the latch set, and an
    /// amount past the reservation keeps its value and sets it. A response after the deadline, or a
    /// final repair's after its window, as the ledger last observed it, is settled at its cost and
    /// is not a success. Replacing a reservation with the known amount is not a release.
    pub(super) fn settle(
        &mut self,
        policy: &RunBudgetPolicy,
        physical: PhysicalId,
        billing: &KnownBilling,
    ) -> Result<SettleOutcome, BudgetRefusal> {
        let attempt = self.attempt(physical).ok_or(BudgetRefusal::NoSuchSend)?;
        let from = attempt.state;
        if !matches!(from, AttemptState::Started | AttemptState::Unknown) {
            return Err(BudgetRefusal::IllegalTransition {
                from,
                to: AttemptState::Settled,
            });
        }
        let (reserved, input_bound, output_bound, is_final_repair) = (
            attempt.reserved,
            attempt.input_bound,
            attempt.output_bound,
            attempt.final_repair.is_some(),
        );
        let computed = computed_charge(
            &policy.tariff,
            billing.prompt_tokens,
            billing.completion_tokens,
            billing.cached_tokens,
        )?;
        let consumed = billing.billed.max(computed);
        if Self::report_hides_the_body(policy, input_bound, billing.prompt_tokens, consumed) {
            return Err(BudgetRefusal::UsageInconsistent);
        }
        // What the wallet would then have spent has to be a number before anything changes.
        self.spent()?
            .checked_add(consumed)
            .ok_or(BudgetRefusal::Overflow)?;
        let record = SettlementRecord {
            billed_usd: billing.billed_text.clone(),
            billed: billing.billed,
            computed,
            consumed,
            prompt_tokens: billing.prompt_tokens,
            completion_tokens: billing.completion_tokens,
            cached_tokens: billing.cached_tokens,
            reasoning_tokens: billing.reasoning_tokens,
            late: from == AttemptState::Unknown,
        };
        let attempt = self
            .attempt_mut(physical)
            .ok_or(BudgetRefusal::NoSuchSend)?;
        attempt.state = AttemptState::Settled;
        attempt.settlement = Some(record);
        if consumed > reserved {
            self.set_latch(LatchReason::OverReservation, physical);
        }
        if billing.prompt_tokens > input_bound {
            self.set_latch(LatchReason::InputOverBound, physical);
        }
        if billing.completion_tokens > output_bound {
            self.set_latch(LatchReason::OutputOverBound, physical);
        }
        if self.past_the_deadline(policy) {
            self.set_latch(LatchReason::ResponseAfterDeadline, physical);
        }
        if is_final_repair && self.past_the_final_repair_window(policy) {
            self.set_latch(LatchReason::ResponseAfterFinalRepairWindow, physical);
        }
        Ok(SettleOutcome {
            response: ResponseIdentity::of_settled(physical),
            consumed,
            latched: self.latch().is_some(),
        })
    }

    /// Whether a settled record is what [`Self::settle`] would have written for it.
    pub(super) fn settlement_is_consistent(
        policy: &RunBudgetPolicy,
        record: &SettlementRecord,
    ) -> bool {
        let usable = record
            .cached_tokens
            .is_none_or(|cached| cached <= record.prompt_tokens)
            && record
                .reasoning_tokens
                .is_none_or(|reasoning| reasoning <= record.completion_tokens);
        usable
            && Money::parse_decimal_up(&record.billed_usd) == Ok(record.billed)
            && computed_charge(
                &policy.tariff,
                record.prompt_tokens,
                record.completion_tokens,
                record.cached_tokens,
            ) == Ok(record.computed)
            && record.consumed == record.billed.max(record.computed)
    }
}

#[cfg(test)]
mod tests {
    use super::super::clock::RunClock;
    use super::super::identity::PhysicalId;
    use super::super::policy::RunBudgetPolicy;
    use super::super::reservation::ReserveRequest;
    use super::super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money, WalletState};
    use super::super::test_support::{FakeClock, policy, policy_with, request};
    use super::{KnownBilling, ReportedUsage, SettleReport, UnknownReason, computed_charge};
    use crate::providers::BilledCostUsd;

    fn cost(token: &str) -> BilledCostUsd {
        BilledCostUsd::from_json_token(token).unwrap()
    }

    fn money(text: &str) -> Money {
        Money::parse_decimal_up(text).unwrap()
    }

    fn known(token: &str, prompt: u64, completion: u64) -> KnownBilling {
        let cost = cost(token);
        KnownBilling::from_reported(&ReportedUsage {
            cost: Some(&cost),
            prompt_tokens: Some(prompt),
            completion_tokens: Some(completion),
            ..ReportedUsage::default()
        })
        .unwrap()
    }

    fn started(policy: &RunBudgetPolicy) -> (WalletState, PhysicalId) {
        let body = request();
        let clock = FakeClock::at(1_000);
        let mut clock = RunClock::new(clock);
        let mut wallet = WalletState::new(policy, "workspace".to_string(), None);
        let reservation = wallet.reserve(policy, &body, &mut clock).unwrap();
        wallet
            .start(policy, reservation.physical, body.body)
            .unwrap();
        (wallet, reservation.physical)
    }

    #[test]
    fn a_missing_cost_is_not_known_and_an_explicit_zero_is() {
        let usage = ReportedUsage {
            prompt_tokens: Some(1),
            completion_tokens: Some(1),
            ..ReportedUsage::default()
        };
        assert_eq!(
            KnownBilling::from_reported(&usage),
            Err(UnknownReason::CostMissing)
        );
        for zero in ["0", "0.0", "0e5"] {
            let billing = known(zero, 1, 1);
            assert_eq!(billing.billed, Money::ZERO, "{zero}");
            assert_eq!(billing.billed_text, zero);
        }
    }

    #[test]
    fn missing_token_counts_are_not_known() {
        let amount = cost("0.1");
        let cases = [(None, Some(1)), (Some(1), None), (None, None)];
        for (prompt, completion) in cases {
            assert_eq!(
                KnownBilling::from_reported(&ReportedUsage {
                    cost: Some(&amount),
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    ..ReportedUsage::default()
                }),
                Err(UnknownReason::UsageMissing)
            );
        }
    }

    #[test]
    fn a_breakdown_larger_than_its_total_is_not_known_and_equal_is() {
        let amount = cost("0.1");
        let usage = |cached, reasoning| ReportedUsage {
            cost: Some(&amount),
            prompt_tokens: Some(10),
            completion_tokens: Some(20),
            cached_tokens: cached,
            reasoning_tokens: reasoning,
        };
        assert_eq!(
            KnownBilling::from_reported(&usage(Some(11), None)),
            Err(UnknownReason::UsageInconsistent)
        );
        assert_eq!(
            KnownBilling::from_reported(&usage(None, Some(21))),
            Err(UnknownReason::UsageInconsistent)
        );
        assert!(KnownBilling::from_reported(&usage(Some(10), Some(20))).is_ok());
    }

    #[test]
    fn token_counts_whose_sum_overflows_are_not_known() {
        let amount = cost("0.1");
        assert_eq!(
            KnownBilling::from_reported(&ReportedUsage {
                cost: Some(&amount),
                prompt_tokens: Some(u64::MAX),
                completion_tokens: Some(1),
                ..ReportedUsage::default()
            }),
            Err(UnknownReason::UsageInconsistent)
        );
    }

    #[test]
    fn the_computed_charge_prices_cached_tokens_inside_the_prompt_and_rounds_up() {
        let tariff = policy().tariff;
        // 100 prompt at 2, 40 completion at 10: 600 per million.
        assert_eq!(
            computed_charge(&tariff, 100, 40, None).unwrap(),
            money("0.0006")
        );
        // 30 of the 100 at 1 instead of 2.
        assert_eq!(
            computed_charge(&tariff, 100, 40, Some(30)).unwrap(),
            money("0.00057")
        );
        let tiny = policy_with(|v| {
            v["tariff"]["input_per_million"] = serde_json::json!("0.0000000000001");
            v["tariff"]["cache_write_per_million"] = serde_json::json!("0");
        })
        .tariff;
        // A millionth of 1e-13 is a tenth of the smallest unit, which rounds up to one.
        assert_eq!(computed_charge(&tiny, 1, 0, None).unwrap().units(), 1);
    }

    #[test]
    fn the_request_fee_is_in_the_computed_charge() {
        let tariff = policy_with(|v| v["tariff"]["request_fee"] = serde_json::json!("0.5")).tariff;
        assert_eq!(
            computed_charge(&tariff, 100, 40, None).unwrap(),
            money("0.5006")
        );
    }

    #[test]
    fn a_write_rate_above_the_input_rate_prices_the_uncached_prompt() {
        let tariff =
            policy_with(|v| v["tariff"]["cache_write_per_million"] = serde_json::json!("3")).tariff;
        assert_eq!(
            computed_charge(&tariff, 100, 0, Some(40)).unwrap(),
            // 60 at 3, 40 at 1.
            money("0.00022")
        );
    }

    #[test]
    fn reasoning_tokens_are_not_added_to_the_completion() {
        let tariff = policy().tariff;
        let amount = cost("0");
        let with_reasoning = KnownBilling::from_reported(&ReportedUsage {
            cost: Some(&amount),
            prompt_tokens: Some(100),
            completion_tokens: Some(40),
            cached_tokens: None,
            reasoning_tokens: Some(40),
        })
        .unwrap();
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        let outcome = wallet.settle(&policy, id, &with_reasoning).unwrap();
        assert_eq!(outcome.consumed, money("0.0006"));
        assert_eq!(
            outcome.consumed,
            computed_charge(&tariff, 100, 40, None).unwrap()
        );
    }

    #[test]
    fn consumption_is_the_larger_and_the_billed_field_is_left_alone() {
        let policy = policy();
        // Billed below the tariff.
        let (mut wallet, id) = started(&policy);
        wallet
            .settle(&policy, id, &known("0.0005", 100, 40))
            .unwrap();
        let record = wallet.attempt(id).unwrap().settlement.clone().unwrap();
        assert_eq!(record.billed_usd, "0.0005");
        assert_eq!(record.billed, money("0.0005"));
        assert_eq!(record.computed, money("0.0006"));
        assert_eq!(record.consumed, money("0.0006"));
        // Billed above it.
        let (mut wallet, id) = started(&policy);
        wallet
            .settle(&policy, id, &known("0.0007", 100, 40))
            .unwrap();
        let record = wallet.attempt(id).unwrap().settlement.clone().unwrap();
        assert_eq!(record.billed, money("0.0007"));
        assert_eq!(record.computed, money("0.0006"));
        assert_eq!(record.consumed, money("0.0007"));
    }

    #[test]
    fn an_explicit_zero_with_tokens_consumes_the_tariff_figure_and_stays_zero_billed() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        wallet.settle(&policy, id, &known("0", 100, 40)).unwrap();
        let record = wallet.attempt(id).unwrap().settlement.clone().unwrap();
        assert_eq!(record.billed_usd, "0");
        assert_eq!(record.billed, Money::ZERO);
        assert_eq!(record.consumed, money("0.0006"));
        assert_eq!(wallet.spent().unwrap(), money("0.0006"));
    }

    #[test]
    fn a_tiny_billed_amount_is_not_zero() {
        // Rates that round up to one unit per million, so the billed amount is what is measured:
        // the figure from the tariff is the same single unit it is.
        let policy = policy_with(|v| {
            v["tariff"]["input_per_million"] = serde_json::json!("1e-30");
            v["tariff"]["cached_input_per_million"] = serde_json::json!("0");
            v["tariff"]["cache_write_per_million"] = serde_json::json!("0");
            v["tariff"]["output_per_million"] = serde_json::json!("1e-30");
        });
        let (mut wallet, id) = started(&policy);
        let outcome = wallet.settle(&policy, id, &known("1e-400", 1, 1)).unwrap();
        let record = wallet.attempt(id).unwrap().settlement.clone().unwrap();
        assert_eq!(record.billed_usd, "1e-400");
        assert_eq!(record.billed.units(), 1);
        assert_ne!(record.billed, Money::ZERO);
        assert_eq!(outcome.consumed.units(), 1);
        assert_eq!(wallet.spent().unwrap().units(), 1);
    }

    /// Against the zero spellings of the same amount: those are zero, this is not.
    #[test]
    fn a_tiny_billed_amount_past_the_reservation_keeps_its_value_and_stops_the_wallet() {
        // The one unit of the reservation is used by the tariff's figure; two units billed is
        // past it.
        let policy = policy_with(|v| {
            v["tariff"]["input_per_million"] = serde_json::json!("1e-30");
            v["tariff"]["cached_input_per_million"] = serde_json::json!("0");
            v["tariff"]["cache_write_per_million"] = serde_json::json!("0");
            v["tariff"]["output_per_million"] = serde_json::json!("1e-30");
        });
        let (mut wallet, id) = started(&policy);
        let reserved = wallet.attempt(id).unwrap().reserved;
        assert_eq!(reserved.units(), 1);
        let outcome = wallet
            .settle(&policy, id, &known("0.000000000000000002", 1, 1))
            .unwrap();
        assert!(outcome.latched);
        assert_eq!(outcome.consumed.units(), 2);
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::OverReservation);
    }

    fn report<'a>(amount: &'a BilledCostUsd, prompt: u64, completion: u64) -> ReportedUsage<'a> {
        ReportedUsage {
            cost: Some(amount),
            prompt_tokens: Some(prompt),
            completion_tokens: Some(completion),
            ..ReportedUsage::default()
        }
    }

    #[test]
    fn no_prompt_tokens_for_a_body_is_not_known_whatever_it_says_it_cost() {
        let policy = policy();
        for (token, completion) in [("0", 0), ("0", 40), ("0.0001", 0), ("0.0001", 40)] {
            let amount = cost(token);
            let (mut wallet, id) = started(&policy);
            assert_eq!(
                wallet.settle_reported(&policy, id, &report(&amount, 0, completion)),
                Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent)),
                "{token} {completion}"
            );
            let attempt = wallet.attempt(id).unwrap();
            assert_eq!(attempt.state, AttemptState::Unknown);
            assert_eq!(
                attempt.unknown_reason,
                Some(UnknownReason::UsageInconsistent)
            );
            assert_eq!(wallet.latch().unwrap().reason, LatchReason::UnknownBilling);
            assert_eq!(wallet.held().unwrap(), money("0.00072"));
            assert_eq!(wallet.spent().unwrap(), Money::ZERO);
            assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Latched));
        }
    }

    #[test]
    fn one_prompt_token_for_a_body_is_known() {
        let policy = policy();
        let amount = cost("0");
        let (mut wallet, id) = started(&policy);
        let settled = wallet.settle_reported(&policy, id, &report(&amount, 1, 0));
        assert!(
            matches!(settled, Ok(SettleReport::Settled(_))),
            "{settled:?}"
        );
        assert!(wallet.latch().is_none());
    }

    #[test]
    fn no_prompt_tokens_is_refused_on_the_direct_way_too_and_leaves_the_send_unresolved() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        let before = wallet.clone();
        assert_eq!(
            wallet.settle(&policy, id, &known("0", 0, 0)).map(|_| ()),
            Err(BudgetRefusal::UsageInconsistent)
        );
        assert_eq!(wallet, before);
        assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Unresolved));
    }

    /// With no bytes there would be nothing a report of no prompt tokens could hide, and so no
    /// send of no bytes is reserved: every send that can be reported on carried a body.
    #[test]
    fn no_body_is_refused_so_no_prompt_tokens_is_never_what_a_send_should_report() {
        let policy = policy();
        let body = ReserveRequest {
            body: b"",
            ..request()
        };
        let mut clock = RunClock::new(FakeClock::at(1_000));
        let mut wallet = WalletState::new(&policy, "workspace".to_string(), None);
        assert_eq!(
            wallet.reserve(&policy, &body, &mut clock),
            Err(BudgetRefusal::BodyNotAdmitted)
        );
        assert!(wallet.attempts.is_empty());

        let (mut wallet, id) = started(&policy);
        let amount = cost("0");
        assert_eq!(
            wallet.settle_reported(&policy, id, &report(&amount, 0, 0)),
            Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent))
        );
    }

    /// What reaches a report at all: an exponent of 20 digits is too large in one direction, is
    /// next to nothing in the other, and a leading zero is no number.
    #[test]
    fn a_billed_cost_is_held_to_its_range_and_grammar_at_the_edges_a_report_can_reach() {
        assert!(BilledCostUsd::from_json_token("1e99999999999999999999").is_none());
        assert!(BilledCostUsd::from_json_token("1e-99999999999999999999").is_some());
        assert!(BilledCostUsd::from_json_token("01").is_none());
    }

    fn free_cached_input() -> RunBudgetPolicy {
        policy_with(|v| v["tariff"]["cached_input_per_million"] = serde_json::json!("0"))
    }

    /// `report` with this many of the prompt tokens cached.
    fn cached_report<'a>(
        amount: &'a BilledCostUsd,
        prompt: u64,
        completion: u64,
        cached: u64,
    ) -> ReportedUsage<'a> {
        ReportedUsage {
            cached_tokens: Some(cached),
            ..report(amount, prompt, completion)
        }
    }

    #[test]
    fn a_body_that_comes_to_nothing_is_not_known_when_it_is_all_cached_at_a_free_rate() {
        let policy = free_cached_input();
        let amount = cost("0");
        let (mut wallet, id) = started(&policy);
        assert_eq!(
            wallet.settle_reported(&policy, id, &cached_report(&amount, 100, 0, 100)),
            Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent))
        );
        let attempt = wallet.attempt(id).unwrap();
        assert_eq!(attempt.state, AttemptState::Unknown);
        assert_eq!(
            attempt.unknown_reason,
            Some(UnknownReason::UsageInconsistent)
        );
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::UnknownBilling);
        assert_eq!(wallet.held().unwrap(), money("0.00072"));
        assert_eq!(wallet.spent().unwrap(), Money::ZERO);
        assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Latched));
    }

    #[test]
    fn a_body_that_comes_to_nothing_is_refused_on_the_direct_way_too() {
        let policy = free_cached_input();
        let (mut wallet, id) = started(&policy);
        let before = wallet.clone();
        let amount = cost("0");
        let billing = KnownBilling::from_reported(&cached_report(&amount, 100, 0, 100)).unwrap();
        assert_eq!(
            wallet.settle(&policy, id, &billing).map(|_| ()),
            Err(BudgetRefusal::UsageInconsistent)
        );
        assert_eq!(wallet, before);
    }

    /// One thing that is not nothing, in each row, and the same free rate: each of them is known.
    #[test]
    fn a_free_cached_rate_settles_whatever_else_the_report_has() {
        let policy = free_cached_input();
        for (token, prompt, completion, cached, consumed) in [
            // An output token.
            ("0", 100, 1, 100, "0.00001"),
            // An uncached prompt token.
            ("0", 100, 0, 99, "0.000002"),
            // A billed amount.
            ("0.000001", 100, 0, 100, "0.000001"),
        ] {
            let amount = cost(token);
            let (mut wallet, id) = started(&policy);
            let settled = wallet
                .settle_reported(
                    &policy,
                    id,
                    &cached_report(&amount, prompt, completion, cached),
                )
                .unwrap();
            let SettleReport::Settled(outcome) = settled else {
                panic!("{token} {prompt} {completion} {cached}: {settled:?}");
            };
            assert_eq!(outcome.consumed, money(consumed), "{token}");
            assert!(wallet.latch().is_none(), "{token}");
            assert!(wallet.is_consistent(&policy), "{token}");
        }
    }

    #[test]
    fn a_report_of_an_amount_past_counting_leaves_the_send_unknown_and_stops_the_wallet() {
        let policy = policy_with(|v| {
            v["tariff"]["output_per_million"] = serde_json::json!("1000");
        });
        let amount = cost("0");
        let (mut wallet, id) = started(&policy);
        assert_eq!(
            wallet.settle_reported(&policy, id, &report(&amount, 1, u64::MAX - 1)),
            Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent))
        );
        assert_eq!(wallet.attempt(id).unwrap().state, AttemptState::Unknown);
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::UnknownBilling);
        // (110 * 2 + 50 * 1000) per million, at this policy's output rate.
        assert_eq!(wallet.held().unwrap(), money("0.05022"));
        assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Latched));
        assert!(wallet.is_consistent(&policy));

        // Late, it changes nothing, and a report that can be counted still settles it.
        let before = wallet.clone();
        assert_eq!(
            wallet.settle_reported(&policy, id, &report(&amount, 1, u64::MAX - 1)),
            Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent))
        );
        assert_eq!(wallet, before);
        assert!(matches!(
            wallet.settle_reported(&policy, id, &report(&amount, 1, 1)),
            Ok(SettleReport::Settled(_))
        ));
    }

    #[test]
    fn a_report_past_what_the_wallet_can_count_leaves_the_send_unknown() {
        let policy = policy();
        let (mut wallet, first) = started(&policy);
        wallet.settle(&policy, first, &known("0", 1, 1)).unwrap();
        let body = request();
        let mut clock = RunClock::new(FakeClock::at(1_000));
        let second = wallet.reserve(&policy, &body, &mut clock).unwrap().physical;
        wallet.start(&policy, second, body.body).unwrap();
        wallet
            .attempt_mut(first)
            .unwrap()
            .settlement
            .as_mut()
            .unwrap()
            .consumed = Money::from_units(u128::MAX);
        let amount = cost("0");
        assert_eq!(
            wallet.settle_reported(&policy, second, &report(&amount, 1, 1)),
            Ok(SettleReport::Unknown(UnknownReason::UsageInconsistent))
        );
        assert_eq!(wallet.attempt(second).unwrap().state, AttemptState::Unknown);
    }

    #[test]
    fn a_report_with_a_missing_part_is_unknown_and_stops_the_wallet() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        assert_eq!(
            wallet.settle_reported(&policy, id, &ReportedUsage::default()),
            Ok(SettleReport::Unknown(UnknownReason::CostMissing))
        );
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::UnknownBilling);
        assert_eq!(wallet.held().unwrap(), money("0.00072"));
    }

    #[test]
    fn a_late_report_that_is_not_known_changes_nothing_and_a_known_one_settles() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        wallet.mark_unknown(id, UnknownReason::Timeout).unwrap();
        let before = wallet.clone();
        assert_eq!(
            wallet.settle_reported(&policy, id, &ReportedUsage::default()),
            Ok(SettleReport::Unknown(UnknownReason::CostMissing))
        );
        assert_eq!(wallet, before);
        let amount = cost("0.0005");
        assert!(matches!(
            wallet.settle_reported(&policy, id, &report(&amount, 100, 40)),
            Ok(SettleReport::Settled(_))
        ));
        assert_eq!(wallet.spent().unwrap(), money("0.0006"));
        let again = wallet.settle_reported(&policy, id, &report(&amount, 100, 40));
        assert_eq!(
            again,
            Err(BudgetRefusal::IllegalTransition {
                from: AttemptState::Settled,
                to: AttemptState::Settled,
            })
        );
    }

    #[test]
    fn a_report_for_a_send_that_was_not_started_is_refused() {
        let policy = policy();
        let body = request();
        let mut clock = RunClock::new(FakeClock::at(1_000));
        let mut wallet = WalletState::new(&policy, "workspace".to_string(), None);
        let id = wallet.reserve(&policy, &body, &mut clock).unwrap().physical;
        let before = wallet.clone();
        assert_eq!(
            wallet.settle_reported(&policy, id, &ReportedUsage::default()),
            Err(BudgetRefusal::IllegalTransition {
                from: AttemptState::Reserved,
                to: AttemptState::Settled,
            })
        );
        assert_eq!(wallet, before);
        assert_eq!(
            wallet.settle_reported(&policy, PhysicalId(99), &ReportedUsage::default()),
            Err(BudgetRefusal::NoSuchSend)
        );
    }

    #[test]
    fn a_response_past_the_deadline_is_settled_at_its_cost_and_is_not_a_success() {
        let wall = policy_with(|v| v["limits"]["wall_seconds"] = serde_json::json!(1));
        // Exactly at the deadline is in time.
        let (mut wallet, id) = started(&wall);
        wallet.clock.high_water_elapsed_ms = 1_000;
        let outcome = wallet.settle(&wall, id, &known("0", 100, 40)).unwrap();
        assert!(!outcome.latched);
        assert_eq!(wallet.success_gate(), Ok(()));

        let (mut wallet, id) = started(&wall);
        wallet.clock.high_water_elapsed_ms = 1_001;
        let outcome = wallet.settle(&wall, id, &known("0", 100, 40)).unwrap();
        assert!(outcome.latched);
        assert_eq!(outcome.consumed, money("0.0006"));
        assert_eq!(wallet.spent().unwrap(), money("0.0006"));
        assert_eq!(wallet.held().unwrap(), Money::ZERO);
        assert_eq!(
            wallet.latch().unwrap().reason,
            LatchReason::ResponseAfterDeadline
        );
        assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Latched));
        assert!(wallet.is_consistent(&wall));
    }

    #[test]
    fn a_known_settlement_replaces_the_hold_with_the_amount_and_is_not_a_release() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        assert_eq!(wallet.held().unwrap(), money("0.00072"));
        wallet
            .settle(&policy, id, &known("0.0005", 100, 40))
            .unwrap();
        assert_eq!(wallet.held().unwrap(), Money::ZERO);
        assert_eq!(wallet.spent().unwrap(), money("0.0006"));
        assert_eq!(wallet.attempt(id).unwrap().state, AttemptState::Settled);
        assert_eq!(wallet.physical_used(), 1);
        assert!(wallet.latch().is_none());
        assert_eq!(wallet.spent_tokens(), 140);
        assert_eq!(wallet.held_tokens(), 0);
    }

    #[test]
    fn an_amount_exactly_at_the_reservation_does_not_latch_and_one_unit_over_does() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        let outcome = wallet
            .settle(&policy, id, &known("0.00072", 100, 40))
            .unwrap();
        assert!(!outcome.latched);
        assert!(wallet.latch().is_none());

        let (mut wallet, id) = started(&policy);
        let outcome = wallet
            .settle(&policy, id, &known("0.000720000000000001", 100, 40))
            .unwrap();
        assert!(outcome.latched);
        assert_eq!(outcome.consumed, money("0.000720000000000001"));
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::OverReservation);
        assert_eq!(wallet.spent().unwrap(), money("0.000720000000000001"));
        assert_eq!(wallet.held().unwrap(), Money::ZERO);
    }

    #[test]
    fn prompt_tokens_past_the_bound_keep_their_value_and_latch() {
        let policy = policy();
        // The bound is the 100 bytes and the 10 of overhead.
        let (mut wallet, id) = started(&policy);
        let outcome = wallet.settle(&policy, id, &known("0", 110, 0)).unwrap();
        assert!(!outcome.latched);

        let (mut wallet, id) = started(&policy);
        wallet.settle(&policy, id, &known("0", 111, 0)).unwrap();
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::InputOverBound);
        let record = wallet.attempt(id).unwrap().settlement.clone().unwrap();
        assert_eq!(record.prompt_tokens, 111);
    }

    #[test]
    fn completion_tokens_past_the_bound_keep_their_value_and_latch() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        let outcome = wallet.settle(&policy, id, &known("0", 1, 50)).unwrap();
        assert!(!outcome.latched);

        let (mut wallet, id) = started(&policy);
        wallet.settle(&policy, id, &known("0", 1, 51)).unwrap();
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::OutputOverBound);
        let record = wallet.attempt(id).unwrap().settlement.clone().unwrap();
        assert_eq!(record.completion_tokens, 51);
    }

    #[test]
    fn a_second_settlement_is_refused_and_overwrites_nothing() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        wallet
            .settle(&policy, id, &known("0.0005", 100, 40))
            .unwrap();
        let before = wallet.clone();
        assert_eq!(
            wallet.settle(&policy, id, &known("0.0007", 1, 1)),
            Err(BudgetRefusal::IllegalTransition {
                from: AttemptState::Settled,
                to: AttemptState::Settled,
            })
        );
        assert_eq!(wallet, before);
    }

    #[test]
    fn a_late_response_settles_the_same_send_once_and_the_latch_stays() {
        let policy = policy();
        let (mut wallet, id) = started(&policy);
        wallet.mark_unknown(id, UnknownReason::Timeout).unwrap();
        assert_eq!(wallet.held().unwrap(), money("0.00072"));
        assert_eq!(wallet.spent().unwrap(), Money::ZERO);

        let outcome = wallet
            .settle(&policy, id, &known("0.0005", 100, 40))
            .unwrap();
        assert!(outcome.latched);
        assert_eq!(wallet.held().unwrap(), Money::ZERO);
        assert_eq!(wallet.spent().unwrap(), money("0.0006"));
        let attempt = wallet.attempt(id).unwrap();
        assert!(attempt.settlement.as_ref().unwrap().late);
        assert_eq!(attempt.unknown_reason, Some(UnknownReason::Timeout));
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::UnknownBilling);
        assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Latched));

        let before = wallet.clone();
        assert!(
            wallet
                .settle(&policy, id, &known("0.0005", 100, 40))
                .is_err()
        );
        assert_eq!(wallet, before);
    }

    #[test]
    fn a_settlement_that_overflows_leaves_the_send_unresolved() {
        let policy = policy_with(|v| {
            v["tariff"]["output_per_million"] = serde_json::json!("1000");
        });
        let (mut wallet, id) = started(&policy);
        let before = wallet.clone();
        assert_eq!(
            wallet.settle(&policy, id, &known("0", 1, u64::MAX - 1)),
            Err(BudgetRefusal::Overflow)
        );
        assert_eq!(wallet, before);
        assert_eq!(wallet.attempt(id).unwrap().state, AttemptState::Started);
        assert_eq!(wallet.success_gate(), Err(BudgetRefusal::Unresolved));
    }

    #[test]
    fn a_settlement_past_what_the_wallet_can_count_leaves_the_send_unresolved() {
        let policy = policy();
        let (mut wallet, first) = started(&policy);
        wallet.settle(&policy, first, &known("0", 1, 1)).unwrap();
        let body = request();
        let mut clock = RunClock::new(FakeClock::at(1_000));
        let second = wallet.reserve(&policy, &body, &mut clock).unwrap().physical;
        wallet.start(&policy, second, body.body).unwrap();
        wallet
            .attempt_mut(first)
            .unwrap()
            .settlement
            .as_mut()
            .unwrap()
            .consumed = Money::from_units(u128::MAX);
        let before = wallet.clone();
        assert_eq!(
            wallet.settle(&policy, second, &known("0", 1, 1)),
            Err(BudgetRefusal::Overflow)
        );
        assert_eq!(wallet, before);
    }
}
