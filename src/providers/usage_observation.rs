//! One reading of a response's raw `usage`, shared by the run-budget wallet and the per-call
//! usage telemetry (Issue #1006, designs 3 and 5).
//!
//! Everything is read from the raw bytes, never from a parsed `serde_json::Value`: a `Value` keeps
//! the last of two repeated keys, so a repeat could hide a count. Every object read here keeps its
//! members in order, and a key that comes twice is seen.
//!
//! Two answers come out of one reading of a Responses body:
//!
//! - **the wallet's**: either a [`ResponsesUsage`] that passed the allowlist and every consistency
//!   rule, or one [`UsageStop`] in fixed words. The rules that depend on the policy's
//!   `cache_write_mode`, and the rule that a sent body cannot read no input or cost nothing, are in
//!   `structured_budget::usage_tariff`, which takes this usage.
//! - **the telemetry's**: the cache-write total, and its TTL split, only when actually reported and
//!   valid ([`CacheWriteObservation`]). Missing is `None`, never zero; an explicit zero is kept; a
//!   total with no split stays a total, with no split made up. It reads only the cache-write paths
//!   of the Issue's table, so it also works for a compatible provider whose usage carries fields the
//!   wallet does not allow.
//!
//! A Chat body has only the telemetry answer ([`observe_chat_cache_write`]), from the Chat paths of
//! the same table.

#![allow(
    dead_code,
    reason = "the wallet and the telemetry take this reading when their send paths are wired in"
)]

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

const USAGE: &str = "usage";

const RESPONSES_INPUT: &str = "input_tokens";
const RESPONSES_INPUT_DETAILS: &str = "input_tokens_details";
const RESPONSES_OUTPUT: &str = "output_tokens";
const RESPONSES_OUTPUT_DETAILS: &str = "output_tokens_details";
const RESPONSES_TOTAL: &str = "total_tokens";

const CHAT_INPUT: &str = "prompt_tokens";
const CHAT_INPUT_DETAILS: &str = "prompt_tokens_details";

const CACHED: &str = "cached_tokens";
const CACHE_WRITE: &str = "cache_write_tokens";
const CACHE_WRITE_DETAILS: &str = "cache_write_tokens_details";
const TTL_30M: &str = "ttl_30m_tokens";
const TTL_12H: &str = "ttl_12h_tokens";
const REASONING: &str = "reasoning_tokens";

const USAGE_KEYS: &[&str] = &[
    RESPONSES_INPUT,
    RESPONSES_INPUT_DETAILS,
    RESPONSES_OUTPUT,
    RESPONSES_OUTPUT_DETAILS,
    RESPONSES_TOTAL,
];
const INPUT_DETAILS_KEYS: &[&str] = &[CACHED, CACHE_WRITE, CACHE_WRITE_DETAILS];
const OUTPUT_DETAILS_KEYS: &[&str] = &[REASONING];
const CACHE_WRITE_DETAILS_KEYS: &[&str] = &[TTL_30M, TTL_12H];

/// Why the wallet cannot take a response's usage. Fixed words, safe for a ledger and a message:
/// nothing of the body is quoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UsageStop {
    /// The body is not one JSON object.
    ResponseMalformed,
    /// The body has no `usage`.
    UsageMissing,
    /// `usage` is `null` or not an object.
    UsageNotObject,
    /// A key comes twice in the body's top level, `usage`, or one of its three detail objects.
    DuplicateKey,
    /// A key the allowlist does not name, with a value other than the JSON integer `0`.
    UnknownField,
    /// A count the allowlist names is not an unsigned JSON integer within `u64`, or a detail
    /// object is not an object: `null`, strings, negatives, fractions, exponents, and overflow.
    InvalidValue,
    /// `input_tokens`, `output_tokens`, `input_tokens_details` or its `cached_tokens` is absent.
    RequiredFieldMissing,
    /// The TTL counts do not add up to `cache_write_tokens`, or there is a TTL with no total.
    CacheWriteSplitMismatch,
    /// Cached and written input together are more than the input.
    CachedAndWrittenOverInput,
    ReasoningOverOutput,
    /// `total_tokens` is not input plus output, or that sum is past counting.
    TotalMismatch,
}

impl UsageStop {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ResponseMalformed => "response_malformed",
            Self::UsageMissing => "usage_missing",
            Self::UsageNotObject => "usage_not_object",
            Self::DuplicateKey => "duplicate_key",
            Self::UnknownField => "unknown_field",
            Self::InvalidValue => "invalid_value",
            Self::RequiredFieldMissing => "required_field_missing",
            Self::CacheWriteSplitMismatch => "cache_write_split_mismatch",
            Self::CachedAndWrittenOverInput => "cached_and_written_over_input",
            Self::ReasoningOverOutput => "reasoning_over_output",
            Self::TotalMismatch => "total_mismatch",
        }
    }
}

/// A Responses usage that passed the allowlist and every rule that holds whatever the policy's
/// cache-write mode. Absent optional counts are `None`, never zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResponsesUsage {
    /// `I`: all input tokens, cached and written ones included.
    pub(crate) input_tokens: u64,
    /// `O`: all output tokens, reasoning included.
    pub(crate) output_tokens: u64,
    /// `C`.
    pub(crate) cached_tokens: u64,
    pub(crate) reasoning_tokens: Option<u64>,
    pub(crate) total_tokens: Option<u64>,
    /// `W`. When both TTLs are there, it is their sum.
    pub(crate) cache_write_tokens: Option<u64>,
    /// `W30`.
    pub(crate) cache_write_30m_tokens: Option<u64>,
    /// `W12`.
    pub(crate) cache_write_12h_tokens: Option<u64>,
}

