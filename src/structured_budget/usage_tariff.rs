//! What a validated usage costs under a `usage_tariff` policy, and the most a request can cost
//! (Issue #1006, design 3). Nothing here touches a disk, a float, or a model name.
//!
//! With `I` input tokens (cached and written ones included), `C` cached, `W30`/`W12` written for
//! each TTL, and `O` output tokens (reasoning included):
//!
//! - the tier is the one `I` falls in, and its rates price the whole request: input of every class
//!   and output alike. Cached and written tokens count toward `I` for the choice.
//! - ordinary input is `I - C - W30 - W12`; a write is never priced on top of the input as well.
//! - the per-million terms are added first and rounded up once, as the v1 rule does
//!   (`settlement::computed_charge`), and the tier's request fee is added after.
//! - reasoning is inside `O`, so it has no term of its own.
//! - `cache_write_mode: none` has no write term; a reported write there must be zero.
//!
//! A sent body cannot have read no input or cost nothing: either is unknown, never a free send.
//! Every sum is checked, and an overflow is a stop, never a wrap.

use serde::{Deserialize, Serialize};

use super::policy_v2::{CacheWriteMode, CacheWriteRates, TariffV2, TierRates};
use super::reservation::{Money, per_million_up};
use crate::providers::usage_observation::ResponsesUsage;

/// Why a validated usage has no charge under the policy. Fixed words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChargeStop {
    /// `ttl_30m_12h` needs the write total and both TTL counts, zeros included.
    CacheWriteFieldMissing,
    /// `none` prices no write, so a reported write has to be zero.
    CacheWriteInNoneMode,
    /// The counts do not fit together (a usage that did not come through the reader).
    UsageInconsistent,
    /// A sent body read no input: the report hides it.
    InputZero,
    /// The usage comes to nothing: the report hides the body.
    ZeroCharge,
    Overflow,
    /// A tier whose write prices do not match the mode (a tariff that did not come through the
    /// decode).
    TariffInconsistent,
}

impl ChargeStop {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::CacheWriteFieldMissing => "cache_write_field_missing",
            Self::CacheWriteInNoneMode => "cache_write_in_none_mode",
            Self::UsageInconsistent => "usage_inconsistent",
            Self::InputZero => "input_zero",
            Self::ZeroCharge => "zero_charge",
            Self::Overflow => "overflow",
            Self::TariffInconsistent => "tariff_inconsistent",
        }
    }
}

/// What a usage was charged, and how: the parts are kept apart for the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UsageCharge {
    /// Index of the tier `I` fell in.
    pub(crate) tier: usize,
    pub(crate) ordinary_input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) cache_write_30m_tokens: u64,
    pub(crate) cache_write_12h_tokens: u64,
    pub(crate) output_tokens: u64,
    /// `I + O`, what the token limit counts.
    pub(crate) tokens: u64,
    /// From the fixed tariff, never a billed amount.
    pub(crate) charge: Money,
}

/// The tier an input of `input_tokens` falls in: the last one it is above, or the first.
pub(crate) fn tier_index(tariff: &TariffV2, input_tokens: u64) -> usize {
    tariff
        .tiers
        .iter()
        .rposition(|tier| tier.above_input_tokens < input_tokens)
        .unwrap_or(0)
}

fn term(tokens: u64, rate: Money) -> Result<u128, ChargeStop> {
    u128::from(tokens)
        .checked_mul(rate.units())
        .ok_or(ChargeStop::Overflow)
}

fn sum(terms: &[u128]) -> Result<u128, ChargeStop> {
    terms
        .iter()
        .try_fold(0_u128, |total, term| total.checked_add(*term))
        .ok_or(ChargeStop::Overflow)
}

const NO_WRITE: CacheWriteRates = CacheWriteRates {
    ttl_30m: Money::ZERO,
    ttl_12h: Money::ZERO,
};

fn write_rates(mode: CacheWriteMode, tier: &TierRates) -> Result<CacheWriteRates, ChargeStop> {
    match (mode, tier.cache_write) {
        (CacheWriteMode::Ttl30m12h, Some(rates)) => Ok(rates),
        (CacheWriteMode::None, None) => Ok(NO_WRITE),
        _ => Err(ChargeStop::TariffInconsistent),
    }
}

