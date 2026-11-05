//! What a provider reported, and the only way a send gets settled.
//!
//! Settling takes a [`KnownBilling`], and the only way to one is a reported cost with both token
//! counts. An explicit zero is a reported cost; an absent one has no value of this type, so there
//! is no path that settles a send at zero because something was missing.
//!
//! What is consumed is the larger of the billed amount and the tariff's figure. The fields stay
//! apart, and the billed one is never rewritten.

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

impl WalletState {
    /// `started` or `unknown` to `settled`, once. A late settlement leaves the latch set, and an
    /// amount past the reservation keeps its value and sets it. Replacing a reservation with the
    /// known amount is not a release.
    pub(crate) fn settle(
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
        let (reserved, input_bound, output_bound) =
            (attempt.reserved, attempt.input_bound, attempt.output_bound);
        let computed = computed_charge(
            &policy.tariff,
            billing.prompt_tokens,
            billing.completion_tokens,
            billing.cached_tokens,
        )?;
        let consumed = billing.billed.max(computed);
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
    use super::super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money, WalletState};
    use super::super::test_support::{FakeClock, general, policy, policy_with, request, route};
    use super::{KnownBilling, ReportedUsage, UnknownReason, computed_charge};
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
        let wire = route();
        let body = request(&wire, general());
        let clock = FakeClock::at(1_000);
        let mut clock = RunClock::new(clock);
        let mut wallet = WalletState::new(policy, "workspace".to_string(), None);
        let reservation = wallet.reserve(policy, &body, &mut clock).unwrap();
        wallet.start(reservation.physical, body.body).unwrap();
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
            v["tariff"]["output_per_million"] = serde_json::json!("0");
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
        let policy = policy_with(|v| {
            v["tariff"]["input_per_million"] = serde_json::json!("0");
            v["tariff"]["cached_input_per_million"] = serde_json::json!("0");
            v["tariff"]["cache_write_per_million"] = serde_json::json!("0");
            v["tariff"]["output_per_million"] = serde_json::json!("0");
        });
        let (mut wallet, id) = started(&policy);
        let outcome = wallet.settle(&policy, id, &known("1e-400", 1, 1)).unwrap();
        assert_eq!(outcome.consumed.units(), 1);
        // No reservation covered it, so the value is kept and the wallet stops.
        assert!(outcome.latched);
        assert_eq!(wallet.spent().unwrap().units(), 1);
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
        let outcome = wallet.settle(&policy, id, &known("0", 0, 50)).unwrap();
        assert!(!outcome.latched);

        let (mut wallet, id) = started(&policy);
        wallet.settle(&policy, id, &known("0", 0, 51)).unwrap();
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
            v["tariff"]["input_per_million"] = serde_json::json!("0");
            v["tariff"]["cached_input_per_million"] = serde_json::json!("0");
            v["tariff"]["cache_write_per_million"] = serde_json::json!("0");
        });
        let (mut wallet, id) = started(&policy);
        let before = wallet.clone();
        assert_eq!(
            wallet.settle(&policy, id, &known("0", 0, u64::MAX)),
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
        let wire = route();
        let body = request(&wire, general());
        let mut clock = RunClock::new(FakeClock::at(1_000));
        let second = wallet.reserve(&policy, &body, &mut clock).unwrap().physical;
        wallet.start(second, body.body).unwrap();
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