impl ResponsesUsage {
    /// What the telemetry records of this usage's cache writes: the same values the wallet took.
    pub(crate) fn cache_write_observation(&self) -> Option<CacheWriteObservation> {
        let total = self.cache_write_tokens?;
        let ttl = match (self.cache_write_30m_tokens, self.cache_write_12h_tokens) {
            (Some(ttl_30m), Some(ttl_12h)) => Some(CacheWriteTtl { ttl_30m, ttl_12h }),
            _ => None,
        };
        Some(CacheWriteObservation { total, ttl })
    }
}

/// The TTL split of a cache write, recorded only when both counts were reported and add up to the
/// total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheWriteTtl {
    pub(crate) ttl_30m: u64,
    pub(crate) ttl_12h: u64,
}

/// A cache write a provider actually reported and that is valid. A body that reports none has no
/// value of this type, so a missing total is never a zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheWriteObservation {
    pub(crate) total: u64,
    pub(crate) ttl: Option<CacheWriteTtl>,
}

/// One reading of a Responses body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResponsesUsageReading {
    /// A usage the wallet can settle: complete, allowlisted, and consistent.
    pub(crate) wallet: Result<ResponsesUsage, UsageStop>,
    /// The allowlisted counts when each of them, and every rule between them, holds, whatever
    /// keys the allowlist does not name say. Equal to `wallet` when that is `Ok`. On a stop it is
    /// kept as what was observed and is never charged: an unknown meter could price something
    /// these counts leave out. Neither an unknown key nor its value is in it.
    pub(crate) observed: Option<ResponsesUsage>,
    pub(crate) cache_write: Option<CacheWriteObservation>,
}

/// The members of one JSON object, in order, repeats kept.
struct Members<'a>(Vec<(String, &'a RawValue)>);

impl<'de> Deserialize<'de> for Members<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Ordered;
        impl<'de> Visitor<'de> for Ordered {
            type Value = Members<'de>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut members = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    let value: &'de RawValue = map.next_value()?;
                    members.push((key, value));
                }
                Ok(Members(members))
            }
        }
        deserializer.deserialize_map(Ordered)
    }
}

/// What one key of an object holds.
enum Member<'a> {
    Absent,
    Repeated,
    Once(&'a RawValue),
}

impl<'a> Members<'a> {
    fn parse(text: &'a str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    fn get(&self, key: &str) -> Member<'a> {
        let mut found = self.0.iter().filter(|(name, _)| name == key);
        match (found.next(), found.next()) {
            (None, _) => Member::Absent,
            (Some((_, value)), None) => Member::Once(value),
            (Some(_), Some(_)) => Member::Repeated,
        }
    }

    fn has_repeat(&self) -> bool {
        self.0
            .iter()
            .enumerate()
            .any(|(at, (key, _))| self.0[..at].iter().any(|(earlier, _)| earlier == key))
    }

    /// Whether every key is listed, or holds the JSON integer `0`.
    fn only_listed_or_zero(&self, listed: &[&str]) -> bool {
        self.0
            .iter()
            .all(|(key, value)| listed.contains(&key.as_str()) || value.get().trim() == "0")
    }
}

/// An unsigned JSON integer within `u64`: digits only, so no sign, fraction, exponent, `null`, or
/// string, and refused past `u64::MAX`.
fn count(value: &RawValue) -> Option<u64> {
    let text = value.get().trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

fn is_null(value: &RawValue) -> bool {
    value.get().trim() == "null"
}

/// The body's `usage` object, refusing a repeat of the key.
fn usage_object(body: &str) -> Result<Members<'_>, UsageStop> {
    let top = Members::parse(body).ok_or(UsageStop::ResponseMalformed)?;
    match top.get(USAGE) {
        Member::Absent => Err(UsageStop::UsageMissing),
        Member::Repeated => Err(UsageStop::DuplicateKey),
        Member::Once(raw) if is_null(raw) => Err(UsageStop::UsageNotObject),
        Member::Once(raw) => Members::parse(raw.get()).ok_or(UsageStop::UsageNotObject),
    }
}

/// A count of an allowlisted key: absent is `None`; there, it has to be a count.
fn wallet_count(object: &Members<'_>, key: &str) -> Result<Option<u64>, UsageStop> {
    match object.get(key) {
        Member::Absent => Ok(None),
        Member::Repeated => Err(UsageStop::DuplicateKey),
        Member::Once(raw) => count(raw).map(Some).ok_or(UsageStop::InvalidValue),
    }
}

/// An allowlisted detail object: absent is `None`; there, it has to be an object of only its listed
/// keys (or zeros), each once.
fn wallet_details<'a>(
    object: &Members<'a>,
    key: &str,
    listed: Option<&[&str]>,
) -> Result<Option<Members<'a>>, UsageStop> {
    match object.get(key) {
        Member::Absent => Ok(None),
        Member::Repeated => Err(UsageStop::DuplicateKey),
        Member::Once(raw) => {
            let details = Members::parse(raw.get()).ok_or(UsageStop::InvalidValue)?;
            checked_object(&details, listed)?;
            Ok(Some(details))
        }
    }
}

/// No key twice, and, when `listed` is given, every key listed or the integer `0`. With no list
/// the keys the allowlist does not name are passed over, never read.
fn checked_object(object: &Members<'_>, listed: Option<&[&str]>) -> Result<(), UsageStop> {
    if object.has_repeat() {
        return Err(UsageStop::DuplicateKey);
    }
    if listed.is_some_and(|listed| !object.only_listed_or_zero(listed)) {
        return Err(UsageStop::UnknownField);
    }
    Ok(())
}