/// The written tokens for each TTL the mode prices.
fn written(mode: CacheWriteMode, usage: &ResponsesUsage) -> Result<(u64, u64), ChargeStop> {
    let reported = (
        usage.cache_write_tokens,
        usage.cache_write_30m_tokens,
        usage.cache_write_12h_tokens,
    );
    match mode {
        CacheWriteMode::Ttl30m12h => {
            let (Some(total), Some(ttl_30m), Some(ttl_12h)) = reported else {
                return Err(ChargeStop::CacheWriteFieldMissing);
            };
            if ttl_30m.checked_add(ttl_12h) != Some(total) {
                return Err(ChargeStop::UsageInconsistent);
            }
            Ok((ttl_30m, ttl_12h))
        }
        CacheWriteMode::None => {
            let (total, ttl_30m, ttl_12h) = reported;
            if [total, ttl_30m, ttl_12h]
                .into_iter()
                .flatten()
                .any(|count| count != 0)
            {
                return Err(ChargeStop::CacheWriteInNoneMode);
            }
            Ok((0, 0))
        }
    }
}

/// The charge of one response's validated usage, for a send that carried a body.
pub(crate) fn charge(tariff: &TariffV2, usage: &ResponsesUsage) -> Result<UsageCharge, ChargeStop> {
    let (write_30m, write_12h) = written(tariff.cache_write_mode, usage)?;
    if usage.input_tokens == 0 {
        return Err(ChargeStop::InputZero);
    }
    if usage
        .reasoning_tokens
        .is_some_and(|reasoning| reasoning > usage.output_tokens)
    {
        return Err(ChargeStop::UsageInconsistent);
    }
    let ordinary = usage
        .input_tokens
        .checked_sub(usage.cached_tokens)
        .and_then(|rest| rest.checked_sub(write_30m))
        .and_then(|rest| rest.checked_sub(write_12h))
        .ok_or(ChargeStop::UsageInconsistent)?;
    let index = tier_index(tariff, usage.input_tokens);
    let tier = &tariff.tiers[index];
    let writes = write_rates(tariff.cache_write_mode, tier)?;
    let numerator = sum(&[
        term(ordinary, tier.input)?,
        term(usage.cached_tokens, tier.cached_input)?,
        term(write_30m, writes.ttl_30m)?,
        term(write_12h, writes.ttl_12h)?,
        term(usage.output_tokens, tier.output)?,
    ])?;
    let charge = per_million_up(numerator)
        .checked_add(tier.request_fee)
        .ok_or(ChargeStop::Overflow)?;
    if charge == Money::ZERO {
        return Err(ChargeStop::ZeroCharge);
    }
    let tokens = usage
        .input_tokens
        .checked_add(usage.output_tokens)
        .ok_or(ChargeStop::Overflow)?;
    Ok(UsageCharge {
        tier: index,
        ordinary_input_tokens: ordinary,
        cached_input_tokens: usage.cached_tokens,
        cache_write_30m_tokens: write_30m,
        cache_write_12h_tokens: write_12h,
        output_tokens: usage.output_tokens,
        tokens,
        charge,
    })
}