/// The total of a cache write and its TTLs agree: a TTL needs a total, both TTLs add up to it, and
/// one TTL is not more than it.
fn split_agrees(total: Option<u64>, ttl_30m: Option<u64>, ttl_12h: Option<u64>) -> bool {
    match (total, ttl_30m, ttl_12h) {
        (_, None, None) => true,
        (None, _, _) => false,
        (Some(total), Some(ttl_30m), Some(ttl_12h)) => ttl_30m.checked_add(ttl_12h) == Some(total),
        (Some(total), Some(one), None) | (Some(total), None, Some(one)) => one <= total,
    }
}

fn wallet_usage(body: &str) -> Result<ResponsesUsage, UsageStop> {
    allowlisted_usage(body, true)
}

/// The allowlisted counts under every rule; with `allowlist_only` false, keys the allowlist does
/// not name are passed over instead of stopping the reading.
fn allowlisted_usage(body: &str, allowlist_only: bool) -> Result<ResponsesUsage, UsageStop> {
    let list = |keys: &'static [&'static str]| allowlist_only.then_some(keys);
    let usage = usage_object(body)?;
    checked_object(&usage, list(USAGE_KEYS))?;
    let input_details = wallet_details(&usage, RESPONSES_INPUT_DETAILS, list(INPUT_DETAILS_KEYS))?;
    let output_details =
        wallet_details(&usage, RESPONSES_OUTPUT_DETAILS, list(OUTPUT_DETAILS_KEYS))?;
    let write_details = match &input_details {
        Some(details) => {
            wallet_details(details, CACHE_WRITE_DETAILS, list(CACHE_WRITE_DETAILS_KEYS))?
        }
        None => None,
    };
    let input = wallet_count(&usage, RESPONSES_INPUT)?;
    let output = wallet_count(&usage, RESPONSES_OUTPUT)?;
    let total = wallet_count(&usage, RESPONSES_TOTAL)?;
    let (cached, write) = match &input_details {
        Some(details) => (
            wallet_count(details, CACHED)?,
            wallet_count(details, CACHE_WRITE)?,
        ),
        None => (None, None),
    };
    let reasoning = match &output_details {
        Some(details) => wallet_count(details, REASONING)?,
        None => None,
    };
    let (ttl_30m, ttl_12h) = match &write_details {
        Some(details) => (
            wallet_count(details, TTL_30M)?,
            wallet_count(details, TTL_12H)?,
        ),
        None => (None, None),
    };
    let (Some(input), Some(output), Some(cached)) = (input, output, cached) else {
        return Err(UsageStop::RequiredFieldMissing);
    };
    if !split_agrees(write, ttl_30m, ttl_12h) {
        return Err(UsageStop::CacheWriteSplitMismatch);
    }
    if cached
        .checked_add(write.unwrap_or(0))
        .is_none_or(|read_and_written| read_and_written > input)
    {
        return Err(UsageStop::CachedAndWrittenOverInput);
    }
    if reasoning.is_some_and(|reasoning| reasoning > output) {
        return Err(UsageStop::ReasoningOverOutput);
    }
    let sum = input.checked_add(output);
    if sum.is_none() || total.is_some_and(|total| Some(total) != sum) {
        return Err(UsageStop::TotalMismatch);
    }
    Ok(ResponsesUsage {
        input_tokens: input,
        output_tokens: output,
        cached_tokens: cached,
        reasoning_tokens: reasoning,
        total_tokens: total,
        cache_write_tokens: write,
        cache_write_30m_tokens: ttl_30m,
        cache_write_12h_tokens: ttl_12h,
    })
}

/// A count the telemetry reads: absent is `Ok(None)`; a repeat or a value that is no count is
/// `Err`, and the whole observation is then left out.
fn observed_count(object: &Members<'_>, key: &str) -> Result<Option<u64>, ()> {
    match object.get(key) {
        Member::Absent => Ok(None),
        Member::Repeated => Err(()),
        Member::Once(raw) => count(raw).map(Some).ok_or(()),
    }
}

/// A detail object the telemetry reads: absent is `Ok(None)`; a repeat, `null`, or not an object
/// is `Err`.
fn observed_object<'a>(object: &Members<'a>, key: &str) -> Result<Option<Members<'a>>, ()> {
    match object.get(key) {
        Member::Absent => Ok(None),
        Member::Repeated => Err(()),
        Member::Once(raw) => Members::parse(raw.get()).map(Some).ok_or(()),
    }
}

/// The cache write at the table's paths under `usage`, with the input count and its details named
/// by the API. Every count read is checked as the wallet checks it, and anything that is not valid
/// (a repeat, a wrong type, a split that does not add up, a write past the input) leaves the whole
/// observation out rather than recording a part of it as valid.
fn observe(body: &str, input_key: &str, details_key: &str) -> Option<CacheWriteObservation> {
    let usage = usage_object(body).ok()?;
    let details = observed_object(&usage, details_key).ok()??;
    let total = observed_count(&details, CACHE_WRITE).ok()?;
    let (ttl_30m, ttl_12h) = match observed_object(&details, CACHE_WRITE_DETAILS).ok()? {
        Some(split) => (
            observed_count(&split, TTL_30M).ok()?,
            observed_count(&split, TTL_12H).ok()?,
        ),
        None => (None, None),
    };
    if !split_agrees(total, ttl_30m, ttl_12h) {
        return None;
    }
    let total = total?;
    let input = observed_count(&usage, input_key).ok()?;
    let cached = observed_count(&details, CACHED).ok()?;
    if let Some(input) = input
        && cached
            .unwrap_or(0)
            .checked_add(total)
            .is_none_or(|read_and_written| read_and_written > input)
    {
        return None;
    }
    let ttl = match (ttl_30m, ttl_12h) {
        (Some(ttl_30m), Some(ttl_12h)) => Some(CacheWriteTtl { ttl_30m, ttl_12h }),
        _ => None,
    };
    Some(CacheWriteObservation { total, ttl })
}