/// The most a request bounded to `input_bound` input tokens and `max_output` output tokens can
/// cost: every tier an input up to the bound can reach, every input class at its highest rate in
/// those tiers, the highest output rate and the highest fee among them. A tier the bound only just
/// reaches is counted, so a response cannot climb into a tier the reservation left out.
pub(crate) fn reservation_bound(
    tariff: &TariffV2,
    input_bound: u64,
    max_output: u64,
) -> Result<Money, ChargeStop> {
    let reachable = &tariff.tiers[..=tier_index(tariff, input_bound)];
    let mut input_rate = Money::ZERO;
    let mut output_rate = Money::ZERO;
    let mut fee = Money::ZERO;
    for tier in reachable {
        let writes = write_rates(tariff.cache_write_mode, tier)?;
        input_rate = input_rate
            .max(tier.input)
            .max(tier.cached_input)
            .max(writes.ttl_30m)
            .max(writes.ttl_12h);
        output_rate = output_rate.max(tier.output);
        fee = fee.max(tier.request_fee);
    }
    let numerator = sum(&[
        term(input_bound, input_rate)?,
        term(max_output, output_rate)?,
    ])?;
    per_million_up(numerator)
        .checked_add(fee)
        .ok_or(ChargeStop::Overflow)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::policy_v2::TariffV2;
    use super::super::policy_v2::test_support::{decode_v2, policy_v2_none_value, policy_v2_value};
    use super::super::test_support::Rng;
    use super::{ChargeStop, charge, reservation_bound, tier_index};
    use crate::providers::usage_observation::{ResponsesUsage, read_responses_usage};

    const UNIT: u128 = 1_000_000_000_000_000_000;

    // ---- The oracle: written apart from the code above, from the decimal text of the prices. ----

    /// A plain decimal ("12", "0.25") in units of 10^-18, by long hand.
    fn units(text: &str) -> u128 {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        assert!(fraction.len() <= 18, "{text}");
        let padded = format!("{fraction:0<18}");
        whole.parse::<u128>().unwrap() * UNIT + padded.parse::<u128>().unwrap()
    }

    /// The prices of [`policy_v2_value`], read again from its text.
    struct OracleTier {
        input: u128,
        cached: u128,
        w30: u128,
        w12: u128,
        output: u128,
        fee: u128,
    }

    fn oracle_tier(value: &Value, index: usize) -> OracleTier {
        let tier = &value["tariff"]["tiers"][index];
        let price = |key: &str| tier.get(key).map_or(0, |v| units(v.as_str().unwrap()));
        OracleTier {
            input: price("input_per_million"),
            cached: price("cached_input_per_million"),
            w30: price("cache_write_30m_per_million"),
            w12: price("cache_write_12h_per_million"),
            output: price("output_per_million"),
            fee: price("request_fee"),
        }
    }

    /// One request's cost: the 272000 boundary on all of `input`, every term summed, then rounded
    /// up once, then the fee.
    fn oracle(value: &Value, input: u64, cached: u64, w30: u64, w12: u64, output: u64) -> u128 {
        let tier = oracle_tier(value, if input <= 272_000 { 0 } else { 1 });
        let ordinary = u128::from(input) - u128::from(cached) - u128::from(w30) - u128::from(w12);
        let sum = ordinary * tier.input
            + u128::from(cached) * tier.cached
            + u128::from(w30) * tier.w30
            + u128::from(w12) * tier.w12
            + u128::from(output) * tier.output;
        sum.div_ceil(1_000_000) + tier.fee
    }

    fn tariff_of(value: &Value) -> TariffV2 {
        decode_v2(value).tariff
    }

    fn usage(input: u64, cached: u64, w30: u64, w12: u64, output: u64) -> ResponsesUsage {
        ResponsesUsage {
            input_tokens: input,
            output_tokens: output,
            cached_tokens: cached,
            reasoning_tokens: None,
            total_tokens: Some(input + output),
            cache_write_tokens: Some(w30 + w12),
            cache_write_30m_tokens: Some(w30),
            cache_write_12h_tokens: Some(w12),
        }
    }

    fn charged(value: &Value, usage: &ResponsesUsage) -> u128 {
        charge(&tariff_of(value), usage).unwrap().charge.units()
    }

    #[test]
    fn a_mixed_usage_costs_what_it_was_worked_out_to_by_hand() {
        // 500 ordinary at 2, 300 cached at 0.2, 150 at 2.5, 50 at 4, 80 out at 8, per million:
        // 1000 + 60 + 375 + 200 + 640 = 2275 micro-dollars.
        let value = policy_v2_value();
        let got = charge(&tariff_of(&value), &usage(1000, 300, 150, 50, 80)).unwrap();
        assert_eq!(got.charge.units(), 2275 * UNIT / 1_000_000);
        assert_eq!(got.tier, 0);
        assert_eq!(got.ordinary_input_tokens, 500);
        assert_eq!(got.cached_input_tokens, 300);
        assert_eq!(got.cache_write_30m_tokens, 150);
        assert_eq!(got.cache_write_12h_tokens, 50);
        assert_eq!(got.output_tokens, 80);
        assert_eq!(got.tokens, 1080);
    }

    #[test]
    fn every_cache_class_matches_the_oracle() {
        let value = policy_v2_value();
        for (name, (input, cached, w30, w12, output)) in [
            ("no cache", (1000, 0, 0, 0, 100)),
            ("cache hit", (1000, 600, 0, 0, 100)),
            ("all cached", (1000, 1000, 0, 0, 100)),
            ("30m write", (1000, 0, 400, 0, 100)),
            ("12h write", (1000, 0, 0, 400, 100)),
            ("mixed", (1000, 300, 150, 50, 100)),
            ("all written", (1000, 0, 600, 400, 1)),
            ("no output", (1000, 300, 150, 50, 0)),
        ] {
            assert_eq!(
                charged(&value, &usage(input, cached, w30, w12, output)),
                oracle(&value, input, cached, w30, w12, output),
                "{name}"
            );
        }
    }

    #[test]
    fn reasoning_is_inside_the_output_and_adds_nothing() {
        let value = policy_v2_value();
        let none = usage(1000, 300, 150, 50, 80);
        let mut some = none;
        some.reasoning_tokens = Some(60);
        let mut all = none;
        all.reasoning_tokens = Some(80);
        let mut zero = none;
        zero.reasoning_tokens = Some(0);
        let expected = oracle(&value, 1000, 300, 150, 50, 80);
        for (name, case) in [("none", none), ("some", some), ("all", all), ("zero", zero)] {
            assert_eq!(charged(&value, &case), expected, "{name}");
            assert_eq!(
                charge(&tariff_of(&value), &case).unwrap().tokens,
                1080,
                "{name}"
            );
        }
    }

    #[test]
    fn the_tier_changes_between_272000_and_272001_for_the_whole_request() {
        let value = policy_v2_value();
        let at = charge(&tariff_of(&value), &usage(272_000, 1000, 100, 100, 500)).unwrap();
        let past = charge(&tariff_of(&value), &usage(272_001, 1000, 100, 100, 500)).unwrap();
        assert_eq!(at.tier, 0);
        assert_eq!(past.tier, 1);
        assert_eq!(
            at.charge.units(),
            oracle(&value, 272_000, 1000, 100, 100, 500)
        );
        assert_eq!(
            past.charge.units(),
            oracle(&value, 272_001, 1000, 100, 100, 500)
        );
        // By hand: every class and the output take the high rates, not only the token past it.
        // 270801 * 4 + 1000 * 0.4 + 100 * 5 + 100 * 8 + 500 * 12 = 1090904 micro-dollars.
        assert_eq!(past.charge.units(), 1_090_904 * UNIT / 1_000_000);
        // Low tier by hand: 270800 * 2 + 1000 * 0.2 + 100 * 2.5 + 100 * 4 + 500 * 8 = 546450.
        assert_eq!(at.charge.units(), 546_450 * UNIT / 1_000_000);
    }

    #[test]
    fn cached_and_written_input_count_toward_the_tier() {
        // Without its cached and written tokens the input would be far below the boundary; the
        // whole input is past it, so the high tier prices it.
        let value = policy_v2_value();
        for (cached, w30, w12) in [
            (200_000, 0, 0),
            (0, 200_000, 0),
            (0, 0, 200_000),
            (100_000, 50_000, 50_000),
        ] {
            let case = usage(272_001, cached, w30, w12, 10);
            let got = charge(&tariff_of(&value), &case).unwrap();
            assert_eq!(got.ordinary_input_tokens, 72_001);
            assert_eq!(got.tier, 1, "{cached} {w30} {w12}");
            assert_eq!(
                got.charge.units(),
                oracle(&value, 272_001, cached, w30, w12, 10)
            );
        }
    }

    #[test]
    fn the_request_fee_is_added_once_after_the_rounding() {
        let mut value = policy_v2_value();
        value["tariff"]["tiers"][0]["request_fee"] = json!("0.01");
        value["tariff"]["tiers"][1]["request_fee"] = json!("0.02");
        assert_eq!(
            charged(&value, &usage(1000, 300, 150, 50, 80)),
            2275 * UNIT / 1_000_000 + UNIT / 100
        );
        assert_eq!(
            charged(&value, &usage(300_000, 0, 0, 0, 1)),
            oracle(&value, 300_000, 0, 0, 0, 1)
        );
        assert_eq!(oracle_tier(&value, 1).fee, UNIT / 50);
    }

    #[test]
    fn the_sum_is_rounded_up_once_not_each_term() {
        // One unit per million for input and output: half a million of each is one unit in all,
        // where rounding each term would make two.
        let mut value = policy_v2_value();
        let tier = &mut value["tariff"]["tiers"][1];
        tier["input_per_million"] = json!("0.000000000000000001");
        tier["output_per_million"] = json!("0.000000000000000001");
        tier["cached_input_per_million"] = json!("0");
        assert_eq!(charged(&value, &usage(500_000, 0, 0, 0, 500_000)), 1);
        assert_eq!(oracle(&value, 500_000, 0, 0, 0, 500_000), 1);
        // One more token of either rounds up to two units.
        assert_eq!(charged(&value, &usage(500_001, 0, 0, 0, 500_000)), 2);
        // A fraction of a unit is a whole unit.
        assert_eq!(charged(&value, &usage(300_000, 0, 0, 0, 1)), 1);
    }

    #[test]
    fn decimal_fractions_of_a_price_are_exact() {
        let mut value = policy_v2_value();
        let tier = &mut value["tariff"]["tiers"][0];
        tier["input_per_million"] = json!("1.234567891234567891");
        tier["cached_input_per_million"] = json!("0.123456789");
        tier["cache_write_30m_per_million"] = json!("1.5");
        tier["cache_write_12h_per_million"] = json!("2.000000000000000001");
        tier["output_per_million"] = json!("9.87654321");
        tier["request_fee"] = json!("0.000001");
        for case in [
            (1, 0, 0, 0, 1),
            (7, 3, 1, 1, 13),
            (999_983, 0, 0, 0, 0),
            (271_999, 100_003, 7, 11, 65_537),
        ] {
            let (input, cached, w30, w12, output) = case;
            assert_eq!(
                charged(&value, &usage(input, cached, w30, w12, output)),
                oracle(&value, input, cached, w30, w12, output),
                "{case:?}"
            );
        }
    }

    #[test]
    fn a_charge_past_counting_is_an_overflow() {
        let mut value = policy_v2_value();
        value["tariff"]["tiers"][1]["input_per_million"] = json!("1e15");
        value["tariff"]["tiers"][1]["output_per_million"] = json!("1e20");
        let tariff = tariff_of(&value);
        // One term past u128: 400000 * 10^33 units.
        assert_eq!(
            charge(&tariff, &usage(400_000, 0, 0, 0, 0)),
            Err(ChargeStop::Overflow)
        );
        // Two terms that each fit, 272001 * 10^33 and 10^38 units, and do not fit together.
        assert!(charge(&tariff, &usage(272_001, 0, 0, 0, 0)).is_ok());
        assert_eq!(
            charge(&tariff, &usage(272_001, 0, 0, 0, 1)),
            Err(ChargeStop::Overflow)
        );
        // The fee on top of the rounded sum: the fee alone is the largest amount there is.
        let mut value = policy_v2_value();
        value["tariff"]["tiers"][0]["request_fee"] =
            json!("340282366920938463463.374607431768211455");
        let tariff = tariff_of(&value);
        assert_eq!(tariff.tiers[0].request_fee.units(), u128::MAX);
        assert_eq!(
            charge(&tariff, &usage(1, 0, 0, 0, 1)),
            Err(ChargeStop::Overflow)
        );
        assert_eq!(reservation_bound(&tariff, 1, 1), Err(ChargeStop::Overflow));
        // A token count past u64 for the total.
        let mut case = usage(1, 0, 0, 0, 0);
        case.output_tokens = u64::MAX;
        case.total_tokens = None;
        assert_eq!(
            charge(&tariff_of(&policy_v2_value()), &case),
            Err(ChargeStop::Overflow)
        );
    }

    #[test]
    fn ttl_mode_needs_all_three_write_counts_even_at_zero() {
        let tariff = tariff_of(&policy_v2_value());
        let full = usage(1000, 0, 0, 0, 10);
        assert!(charge(&tariff, &full).is_ok());
        for missing in 0..3 {
            let mut case = full;
            match missing {
                0 => case.cache_write_tokens = None,
                1 => case.cache_write_30m_tokens = None,
                _ => case.cache_write_12h_tokens = None,
            }
            assert_eq!(
                charge(&tariff, &case),
                Err(ChargeStop::CacheWriteFieldMissing),
                "{missing}"
            );
        }
        // A total only, the way a provider with no split reports it, is not taken at a bound.
        let mut total_only = usage(1000, 0, 100, 0, 10);
        total_only.cache_write_30m_tokens = None;
        total_only.cache_write_12h_tokens = None;
        assert_eq!(
            charge(&tariff, &total_only),
            Err(ChargeStop::CacheWriteFieldMissing)
        );
        // A split that does not add up, made outside the reader.
        let mut wrong = usage(1000, 0, 100, 0, 10);
        wrong.cache_write_tokens = Some(99);
        assert_eq!(charge(&tariff, &wrong), Err(ChargeStop::UsageInconsistent));
    }

    #[test]
    fn none_mode_allows_no_write_fields_and_only_zero_writes() {
        let value = policy_v2_none_value();
        let tariff = tariff_of(&value);
        let mut absent = usage(1000, 300, 0, 0, 80);
        absent.cache_write_tokens = None;
        absent.cache_write_30m_tokens = None;
        absent.cache_write_12h_tokens = None;
        // No write term: 700 * 2 + 300 * 0.2 + 80 * 8 = 2100 micro-dollars.
        assert_eq!(
            charge(&tariff, &absent).unwrap().charge.units(),
            2100 * UNIT / 1_000_000
        );
        assert_eq!(
            charge(&tariff, &absent).unwrap().charge.units(),
            oracle(&value, 1000, 300, 0, 0, 80)
        );
        let zeros = usage(1000, 300, 0, 0, 80);
        assert_eq!(charge(&tariff, &zeros), charge(&tariff, &absent));
        for field in 0..3 {
            let mut case = absent;
            match field {
                0 => case.cache_write_tokens = Some(1),
                1 => case.cache_write_30m_tokens = Some(1),
                _ => case.cache_write_12h_tokens = Some(1),
            }
            assert_eq!(
                charge(&tariff, &case),
                Err(ChargeStop::CacheWriteInNoneMode),
                "{field}"
            );
        }
        assert_eq!(
            charge(&tariff, &usage(1000, 0, 100, 50, 80)),
            Err(ChargeStop::CacheWriteInNoneMode)
        );
    }

    #[test]
    fn no_input_or_no_charge_is_unknown() {
        let tariff = tariff_of(&policy_v2_value());
        assert_eq!(
            charge(&tariff, &usage(0, 0, 0, 0, 0)),
            Err(ChargeStop::InputZero)
        );
        assert_eq!(
            charge(&tariff, &usage(0, 0, 0, 0, 50)),
            Err(ChargeStop::InputZero)
        );
        // All cached at a free rate with no output comes to nothing.
        let mut value = policy_v2_none_value();
        value["tariff"]["tiers"][0]["cached_input_per_million"] = json!("0");
        let free = tariff_of(&value);
        let mut case = usage(1000, 1000, 0, 0, 0);
        case.cache_write_tokens = None;
        case.cache_write_30m_tokens = None;
        case.cache_write_12h_tokens = None;
        assert_eq!(charge(&free, &case), Err(ChargeStop::ZeroCharge));
        // One output token is something.
        case.output_tokens = 1;
        assert!(charge(&free, &case).is_ok());
    }

    #[test]
    fn counts_that_do_not_fit_together_are_refused() {
        let tariff = tariff_of(&policy_v2_value());
        assert_eq!(
            charge(&tariff, &usage(100, 60, 30, 20, 10)),
            Err(ChargeStop::UsageInconsistent)
        );
        let mut case = usage(100, 0, 0, 0, 10);
        case.reasoning_tokens = Some(11);
        assert_eq!(charge(&tariff, &case), Err(ChargeStop::UsageInconsistent));
    }

    #[test]
    fn the_tier_of_an_input_is_the_last_it_is_above() {
        let tariff = tariff_of(&policy_v2_value());
        for (input, tier) in [
            (0, 0),
            (1, 0),
            (271_999, 0),
            (272_000, 0),
            (272_001, 1),
            (u64::MAX, 1),
        ] {
            assert_eq!(tier_index(&tariff, input), tier, "{input}");
        }
    }

    /// The oracle's bound: the highest input rate of any class in any reachable tier on all of the
    /// bound, the highest output rate on all of the output, the highest fee.
    fn oracle_bound(value: &Value, input_bound: u64, max_output: u64) -> u128 {
        let tiers = if input_bound <= 272_000 {
            vec![0]
        } else {
            vec![0, 1]
        };
        let tiers = tiers
            .iter()
            .map(|i| oracle_tier(value, *i))
            .collect::<Vec<_>>();
        let input = tiers
            .iter()
            .map(|t| t.input.max(t.cached).max(t.w30).max(t.w12))
            .max()
            .unwrap();
        let output = tiers.iter().map(|t| t.output).max().unwrap();
        let fee = tiers.iter().map(|t| t.fee).max().unwrap();
        (u128::from(input_bound) * input + u128::from(max_output) * output).div_ceil(1_000_000)
            + fee
    }

    #[test]
    fn the_reservation_takes_a_tier_only_the_bound_reaches() {
        let value = policy_v2_value();
        let tariff = tariff_of(&value);
        // At the boundary only the low tier is reachable: its highest input rate is 4 (12h write).
        let at = reservation_bound(&tariff, 272_000, 50).unwrap();
        assert_eq!(at.units(), (272_000 * 4 + 50 * 8) * UNIT / 1_000_000);
        // One token past it, the high tier is reachable: 8 (12h write) and output 12.
        let past = reservation_bound(&tariff, 272_001, 50).unwrap();
        assert_eq!(past.units(), (272_001 * 8 + 50 * 12) * UNIT / 1_000_000);
        for (bound, output) in [(1, 1), (272_000, 50), (272_001, 50), (1_050_000, 128_000)] {
            assert_eq!(
                reservation_bound(&tariff, bound, output).unwrap().units(),
                oracle_bound(&value, bound, output),
                "{bound} {output}"
            );
        }
        // In none mode the write rates are not there to take.
        let none = policy_v2_none_value();
        assert_eq!(
            reservation_bound(&tariff_of(&none), 1000, 10)
                .unwrap()
                .units(),
            (1000 * 2 + 10 * 8) * UNIT / 1_000_000
        );
        assert_eq!(
            oracle_bound(&none, 1000, 10),
            (1000 * 2 + 10 * 8) * UNIT / 1_000_000
        );
    }

    #[test]
    fn the_reservation_takes_the_highest_fee_and_output_of_any_reachable_tier() {
        // A lower tier can be the dearer one: the bound takes each highest wherever it is.
        let mut value = policy_v2_value();
        value["tariff"]["tiers"][0]["request_fee"] = json!("0.5");
        value["tariff"]["tiers"][0]["output_per_million"] = json!("20");
        value["tariff"]["tiers"][1]["request_fee"] = json!("0.1");
        let tariff = tariff_of(&value);
        let got = reservation_bound(&tariff, 300_000, 100).unwrap();
        assert_eq!(got.units(), oracle_bound(&value, 300_000, 100));
        assert_eq!(
            got.units(),
            (300_000 * 8 + 100 * 20) * UNIT / 1_000_000 + UNIT / 2
        );
    }

    #[test]
    fn a_reservation_past_counting_is_an_overflow() {
        let mut value = policy_v2_value();
        value["tariff"]["tiers"][0]["cache_write_12h_per_million"] = json!("1e20");
        let tariff = tariff_of(&value);
        assert_eq!(reservation_bound(&tariff, 4, 0), Err(ChargeStop::Overflow));
        assert!(reservation_bound(&tariff, 3, 0).is_ok());
    }

    /// Review 1, should 3: each input class in turn is the one high price, and the reservation
    /// takes it, in ttl mode on one tier and from a lower tier the bound also reaches.
    #[test]
    fn the_reservation_takes_whichever_input_class_is_the_dearest() {
        // One tier, every rate 1, then one class at 100: the bound at 100, the output at 1.
        let bound: u128 = 1000;
        let output: u128 = 7;
        let expected = |high: u128| (bound * high * UNIT + output * UNIT).div_ceil(1_000_000);
        for key in [
            "input_per_million",
            "cached_input_per_million",
            "cache_write_30m_per_million",
            "cache_write_12h_per_million",
        ] {
            let mut value = policy_v2_value();
            value["tariff"]["tiers"].as_array_mut().unwrap().truncate(1);
            let tier = &mut value["tariff"]["tiers"][0];
            for each in [
                "input_per_million",
                "cached_input_per_million",
                "cache_write_30m_per_million",
                "cache_write_12h_per_million",
                "output_per_million",
            ] {
                tier[each] = json!("1");
            }
            tier[key] = json!("100");
            let reserved = reservation_bound(&tariff_of(&value), 1000, 7).unwrap();
            assert_eq!(reserved.units(), expected(100), "{key}");
            // And the formula is not every class at 100.
            assert_ne!(reserved.units(), expected(1), "{key}");
        }
        // Two tiers, the lower one's ordinary input the dearest: a bound past the boundary still
        // takes it.
        let mut value = policy_v2_value();
        for index in [0, 1] {
            let tier = &mut value["tariff"]["tiers"][index];
            for each in [
                "input_per_million",
                "cached_input_per_million",
                "cache_write_30m_per_million",
                "cache_write_12h_per_million",
                "output_per_million",
            ] {
                tier[each] = json!("1");
            }
        }
        value["tariff"]["tiers"][0]["input_per_million"] = json!("100");
        let past: u128 = 300_000;
        assert_eq!(
            reservation_bound(&tariff_of(&value), 300_000, 7)
                .unwrap()
                .units(),
            (past * 100 * UNIT + output * UNIT).div_ceil(1_000_000)
        );
        // The same for the lower tier's 12h write.
        value["tariff"]["tiers"][0]["input_per_million"] = json!("1");
        value["tariff"]["tiers"][0]["cache_write_12h_per_million"] = json!("100");
        assert_eq!(
            reservation_bound(&tariff_of(&value), 300_000, 7)
                .unwrap()
                .units(),
            (past * 100 * UNIT + output * UNIT).div_ceil(1_000_000)
        );
    }

    #[test]
    fn no_reachable_usage_costs_more_than_its_reservation() {
        let mut rng = Rng::seeded(1006);
        for mode in [policy_v2_value(), policy_v2_none_value()] {
            let tariff = tariff_of(&mode);
            let ttl = mode["tariff"]["cache_write_mode"] == "ttl_30m_12h";
            for _ in 0..2000 {
                let bound = 1 + rng.next() % 600_000;
                let max_output = rng.next() % 200_000;
                let reserved = reservation_bound(&tariff, bound, max_output).unwrap();
                let input = 1 + rng.next() % bound;
                let output = rng.next() % (max_output + 1);
                let cached = rng.next() % (input + 1);
                let (w30, w12) = if ttl {
                    let w30 = rng.next() % (input - cached + 1);
                    (w30, rng.next() % (input - cached - w30 + 1))
                } else {
                    (0, 0)
                };
                let case = usage(input, cached, w30, w12, output);
                match charge(&tariff, &case) {
                    Ok(got) => {
                        assert!(got.charge <= reserved, "{case:?} {bound} {max_output}");
                        assert_eq!(
                            got.charge.units(),
                            oracle(&mode, input, cached, w30, w12, output)
                        );
                    }
                    Err(stop) => assert_eq!(stop, ChargeStop::ZeroCharge, "{case:?}"),
                }
            }
        }
    }

    #[test]
    fn a_read_body_is_charged_through_the_shared_reader() {
        let value = policy_v2_value();
        let body = json!({"usage": {
            "input_tokens": 272_001,
            "input_tokens_details": {
                "cached_tokens": 200_000,
                "cache_write_tokens": 1000,
                "cache_write_tokens_details": {"ttl_30m_tokens": 600, "ttl_12h_tokens": 400}
            },
            "output_tokens": 900,
            "output_tokens_details": {"reasoning_tokens": 700},
            "total_tokens": 272_901
        }})
        .to_string();
        let usage = read_responses_usage(&body).wallet.unwrap();
        let got = charge(&tariff_of(&value), &usage).unwrap();
        assert_eq!(
            got.charge.units(),
            oracle(&value, 272_001, 200_000, 600, 400, 900)
        );
        assert_eq!(got.tokens, 272_901);
        // The same body with only the write total cannot be charged in ttl mode, and can in none
        // mode only if that total is zero.
        let total_only = body.replace(
            r#","cache_write_tokens_details":{"ttl_12h_tokens":400,"ttl_30m_tokens":600}"#,
            "",
        );
        assert_ne!(total_only, body);
        let usage = read_responses_usage(&total_only).wallet.unwrap();
        assert_eq!(
            charge(&tariff_of(&value), &usage),
            Err(ChargeStop::CacheWriteFieldMissing)
        );
        assert_eq!(
            charge(&tariff_of(&policy_v2_none_value()), &usage),
            Err(ChargeStop::CacheWriteInNoneMode)
        );
    }

    #[test]
    fn the_stop_words_are_fixed() {
        for (stop, word) in [
            (
                ChargeStop::CacheWriteFieldMissing,
                "cache_write_field_missing",
            ),
            (ChargeStop::CacheWriteInNoneMode, "cache_write_in_none_mode"),
            (ChargeStop::UsageInconsistent, "usage_inconsistent"),
            (ChargeStop::InputZero, "input_zero"),
            (ChargeStop::ZeroCharge, "zero_charge"),
            (ChargeStop::Overflow, "overflow"),
            (ChargeStop::TariffInconsistent, "tariff_inconsistent"),
        ] {
            assert_eq!(stop.as_str(), word);
            assert_eq!(serde_json::to_value(stop).unwrap(), json!(word));
        }
    }

    #[test]
    fn a_tariff_whose_tiers_do_not_match_its_mode_is_refused() {
        let mut tariff = tariff_of(&policy_v2_value());
        tariff.tiers[1].cache_write = None;
        assert_eq!(
            charge(&tariff, &usage(300_000, 0, 0, 0, 1)),
            Err(ChargeStop::TariffInconsistent)
        );
        assert_eq!(
            reservation_bound(&tariff, 300_000, 1),
            Err(ChargeStop::TariffInconsistent)
        );
        // The low tier is as it was.
        assert!(charge(&tariff, &usage(1000, 0, 0, 0, 1)).is_ok());
    }
}