/// Reads a Responses body once for the wallet and the telemetry.
pub(crate) fn read_responses_usage(body: &str) -> ResponsesUsageReading {
    let wallet = wallet_usage(body);
    let observed = match wallet {
        Ok(usage) => Some(usage),
        Err(UsageStop::UnknownField) => allowlisted_usage(body, false).ok(),
        Err(_) => None,
    };
    ResponsesUsageReading {
        wallet,
        observed,
        cache_write: observe(body, RESPONSES_INPUT, RESPONSES_INPUT_DETAILS),
    }
}

/// The cache write a Chat body reports at `usage.prompt_tokens_details`, and only there.
pub(crate) fn observe_chat_cache_write(body: &str) -> Option<CacheWriteObservation> {
    observe(body, CHAT_INPUT, CHAT_INPUT_DETAILS)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{
        CacheWriteObservation, CacheWriteTtl, ResponsesUsage, UsageStop, observe_chat_cache_write,
        read_responses_usage,
    };

    /// A complete Responses usage: 1000 in, 300 cached, 200 written (150 for 30m, 50 for 12h),
    /// 80 out of which 30 reasoning.
    fn complete() -> Value {
        json!({
            "id": "resp_1",
            "output": [],
            "usage": {
                "input_tokens": 1000,
                "input_tokens_details": {
                    "cached_tokens": 300,
                    "cache_write_tokens": 200,
                    "cache_write_tokens_details": {"ttl_30m_tokens": 150, "ttl_12h_tokens": 50}
                },
                "output_tokens": 80,
                "output_tokens_details": {"reasoning_tokens": 30},
                "total_tokens": 1080
            }
        })
    }

    fn with(change: impl FnOnce(&mut Value)) -> String {
        let mut value = complete();
        change(&mut value);
        value.to_string()
    }

    fn stop(body: &str) -> UsageStop {
        read_responses_usage(body).wallet.unwrap_err()
    }

    fn observed(total: u64, ttl: Option<(u64, u64)>) -> Option<CacheWriteObservation> {
        Some(CacheWriteObservation {
            total,
            ttl: ttl.map(|(ttl_30m, ttl_12h)| CacheWriteTtl { ttl_30m, ttl_12h }),
        })
    }

    #[test]
    fn a_complete_usage_is_read_with_its_exact_counts() {
        let reading = read_responses_usage(&complete().to_string());
        assert_eq!(
            reading.wallet,
            Ok(ResponsesUsage {
                input_tokens: 1000,
                output_tokens: 80,
                cached_tokens: 300,
                reasoning_tokens: Some(30),
                total_tokens: Some(1080),
                cache_write_tokens: Some(200),
                cache_write_30m_tokens: Some(150),
                cache_write_12h_tokens: Some(50),
            })
        );
        assert_eq!(reading.cache_write, observed(200, Some((150, 50))));
    }

    #[test]
    fn the_wallet_and_the_telemetry_agree_on_what_the_wallet_takes() {
        for body in [
            complete().to_string(),
            with(|v| {
                v["usage"]["input_tokens_details"]
                    .as_object_mut()
                    .unwrap()
                    .remove("cache_write_tokens_details");
            }),
            with(|v| {
                let details = v["usage"]["input_tokens_details"].as_object_mut().unwrap();
                details.remove("cache_write_tokens_details");
                details.remove("cache_write_tokens");
            }),
            with(|v| {
                v["usage"]["input_tokens_details"]["cache_write_tokens"] = json!(0);
                v["usage"]["input_tokens_details"]["cache_write_tokens_details"] =
                    json!({"ttl_30m_tokens": 0, "ttl_12h_tokens": 0});
            }),
        ] {
            let reading = read_responses_usage(&body);
            let usage = reading.wallet.unwrap();
            assert_eq!(
                usage.cache_write_observation(),
                reading.cache_write,
                "{body}"
            );
        }
    }

    #[test]
    fn the_smallest_usage_has_input_output_and_cached_only() {
        let body = json!({"usage": {
            "input_tokens": 5,
            "input_tokens_details": {"cached_tokens": 0},
            "output_tokens": 1
        }})
        .to_string();
        let reading = read_responses_usage(&body);
        let usage = reading.wallet.unwrap();
        assert_eq!(
            (usage.input_tokens, usage.output_tokens, usage.cached_tokens),
            (5, 1, 0)
        );
        assert_eq!(usage.reasoning_tokens, None);
        assert_eq!(usage.total_tokens, None);
        assert_eq!(usage.cache_write_tokens, None);
        assert_eq!(usage.cache_write_30m_tokens, None);
        assert_eq!(usage.cache_write_12h_tokens, None);
        // Missing is not zero.
        assert_eq!(reading.cache_write, None);
    }

    #[test]
    fn a_body_or_usage_that_is_not_an_object_is_named() {
        for body in ["", "not json", "[]", "null", "1", r#"{"usage":1"#] {
            assert_eq!(stop(body), UsageStop::ResponseMalformed, "{body}");
        }
        assert_eq!(stop(r#"{"id":"r"}"#), UsageStop::UsageMissing);
        for usage in ["null", "1", "\"x\"", "[]", "true"] {
            assert_eq!(
                stop(&format!(r#"{{"usage":{usage}}}"#)),
                UsageStop::UsageNotObject,
                "{usage}"
            );
        }
    }

    #[test]
    fn a_repeated_key_is_seen_at_every_level() {
        let text = complete().to_string();
        for (once, twice) in [
            // The body's top level.
            (r#""usage":{"#, r#""usage":{},"usage":{"#),
            // usage.
            (
                r#""input_tokens":1000"#,
                r#""input_tokens":1,"input_tokens":1000"#,
            ),
            (
                r#""total_tokens":1080"#,
                r#""total_tokens":1080,"total_tokens":1080"#,
            ),
            (
                r#""output_tokens_details":{"#,
                r#""output_tokens_details":{"reasoning_tokens":0},"output_tokens_details":{"#,
            ),
            // input_tokens_details.
            (
                r#""cached_tokens":300"#,
                r#""cached_tokens":0,"cached_tokens":300"#,
            ),
            (
                r#""cache_write_tokens":200"#,
                r#""cache_write_tokens":200,"cache_write_tokens":200"#,
            ),
            // output_tokens_details.
            (
                r#""reasoning_tokens":30"#,
                r#""reasoning_tokens":30,"reasoning_tokens":30"#,
            ),
            // cache_write_tokens_details.
            (
                r#""ttl_30m_tokens":150"#,
                r#""ttl_30m_tokens":0,"ttl_30m_tokens":150"#,
            ),
            (
                r#""ttl_12h_tokens":50"#,
                r#""ttl_12h_tokens":50,"ttl_12h_tokens":50"#,
            ),
        ] {
            assert_eq!(text.matches(once).count(), 1, "{once}");
            let body = text.replace(once, twice);
            assert_eq!(stop(&body), UsageStop::DuplicateKey, "{twice}");
        }
        // A repeat of an unknown zero is a repeat too.
        let body = text.replace(
            r#""total_tokens":1080"#,
            r#""total_tokens":1080,"x":0,"x":0"#,
        );
        assert_eq!(stop(&body), UsageStop::DuplicateKey);
    }

    #[test]
    fn a_repeated_cache_write_key_leaves_the_telemetry_out() {
        let text = complete().to_string();
        for (once, twice) in [
            (r#""usage":{"#, r#""usage":{},"usage":{"#),
            (
                r#""input_tokens_details":{"#,
                r#""input_tokens_details":{},"input_tokens_details":{"#,
            ),
            (
                r#""cache_write_tokens":200"#,
                r#""cache_write_tokens":0,"cache_write_tokens":200"#,
            ),
            (
                r#""ttl_30m_tokens":150"#,
                r#""ttl_30m_tokens":150,"ttl_30m_tokens":150"#,
            ),
            (
                r#""cached_tokens":300"#,
                r#""cached_tokens":300,"cached_tokens":300"#,
            ),
            (
                r#""input_tokens":1000"#,
                r#""input_tokens":1000,"input_tokens":1000"#,
            ),
        ] {
            let body = text.replace(once, twice);
            assert_eq!(read_responses_usage(&body).cache_write, None, "{twice}");
        }
    }

    #[test]
    fn every_malformed_count_is_an_invalid_value() {
        let pointers = [
            "/usage/input_tokens",
            "/usage/output_tokens",
            "/usage/total_tokens",
            "/usage/input_tokens_details/cached_tokens",
            "/usage/input_tokens_details/cache_write_tokens",
            "/usage/input_tokens_details/cache_write_tokens_details/ttl_30m_tokens",
            "/usage/input_tokens_details/cache_write_tokens_details/ttl_12h_tokens",
            "/usage/output_tokens_details/reasoning_tokens",
        ];
        for pointer in pointers {
            for bad in [
                json!(null),
                json!(-1),
                json!(1.5),
                json!(1.0),
                json!(1e3),
                json!("1"),
                json!(true),
                json!([1]),
                json!({"n": 1}),
            ] {
                let body = with(|v| *v.pointer_mut(pointer).unwrap() = bad.clone());
                assert_eq!(stop(&body), UsageStop::InvalidValue, "{pointer} {bad}");
            }
            // One past u64, spelled as the integer token a provider could send.
            let body = with(|v| *v.pointer_mut(pointer).unwrap() = json!("OVERFLOW"))
                .replace("\"OVERFLOW\"", "18446744073709551616");
            assert_eq!(stop(&body), UsageStop::InvalidValue, "{pointer} overflow");
        }
        // u64::MAX itself is a count.
        let body = complete().to_string().replace(
            r#""input_tokens":1000"#,
            &format!(r#""input_tokens":{}"#, u64::MAX),
        );
        assert_eq!(stop(&body), UsageStop::TotalMismatch);
        for pointer in [
            "/usage/input_tokens_details",
            "/usage/output_tokens_details",
            "/usage/input_tokens_details/cache_write_tokens_details",
        ] {
            for bad in [json!(null), json!(0), json!("x"), json!([])] {
                let body = with(|v| *v.pointer_mut(pointer).unwrap() = bad.clone());
                assert_eq!(stop(&body), UsageStop::InvalidValue, "{pointer} {bad}");
            }
        }
    }

    #[test]
    fn a_malformed_cache_write_count_leaves_the_telemetry_out() {
        for pointer in [
            "/usage/input_tokens_details/cache_write_tokens",
            "/usage/input_tokens_details/cache_write_tokens_details/ttl_30m_tokens",
            "/usage/input_tokens_details/cache_write_tokens_details/ttl_12h_tokens",
            "/usage/input_tokens_details/cache_write_tokens_details",
            "/usage/input_tokens_details",
        ] {
            for bad in [json!(null), json!(-1), json!(1.5), json!("200")] {
                let body = with(|v| *v.pointer_mut(pointer).unwrap() = bad.clone());
                assert_eq!(
                    read_responses_usage(&body).cache_write,
                    None,
                    "{pointer} {bad}"
                );
            }
        }
        // With no split to disagree with, a malformed total is neither a zero nor a total.
        for bad in [json!(null), json!(-1), json!(1.5), json!("200"), json!([])] {
            let body = with(|v| {
                let details = v["usage"]["input_tokens_details"].as_object_mut().unwrap();
                details.remove("cache_write_tokens_details");
                details.insert("cache_write_tokens".into(), bad.clone());
            });
            assert_eq!(read_responses_usage(&body).cache_write, None, "{bad}");
        }
    }

    #[test]
    fn an_unknown_key_passes_only_as_the_integer_zero() {
        for pointer in [
            "/usage",
            "/usage/input_tokens_details",
            "/usage/output_tokens_details",
            "/usage/input_tokens_details/cache_write_tokens_details",
        ] {
            let body = with(|v| {
                v.pointer_mut(pointer).unwrap()["audio_tokens"] = json!(0);
            });
            assert!(read_responses_usage(&body).wallet.is_ok(), "{pointer} zero");
            for bad in [
                json!(1),
                json!(0.0),
                json!(-0.0),
                json!(null),
                json!("0"),
                json!([]),
                json!({}),
                json!(false),
            ] {
                let body = with(|v| {
                    v.pointer_mut(pointer).unwrap()["audio_tokens"] = bad.clone();
                });
                assert_eq!(stop(&body), UsageStop::UnknownField, "{pointer} {bad}");
            }
        }
        // An unknown non-zero key, such as a compatible provider's cost, stops the wallet and
        // leaves the cache-write observation as it is.
        let body = with(|v| v["usage"]["cost"] = json!(0.0012));
        let reading = read_responses_usage(&body);
        assert_eq!(reading.wallet, Err(UsageStop::UnknownField));
        assert_eq!(reading.cache_write, observed(200, Some((150, 50))));
        // A key outside `usage` is not the wallet's.
        let body = with(|v| v["billing"] = json!({"amount": 3}));
        assert!(read_responses_usage(&body).wallet.is_ok());
    }

    #[test]
    fn a_usage_stopped_only_for_an_unknown_key_is_still_observed_and_no_other_stop_is() {
        let complete_usage = read_responses_usage(&complete().to_string())
            .wallet
            .unwrap();
        for pointer in [
            "/usage",
            "/usage/input_tokens_details",
            "/usage/output_tokens_details",
            "/usage/input_tokens_details/cache_write_tokens_details",
        ] {
            let body = with(|v| v.pointer_mut(pointer).unwrap()["future_meter"] = json!(1));
            let reading = read_responses_usage(&body);
            assert_eq!(reading.wallet, Err(UsageStop::UnknownField), "{pointer}");
            assert_eq!(reading.observed, Some(complete_usage), "{pointer}");
        }
        // A complete usage is observed as it is settled.
        let reading = read_responses_usage(&complete().to_string());
        assert_eq!(reading.observed, reading.wallet.ok());
        // Any other stop observes nothing: a repeat, a bad value, a rule broken, a count missing,
        // even with an unknown key beside it.
        for body in [
            with(|v| v["usage"]["future_meter"] = json!(1)).replace(
                r#""input_tokens":1000"#,
                r#""input_tokens":1,"input_tokens":1000"#,
            ),
            with(|v| {
                v["usage"]["future_meter"] = json!(1);
                v["usage"]["input_tokens"] = json!("1000");
            }),
            with(|v| {
                v["usage"]["future_meter"] = json!(1);
                v["usage"]["output_tokens_details"]["reasoning_tokens"] = json!(81);
            }),
            with(|v| {
                v["usage"]["future_meter"] = json!(1);
                v["usage"].as_object_mut().unwrap().remove("output_tokens");
            }),
            with(|v| v["usage"]["total_tokens"] = json!(1)),
            "{}".to_string(),
        ] {
            assert_eq!(read_responses_usage(&body).observed, None, "{body}");
        }
    }

    #[test]
    fn a_required_count_that_is_absent_is_named() {
        for pointer in [
            "/usage/input_tokens",
            "/usage/output_tokens",
            "/usage/input_tokens_details",
            "/usage/input_tokens_details/cached_tokens",
        ] {
            let (parent, key) = pointer.rsplit_once('/').unwrap();
            let body = with(|v| {
                v.pointer_mut(parent)
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
            });
            assert_eq!(stop(&body), UsageStop::RequiredFieldMissing, "{pointer}");
        }
    }

    #[test]
    fn a_ttl_split_that_does_not_add_up_stops_and_is_not_recorded() {
        for (total, ttl_30m, ttl_12h) in [(200, 150, 49), (200, 150, 51), (200, 0, 0), (0, 1, 0)] {
            let body = with(|v| {
                let details = &mut v["usage"]["input_tokens_details"];
                details["cache_write_tokens"] = json!(total);
                details["cache_write_tokens_details"] =
                    json!({"ttl_30m_tokens": ttl_30m, "ttl_12h_tokens": ttl_12h});
            });
            let reading = read_responses_usage(&body);
            assert_eq!(
                reading.wallet,
                Err(UsageStop::CacheWriteSplitMismatch),
                "{total} {ttl_30m} {ttl_12h}"
            );
            assert_eq!(reading.cache_write, None, "{total} {ttl_30m} {ttl_12h}");
        }
        // A split with no total.
        let body = with(|v| {
            v["usage"]["input_tokens_details"]
                .as_object_mut()
                .unwrap()
                .remove("cache_write_tokens");
        });
        let reading = read_responses_usage(&body);
        assert_eq!(reading.wallet, Err(UsageStop::CacheWriteSplitMismatch));
        assert_eq!(reading.cache_write, None);
        // One TTL past the total.
        let body = with(|v| {
            v["usage"]["input_tokens_details"]["cache_write_tokens_details"] =
                json!({"ttl_30m_tokens": 201});
        });
        let reading = read_responses_usage(&body);
        assert_eq!(reading.wallet, Err(UsageStop::CacheWriteSplitMismatch));
        assert_eq!(reading.cache_write, None);
        // A sum past counting.
        let body = complete()
            .to_string()
            .replace(
                r#""ttl_30m_tokens":150"#,
                &format!(r#""ttl_30m_tokens":{}"#, u64::MAX),
            )
            .replace(
                r#""ttl_12h_tokens":50"#,
                &format!(r#""ttl_12h_tokens":{}"#, u64::MAX),
            );
        assert_eq!(stop(&body), UsageStop::CacheWriteSplitMismatch);
    }

    #[test]
    fn a_total_with_one_ttl_or_none_is_a_total_with_no_split() {
        let body = with(|v| {
            v["usage"]["input_tokens_details"]["cache_write_tokens_details"] =
                json!({"ttl_30m_tokens": 150});
        });
        let reading = read_responses_usage(&body);
        let usage = reading.wallet.unwrap();
        assert_eq!(usage.cache_write_30m_tokens, Some(150));
        assert_eq!(usage.cache_write_12h_tokens, None);
        assert_eq!(reading.cache_write, observed(200, None));
        let body = with(|v| {
            v["usage"]["input_tokens_details"]
                .as_object_mut()
                .unwrap()
                .remove("cache_write_tokens_details");
        });
        assert_eq!(read_responses_usage(&body).cache_write, observed(200, None));
    }

    #[test]
    fn an_explicit_zero_write_is_kept_and_both_ttls_may_be_written() {
        let body = with(|v| {
            v["usage"]["input_tokens_details"]["cache_write_tokens"] = json!(0);
            v["usage"]["input_tokens_details"]["cache_write_tokens_details"] =
                json!({"ttl_30m_tokens": 0, "ttl_12h_tokens": 0});
        });
        assert_eq!(
            read_responses_usage(&body).cache_write,
            observed(0, Some((0, 0)))
        );
        // Written for both TTLs in one response.
        assert_eq!(
            read_responses_usage(&complete().to_string()).cache_write,
            observed(200, Some((150, 50)))
        );
    }

    #[test]
    fn cached_and_written_input_cannot_pass_the_input() {
        // 300 cached + 200 written = 500: exactly the input passes.
        let body = with(|v| v["usage"]["input_tokens"] = json!(500));
        let body = body.replace(r#""total_tokens":1080"#, r#""total_tokens":580"#);
        assert!(read_responses_usage(&body).wallet.is_ok());
        let body = with(|v| v["usage"]["input_tokens"] = json!(499));
        let body = body.replace(r#""total_tokens":1080"#, r#""total_tokens":579"#);
        let reading = read_responses_usage(&body);
        assert_eq!(reading.wallet, Err(UsageStop::CachedAndWrittenOverInput));
        assert_eq!(reading.cache_write, None);
        // Cached alone past the input.
        let body = json!({"usage": {
            "input_tokens": 5,
            "input_tokens_details": {"cached_tokens": 6},
            "output_tokens": 1
        }})
        .to_string();
        assert_eq!(stop(&body), UsageStop::CachedAndWrittenOverInput);
        // Past counting.
        let body = complete().to_string().replace(
            r#""cached_tokens":300"#,
            &format!(r#""cached_tokens":{}"#, u64::MAX),
        );
        assert_eq!(stop(&body), UsageStop::CachedAndWrittenOverInput);
    }

    #[test]
    fn reasoning_cannot_pass_the_output() {
        let body = with(|v| v["usage"]["output_tokens_details"]["reasoning_tokens"] = json!(80));
        assert!(read_responses_usage(&body).wallet.is_ok());
        let body = with(|v| v["usage"]["output_tokens_details"]["reasoning_tokens"] = json!(81));
        let reading = read_responses_usage(&body);
        assert_eq!(reading.wallet, Err(UsageStop::ReasoningOverOutput));
        // Not a cache-write matter.
        assert_eq!(reading.cache_write, observed(200, Some((150, 50))));
        // No reasoning at all, and an empty details object, are both no reasoning.
        let body = with(|v| v["usage"]["output_tokens_details"] = json!({}));
        assert_eq!(
            read_responses_usage(&body).wallet.unwrap().reasoning_tokens,
            None
        );
    }

    #[test]
    fn a_total_that_is_not_input_plus_output_stops() {
        for total in [1079, 1081, 0] {
            let body = with(|v| v["usage"]["total_tokens"] = json!(total));
            assert_eq!(stop(&body), UsageStop::TotalMismatch, "{total}");
        }
        let body = with(|v| {
            v["usage"].as_object_mut().unwrap().remove("total_tokens");
        });
        assert_eq!(
            read_responses_usage(&body).wallet.unwrap().total_tokens,
            None
        );
    }

    #[test]
    fn an_all_zero_usage_is_read_as_reported() {
        // Whether a zero input may be charged is the tariff's rule; the reading keeps the zeros.
        let body = json!({"usage": {
            "input_tokens": 0,
            "input_tokens_details": {
                "cached_tokens": 0,
                "cache_write_tokens": 0,
                "cache_write_tokens_details": {"ttl_30m_tokens": 0, "ttl_12h_tokens": 0}
            },
            "output_tokens": 0,
            "output_tokens_details": {"reasoning_tokens": 0},
            "total_tokens": 0
        }})
        .to_string();
        let reading = read_responses_usage(&body);
        let usage = reading.wallet.unwrap();
        assert_eq!((usage.input_tokens, usage.output_tokens), (0, 0));
        assert_eq!(usage.cache_write_tokens, Some(0));
        assert_eq!(reading.cache_write, observed(0, Some((0, 0))));
    }

    #[test]
    fn the_stop_words_are_fixed() {
        for (stop, word) in [
            (UsageStop::ResponseMalformed, "response_malformed"),
            (UsageStop::UsageMissing, "usage_missing"),
            (UsageStop::UsageNotObject, "usage_not_object"),
            (UsageStop::DuplicateKey, "duplicate_key"),
            (UsageStop::UnknownField, "unknown_field"),
            (UsageStop::InvalidValue, "invalid_value"),
            (UsageStop::RequiredFieldMissing, "required_field_missing"),
            (
                UsageStop::CacheWriteSplitMismatch,
                "cache_write_split_mismatch",
            ),
            (
                UsageStop::CachedAndWrittenOverInput,
                "cached_and_written_over_input",
            ),
            (UsageStop::ReasoningOverOutput, "reasoning_over_output"),
            (UsageStop::TotalMismatch, "total_mismatch"),
        ] {
            assert_eq!(stop.as_str(), word);
            assert_eq!(serde_json::to_value(stop).unwrap(), json!(word));
        }
    }

    fn chat(details: Value) -> String {
        json!({
            "choices": [],
            "usage": {
                "prompt_tokens": 1000,
                "completion_tokens": 10,
                "cost": 0.0123,
                "is_byok": false,
                "prompt_tokens_details": details
            }
        })
        .to_string()
    }

    #[test]
    fn a_chat_cache_write_is_read_at_the_table_paths_only() {
        let body = chat(json!({
            "cached_tokens": 100,
            "cache_write_tokens": 40,
            "cache_write_tokens_details": {"ttl_30m_tokens": 30, "ttl_12h_tokens": 10},
            "audio_tokens": 3
        }));
        assert_eq!(
            observe_chat_cache_write(&body),
            observed(40, Some((30, 10)))
        );
        let body = chat(json!({"cached_tokens": 100, "cache_write_tokens": 40}));
        assert_eq!(observe_chat_cache_write(&body), observed(40, None));
        let body = chat(json!({"cached_tokens": 100, "cache_write_tokens": 0}));
        assert_eq!(observe_chat_cache_write(&body), observed(0, None));
        // Absent, or another provider's name for it, is not observed.
        for details in [
            json!({"cached_tokens": 100}),
            json!({"cache_creation_input_tokens": 40}),
            json!({"cache_write_input_tokens": 40}),
            json!({"cached_tokens": 100, "write_tokens": 40}),
        ] {
            assert_eq!(
                observe_chat_cache_write(&chat(details.clone())),
                None,
                "{details}"
            );
        }
        // Not under prompt_tokens_details, not at the Responses path, not at the top.
        for body in [
            json!({"usage": {"prompt_tokens": 10, "cache_write_tokens": 4}}),
            json!({"usage": {"prompt_tokens": 10, "input_tokens_details": {"cache_write_tokens": 4}}}),
            json!({"cache_write_tokens": 4, "usage": {"prompt_tokens": 10}}),
            json!({"usage": {"prompt_tokens": 10, "completion_tokens_details": {"cache_write_tokens": 4}}}),
        ] {
            assert_eq!(observe_chat_cache_write(&body.to_string()), None, "{body}");
        }
        // And a Responses body read as Chat, or a Chat body read as Responses, observes nothing.
        assert_eq!(observe_chat_cache_write(&complete().to_string()), None);
        let chat_body = chat(json!({"cached_tokens": 1, "cache_write_tokens": 4}));
        assert_eq!(read_responses_usage(&chat_body).cache_write, None);
    }

    #[test]
    fn a_chat_cache_write_that_is_not_valid_is_not_observed() {
        for details in [
            json!({"cache_write_tokens": null}),
            json!({"cache_write_tokens": -1}),
            json!({"cache_write_tokens": 1.5}),
            json!({"cache_write_tokens": "40"}),
            json!({"cache_write_tokens": 40, "cache_write_tokens_details": {"ttl_30m_tokens": 30, "ttl_12h_tokens": 11}}),
            json!({"cache_write_tokens": 40, "cache_write_tokens_details": null}),
            json!({"cache_write_tokens_details": {"ttl_30m_tokens": 30, "ttl_12h_tokens": 10}}),
            json!({"cached_tokens": 961, "cache_write_tokens": 40}),
            json!({"cache_write_tokens": 1001}),
        ] {
            assert_eq!(
                observe_chat_cache_write(&chat(details.clone())),
                None,
                "{details}"
            );
        }
        // Exactly the input, cached and written, is valid.
        let body = chat(json!({"cached_tokens": 960, "cache_write_tokens": 40}));
        assert_eq!(observe_chat_cache_write(&body), observed(40, None));
        // A repeated key is not observed.
        let body = chat(json!({"cache_write_tokens": 40})).replace(
            r#""cache_write_tokens":40"#,
            r#""cache_write_tokens":4,"cache_write_tokens":40"#,
        );
        assert_eq!(observe_chat_cache_write(&body), None);
        // No usage, a body that is no object, details that are null: nothing.
        for body in [
            "{}",
            "[]",
            "",
            r#"{"usage":null}"#,
            r#"{"usage":{"prompt_tokens_details":null}}"#,
        ] {
            assert_eq!(observe_chat_cache_write(body), None, "{body}");
        }
    }

    #[test]
    fn each_reading_stands_alone() {
        // Nothing of one body is carried to the next.
        let first = read_responses_usage(&complete().to_string());
        assert!(first.cache_write.is_some());
        let second = read_responses_usage(
            r#"{"usage":{"input_tokens":1,"input_tokens_details":{"cached_tokens":0},"output_tokens":1}}"#,
        );
        assert_eq!(second.cache_write, None);
        assert_eq!(second.wallet.unwrap().cache_write_tokens, None);
    }
}
