//! The operator's declaration of one `usage_tariff` wallet (Issue #1006, design 1).
//!
//! `run-budget.policy/v2` is a separate protocol from v1, never a reading of it: a v1 file is the
//! wrong protocol here and a v2 file the wrong protocol there. It declares, and only it declares,
//! the one route a wallet of this basis allows (direct, non-streaming Responses), the prices of a
//! fixed tariff revision by input-length tier, and how a request's input is bounded. The product
//! holds no price of its own and branches on no model name.
//!
//! The decode is as strict as v1's: an unknown field, a repeated key, a value of the wrong type, a
//! zero bound, or a price that is not a decimal string refuses the whole file, in fixed words.
//!
//! What is declared and not checked here: whether the prices, the region and surcharge statement,
//! or `provider_max_input_tokens` match the operator's real contract. The host cannot prove those;
//! the wallet stops when an observed input passes the bound it reserved for.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer};

use super::policy::{Limits, valid_budget_id};
use super::reservation::Money;
use crate::planner::structured_stages::receipt::sha256_hex;

pub(crate) const POLICY_V2_PROTOCOL: &str = "run-budget.policy/v2";
pub(crate) const ACCOUNTING_BASIS_USAGE_TARIFF: &str = "usage_tariff";
pub(crate) const ROUTE_API_RESPONSES: &str = "responses";
/// The one tier this wallet prices: the standard one, named on the wire.
pub(crate) const SERVICE_TIER_DEFAULT: &str = "default";
/// A request with only text, calls, call outputs and tool schemas is bounded by its bytes and the
/// overhead; one with an opaque item (a reasoning item) by `provider_max_input_tokens`.
pub(crate) const INPUT_BOUND_METHOD: &str = "body_bytes_plus_overhead_or_provider_max";
const CACHE_WRITE_MODE_TTL: &str = "ttl_30m_12h";
const CACHE_WRITE_MODE_NONE: &str = "none";
const MAX_POLICY_BYTES: usize = 1 << 20;

/// Fixed words, safe for a message: a decoder's own text could quote the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyV2Refusal {
    TooLarge,
    /// Not one JSON object of exactly the declared fields, each once, each of its type.
    Malformed,
    WrongProtocol,
    WrongAccountingBasis,
    InvalidBudgetId,
    BudgetIdDiffers,
    InvalidAmount,
    UnsupportedApi,
    InvalidEndpoint,
    InvalidRoute,
    InvalidResponseModels,
    UnsupportedServiceTier,
    InvalidPriceConditions,
    InvalidTariff,
    InvalidCacheWriteMode,
    /// A write price where the mode has none, or none where it needs both.
    CacheWritePricesDiffer,
    InvalidTiers,
    InvalidInputBound,
    ZeroBound,
    InvalidLedgerDir,
    LimitTooLarge,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    protocol: String,
    accounting_basis: String,
    budget_id: String,
    limits: RawLimits,
    route: RawRoute,
    tariff: RawTariff,
    input_bound: RawInputBound,
    max_output_tokens: u64,
    max_input_tokens: u64,
    ledger_dir: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLimits {
    cost_usd: String,
    physical_sends: u64,
    logical_global: u64,
    logical_per_stage: u64,
    tokens: u64,
    wall_seconds: u64,
    final_repair_attempts: u64,
    final_repair_seconds: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoute {
    api: String,
    endpoint: String,
    model: String,
    response_models: Vec<String>,
    effort: String,
    service_tier: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTariff {
    revision: String,
    price_conditions: RawPriceConditions,
    cache_write_mode: String,
    tiers: Vec<RawTier>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPriceConditions {
    region: String,
    regional_surcharge: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTier {
    above_input_tokens: u64,
    input_per_million: String,
    cached_input_per_million: String,
    /// Absent is not there; `null` is not a price and is refused as a wrong type.
    #[serde(default, deserialize_with = "present")]
    cache_write_30m_per_million: Option<String>,
    #[serde(default, deserialize_with = "present")]
    cache_write_12h_per_million: Option<String>,
    output_per_million: String,
    request_fee: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInputBound {
    method: String,
    input_overhead: u64,
    #[serde(default, deserialize_with = "present")]
    provider_max_input_tokens: Option<u64>,
}

/// A field that is there has its own type: `null` is a wrong type, not an absent field.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// What the wallet's requests put on the wire, and the response models it accepts by exact name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResponsesRoute {
    pub(crate) api: String,
    /// The direct endpoint the transport must report it posted to.
    pub(crate) endpoint: String,
    pub(crate) model: String,
    /// Exact strings, each once, never a prefix or an alias.
    pub(crate) response_models: Vec<String>,
    pub(crate) effort: String,
    pub(crate) service_tier: String,
}

impl ResponsesRoute {
    /// Whether a response's model is one of the declared names, by exact match only.
    pub(crate) fn accepts_response_model(&self, model: &str) -> bool {
        self.response_models
            .iter()
            .any(|declared| declared == model)
    }
}

/// The operator's statement of where and how the prices apply. Declared, not verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PriceConditions {
    pub(crate) region: String,
    pub(crate) regional_surcharge: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheWriteMode {
    /// Cache writes are priced by TTL, and a response must report the total and both TTLs.
    Ttl30m12h,
    /// No write is priced, and a response may report no write, or only writes of zero.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheWriteRates {
    pub(crate) ttl_30m: Money,
    pub(crate) ttl_12h: Money,
}

/// The rates of one input-length tier, per million tokens. The tier applies to the whole request
/// whose input tokens (cached and written ones included) are past `above_input_tokens` and not past
/// the next tier's; the first tier also takes zero.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TierRates {
    pub(crate) above_input_tokens: u64,
    pub(crate) input: Money,
    pub(crate) cached_input: Money,
    /// `Some` in every tier of [`CacheWriteMode::Ttl30m12h`], `None` in every tier of
    /// [`CacheWriteMode::None`].
    pub(crate) cache_write: Option<CacheWriteRates>,
    pub(crate) output: Money,
    pub(crate) request_fee: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TariffV2 {
    pub(crate) revision: String,
    pub(crate) price_conditions: PriceConditions,
    pub(crate) cache_write_mode: CacheWriteMode,
    /// At least one, the first above zero tokens, each above more than the one before.
    pub(crate) tiers: Vec<TierRates>,
}

/// How a request's input is bounded before it is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputBound {
    pub(crate) input_overhead: u64,
    /// `None` when not declared: a request with an opaque item then cannot be bounded and is not
    /// sent. Never zero.
    pub(crate) provider_max_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunBudgetPolicyV2 {
    pub(crate) budget_id: String,
    /// Of the exact bytes admitted. A ledger of this basis holds a wallet to it.
    pub(crate) sha256: String,
    pub(crate) limits: Limits,
    pub(crate) route: ResponsesRoute,
    pub(crate) tariff: TariffV2,
    pub(crate) input_bound: InputBound,
    pub(crate) max_output_tokens: u64,
    pub(crate) max_input_tokens: u64,
    pub(crate) ledger_dir: PathBuf,
}

fn amount(text: &str) -> Result<Money, PolicyV2Refusal> {
    Money::parse_decimal_up(text).map_err(|_| PolicyV2Refusal::InvalidAmount)
}

fn milliseconds(seconds: u64) -> Result<u64, PolicyV2Refusal> {
    seconds
        .checked_mul(1000)
        .ok_or(PolicyV2Refusal::LimitTooLarge)
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn limits(raw: &RawLimits) -> Result<Limits, PolicyV2Refusal> {
    // As in v1: exactly at a limit passes, so a zero here would pass a first send.
    if raw.wall_seconds == 0 || raw.final_repair_seconds == 0 {
        return Err(PolicyV2Refusal::ZeroBound);
    }
    Ok(Limits {
        cost: amount(&raw.cost_usd)?,
        physical_sends: raw.physical_sends,
        logical_global: raw.logical_global,
        logical_per_stage: raw.logical_per_stage,
        tokens: raw.tokens,
        wall_ms: milliseconds(raw.wall_seconds)?,
        final_repair_attempts: raw.final_repair_attempts,
        final_repair_ms: milliseconds(raw.final_repair_seconds)?,
    })
}

/// An absolute `https` URL with a host and nothing a proxy or a redirect could hide behind.
fn endpoint(text: &str) -> Result<String, PolicyV2Refusal> {
    let url = url::Url::parse(text).map_err(|_| PolicyV2Refusal::InvalidEndpoint)?;
    if url.scheme() != "https"
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || text != url.as_str()
    {
        return Err(PolicyV2Refusal::InvalidEndpoint);
    }
    Ok(text.to_string())
}

fn route(raw: RawRoute) -> Result<ResponsesRoute, PolicyV2Refusal> {
    if raw.api != ROUTE_API_RESPONSES {
        return Err(PolicyV2Refusal::UnsupportedApi);
    }
    let endpoint = endpoint(&raw.endpoint)?;
    if blank(&raw.model) || blank(&raw.effort) {
        return Err(PolicyV2Refusal::InvalidRoute);
    }
    let mut unique = raw.response_models.iter().collect::<Vec<_>>();
    unique.sort();
    unique.dedup();
    if raw.response_models.is_empty()
        || unique.len() != raw.response_models.len()
        || raw.response_models.iter().any(|model| blank(model))
    {
        return Err(PolicyV2Refusal::InvalidResponseModels);
    }
    if raw.service_tier != SERVICE_TIER_DEFAULT {
        return Err(PolicyV2Refusal::UnsupportedServiceTier);
    }
    Ok(ResponsesRoute {
        api: raw.api,
        endpoint,
        model: raw.model,
        response_models: raw.response_models,
        effort: raw.effort,
        service_tier: raw.service_tier,
    })
}

fn cache_write_mode(text: &str) -> Result<CacheWriteMode, PolicyV2Refusal> {
    match text {
        CACHE_WRITE_MODE_TTL => Ok(CacheWriteMode::Ttl30m12h),
        CACHE_WRITE_MODE_NONE => Ok(CacheWriteMode::None),
        _ => Err(PolicyV2Refusal::InvalidCacheWriteMode),
    }
}

fn tier(raw: RawTier, mode: CacheWriteMode) -> Result<TierRates, PolicyV2Refusal> {
    let input = amount(&raw.input_per_million)?;
    let output = amount(&raw.output_per_million)?;
    // As in v1: with these at zero the money limit would stop nothing.
    if input == Money::ZERO || output == Money::ZERO {
        return Err(PolicyV2Refusal::InvalidTariff);
    }
    let cache_write = match (
        mode,
        raw.cache_write_30m_per_million,
        raw.cache_write_12h_per_million,
    ) {
        (CacheWriteMode::Ttl30m12h, Some(ttl_30m), Some(ttl_12h)) => Some(CacheWriteRates {
            ttl_30m: amount(&ttl_30m)?,
            ttl_12h: amount(&ttl_12h)?,
        }),
        (CacheWriteMode::None, None, None) => None,
        _ => return Err(PolicyV2Refusal::CacheWritePricesDiffer),
    };
    Ok(TierRates {
        above_input_tokens: raw.above_input_tokens,
        input,
        cached_input: amount(&raw.cached_input_per_million)?,
        cache_write,
        output,
        request_fee: amount(&raw.request_fee)?,
    })
}

fn tariff(raw: RawTariff) -> Result<TariffV2, PolicyV2Refusal> {
    if blank(&raw.revision) {
        return Err(PolicyV2Refusal::InvalidTariff);
    }
    if blank(&raw.price_conditions.region) || blank(&raw.price_conditions.regional_surcharge) {
        return Err(PolicyV2Refusal::InvalidPriceConditions);
    }
    let mode = cache_write_mode(&raw.cache_write_mode)?;
    let starts_at_zero = raw
        .tiers
        .first()
        .is_some_and(|first| first.above_input_tokens == 0);
    let increasing = raw
        .tiers
        .windows(2)
        .all(|pair| pair[0].above_input_tokens < pair[1].above_input_tokens);
    if !starts_at_zero || !increasing {
        return Err(PolicyV2Refusal::InvalidTiers);
    }
    let tiers = raw
        .tiers
        .into_iter()
        .map(|raw| tier(raw, mode))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TariffV2 {
        revision: raw.revision,
        price_conditions: PriceConditions {
            region: raw.price_conditions.region,
            regional_surcharge: raw.price_conditions.regional_surcharge,
        },
        cache_write_mode: mode,
        tiers,
    })
}

fn input_bound(raw: &RawInputBound) -> Result<InputBound, PolicyV2Refusal> {
    if raw.method != INPUT_BOUND_METHOD {
        return Err(PolicyV2Refusal::InvalidInputBound);
    }
    if raw.provider_max_input_tokens == Some(0) {
        return Err(PolicyV2Refusal::ZeroBound);
    }
    Ok(InputBound {
        input_overhead: raw.input_overhead,
        provider_max_input_tokens: raw.provider_max_input_tokens,
    })
}

fn ledger_dir(text: &str) -> Result<PathBuf, PolicyV2Refusal> {
    let path = Path::new(text);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PolicyV2Refusal::InvalidLedgerDir);
    }
    Ok(path.to_path_buf())
}

/// Whether a declaration names this protocol. Only the one field is read, leniently, to choose
/// the decoder; the decoder then reads the whole file as strictly as ever, so a file that only
/// looks like v2 here is still refused there.
pub(crate) fn names_protocol_v2(bytes: &[u8]) -> bool {
    #[derive(Deserialize)]
    struct Named {
        protocol: String,
    }
    bytes.len() <= MAX_POLICY_BYTES
        && serde_json::from_slice::<Named>(bytes)
            .is_ok_and(|named| named.protocol == POLICY_V2_PROTOCOL)
}

impl RunBudgetPolicyV2 {
    /// The ID is the one the entry was asked to use: a file copied under another name is refused.
    pub(crate) fn decode(bytes: &[u8], expected_id: &str) -> Result<Self, PolicyV2Refusal> {
        if bytes.len() > MAX_POLICY_BYTES {
            return Err(PolicyV2Refusal::TooLarge);
        }
        if !valid_budget_id(expected_id) {
            return Err(PolicyV2Refusal::InvalidBudgetId);
        }
        let raw: RawPolicy =
            serde_json::from_slice(bytes).map_err(|_| PolicyV2Refusal::Malformed)?;
        if raw.protocol != POLICY_V2_PROTOCOL {
            return Err(PolicyV2Refusal::WrongProtocol);
        }
        if raw.accounting_basis != ACCOUNTING_BASIS_USAGE_TARIFF {
            return Err(PolicyV2Refusal::WrongAccountingBasis);
        }
        if raw.budget_id != expected_id {
            return Err(PolicyV2Refusal::BudgetIdDiffers);
        }
        if raw.max_output_tokens == 0 || raw.max_input_tokens == 0 {
            return Err(PolicyV2Refusal::ZeroBound);
        }
        Ok(Self {
            sha256: sha256_hex(bytes),
            limits: limits(&raw.limits)?,
            route: route(raw.route)?,
            tariff: tariff(raw.tariff)?,
            input_bound: input_bound(&raw.input_bound)?,
            ledger_dir: ledger_dir(&raw.ledger_dir)?,
            budget_id: raw.budget_id,
            max_output_tokens: raw.max_output_tokens,
            max_input_tokens: raw.max_input_tokens,
        })
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use serde_json::{Value, json};

    use super::RunBudgetPolicyV2;

    pub(crate) const WALLET: &str = "wallet-v2";

    /// Two tiers split at 272000 input tokens, writes priced by TTL, all prices synthetic.
    pub(crate) fn policy_v2_value() -> Value {
        json!({
            "protocol": "run-budget.policy/v2",
            "accounting_basis": "usage_tariff",
            "budget_id": WALLET,
            "limits": {
                "cost_usd": "1000",
                "physical_sends": 1000,
                "logical_global": 1000,
                "logical_per_stage": 1000,
                "tokens": 10_000_000,
                "wall_seconds": 100_000,
                "final_repair_attempts": 1000,
                "final_repair_seconds": 100_000
            },
            "route": {
                "api": "responses",
                "endpoint": "https://api.example.test/v1/responses",
                "model": "m-1",
                "response_models": ["m-1-snapshot-a", "m-1-snapshot-b"],
                "effort": "high",
                "service_tier": "default"
            },
            "tariff": {
                "revision": "synthetic-2026-11",
                "price_conditions": {"region": "global", "regional_surcharge": "none"},
                "cache_write_mode": "ttl_30m_12h",
                "tiers": [
                    {
                        "above_input_tokens": 0,
                        "input_per_million": "2",
                        "cached_input_per_million": "0.2",
                        "cache_write_30m_per_million": "2.5",
                        "cache_write_12h_per_million": "4",
                        "output_per_million": "8",
                        "request_fee": "0"
                    },
                    {
                        "above_input_tokens": 272_000,
                        "input_per_million": "4",
                        "cached_input_per_million": "0.4",
                        "cache_write_30m_per_million": "5",
                        "cache_write_12h_per_million": "8",
                        "output_per_million": "12",
                        "request_fee": "0"
                    }
                ]
            },
            "input_bound": {
                "method": "body_bytes_plus_overhead_or_provider_max",
                "input_overhead": 4,
                "provider_max_input_tokens": 1_050_000
            },
            "max_output_tokens": 50,
            "max_input_tokens": 2_000_000,
            "ledger_dir": "/operator/ledgers"
        })
    }

    /// [`policy_v2_value`] with no write prices and `cache_write_mode: none`.
    pub(crate) fn policy_v2_none_value() -> Value {
        let mut value = policy_v2_value();
        value["tariff"]["cache_write_mode"] = json!("none");
        for tier in value["tariff"]["tiers"].as_array_mut().unwrap() {
            let tier = tier.as_object_mut().unwrap();
            tier.remove("cache_write_30m_per_million");
            tier.remove("cache_write_12h_per_million");
        }
        value
    }

    pub(crate) fn decode_v2(value: &Value) -> RunBudgetPolicyV2 {
        RunBudgetPolicyV2::decode(value.to_string().as_bytes(), WALLET).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::reservation::Money;
    use super::test_support::{WALLET, decode_v2, policy_v2_none_value, policy_v2_value};
    use super::{CacheWriteMode, PolicyV2Refusal, RunBudgetPolicyV2};

    fn decoded(value: &Value) -> Result<RunBudgetPolicyV2, PolicyV2Refusal> {
        RunBudgetPolicyV2::decode(value.to_string().as_bytes(), WALLET)
    }

    fn refused(change: impl FnOnce(&mut Value)) -> PolicyV2Refusal {
        let mut value = policy_v2_value();
        change(&mut value);
        decoded(&value).unwrap_err()
    }

    fn refused_none(change: impl FnOnce(&mut Value)) -> PolicyV2Refusal {
        let mut value = policy_v2_none_value();
        change(&mut value);
        decoded(&value).unwrap_err()
    }

    fn money(text: &str) -> Money {
        Money::parse_decimal_up(text).unwrap()
    }

    /// Every object path of a value, the root as `[]`.
    fn object_paths(value: &Value, at: Vec<String>, out: &mut Vec<Vec<String>>) {
        match value {
            Value::Object(map) => {
                out.push(at.clone());
                for (key, child) in map {
                    let mut next = at.clone();
                    next.push(key.clone());
                    object_paths(child, next, out);
                }
            }
            Value::Array(items) => {
                for (index, child) in items.iter().enumerate() {
                    let mut next = at.clone();
                    next.push(index.to_string());
                    object_paths(child, next, out);
                }
            }
            _ => {}
        }
    }

    fn at<'a>(value: &'a mut Value, path: &[String]) -> &'a mut Value {
        path.iter().fold(value, |node, key| match node {
            Value::Array(items) => &mut items[key.parse::<usize>().unwrap()],
            other => &mut other[key.as_str()],
        })
    }

    #[test]
    fn a_complete_policy_decodes_with_its_exact_values() {
        let policy = decode_v2(&policy_v2_value());
        assert_eq!(policy.budget_id, WALLET);
        assert_eq!(policy.sha256.len(), 64);
        assert_eq!(policy.limits.cost, money("1000"));
        assert_eq!(policy.limits.wall_ms, 100_000_000);
        assert_eq!(policy.limits.final_repair_ms, 100_000_000);
        assert_eq!(policy.route.api, "responses");
        assert_eq!(
            policy.route.endpoint,
            "https://api.example.test/v1/responses"
        );
        assert_eq!(policy.route.model, "m-1");
        assert_eq!(
            policy.route.response_models,
            ["m-1-snapshot-a", "m-1-snapshot-b"]
        );
        assert_eq!(policy.route.effort, "high");
        assert_eq!(policy.route.service_tier, "default");
        assert_eq!(policy.tariff.revision, "synthetic-2026-11");
        assert_eq!(policy.tariff.price_conditions.region, "global");
        assert_eq!(policy.tariff.price_conditions.regional_surcharge, "none");
        assert_eq!(policy.tariff.cache_write_mode, CacheWriteMode::Ttl30m12h);
        assert_eq!(policy.tariff.tiers.len(), 2);
        let high = &policy.tariff.tiers[1];
        assert_eq!(high.above_input_tokens, 272_000);
        assert_eq!(high.input, money("4"));
        assert_eq!(high.cached_input, money("0.4"));
        let write = high.cache_write.unwrap();
        assert_eq!(write.ttl_30m, money("5"));
        assert_eq!(write.ttl_12h, money("8"));
        assert_eq!(high.output, money("12"));
        assert_eq!(policy.input_bound.input_overhead, 4);
        assert_eq!(
            policy.input_bound.provider_max_input_tokens,
            Some(1_050_000)
        );
        assert_eq!(policy.max_output_tokens, 50);
        assert_eq!(policy.max_input_tokens, 2_000_000);
        assert_eq!(policy.ledger_dir, std::path::Path::new("/operator/ledgers"));
    }

    #[test]
    fn the_digest_is_of_the_exact_bytes() {
        let bytes = policy_v2_value().to_string();
        let one = RunBudgetPolicyV2::decode(bytes.as_bytes(), WALLET).unwrap();
        let again = RunBudgetPolicyV2::decode(bytes.as_bytes(), WALLET).unwrap();
        let spaced = RunBudgetPolicyV2::decode(format!("{bytes} ").as_bytes(), WALLET).unwrap();
        assert_eq!(one.sha256, again.sha256);
        assert_ne!(one.sha256, spaced.sha256);
        assert_eq!(
            one.sha256,
            crate::planner::structured_stages::receipt::sha256_hex(bytes.as_bytes())
        );
    }

    #[test]
    fn a_none_mode_policy_decodes_without_write_prices() {
        let policy = decode_v2(&policy_v2_none_value());
        assert_eq!(policy.tariff.cache_write_mode, CacheWriteMode::None);
        assert!(policy.tariff.tiers.iter().all(|t| t.cache_write.is_none()));
    }

    #[test]
    fn every_missing_field_is_refused_at_every_level() {
        let value = policy_v2_value();
        let mut paths = Vec::new();
        object_paths(&value, Vec::new(), &mut paths);
        let mut checked = 0;
        for path in paths {
            let keys = at(&mut value.clone(), &path)
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            for key in keys {
                // The one optional field: its absence is a declaration that it is not there.
                if key == "provider_max_input_tokens" {
                    continue;
                }
                let refusal = refused(|v| {
                    at(v, &path).as_object_mut().unwrap().remove(&key);
                });
                // In ttl mode a missing write price is the mode's refusal, not a shape's.
                let expected = if key.starts_with("cache_write_") && key.ends_with("_per_million") {
                    PolicyV2Refusal::CacheWritePricesDiffer
                } else {
                    PolicyV2Refusal::Malformed
                };
                assert_eq!(refusal, expected, "{path:?}.{key}");
                checked += 1;
            }
        }
        // 10 top-level, 8 limits, 6 route, 4 tariff, 2 conditions, 7 + 7 tiers, 2 input bound.
        assert_eq!(checked, 46);
    }

    #[test]
    fn an_unknown_field_is_refused_at_every_level() {
        let value = policy_v2_value();
        let mut paths = Vec::new();
        object_paths(&value, Vec::new(), &mut paths);
        assert_eq!(paths.len(), 8);
        for path in paths {
            assert_eq!(
                refused(|v| {
                    at(v, &path)
                        .as_object_mut()
                        .unwrap()
                        .insert("extra".into(), json!(0));
                }),
                PolicyV2Refusal::Malformed,
                "{path:?}"
            );
        }
    }

    #[test]
    fn a_repeated_key_is_refused_at_every_level() {
        let text = policy_v2_value().to_string();
        for (once, twice) in [
            (
                r#""protocol":"run-budget.policy/v2""#,
                r#""protocol":"run-budget.policy/v2","protocol":"run-budget.policy/v2""#,
            ),
            (
                r#""cost_usd":"1000""#,
                r#""cost_usd":"1000","cost_usd":"1""#,
            ),
            (r#""model":"m-1""#, r#""model":"m-1","model":"m-1""#),
            (
                r#""region":"global""#,
                r#""region":"global","region":"global""#,
            ),
            (
                r#""above_input_tokens":272000"#,
                r#""above_input_tokens":272000,"above_input_tokens":272000"#,
            ),
            (
                r#""input_overhead":4"#,
                r#""input_overhead":4,"input_overhead":4"#,
            ),
            (
                r#""cache_write_mode":"ttl_30m_12h""#,
                r#""cache_write_mode":"ttl_30m_12h","cache_write_mode":"ttl_30m_12h""#,
            ),
        ] {
            assert_eq!(text.matches(once).count(), 1, "{once}");
            let doubled = text.replace(once, twice);
            assert_eq!(
                RunBudgetPolicyV2::decode(doubled.as_bytes(), WALLET),
                Err(PolicyV2Refusal::Malformed),
                "{twice}"
            );
        }
        // A repeated write price in a tier is a repeated key too, not a second price.
        let doubled = text.replacen(
            r#""cache_write_30m_per_million":"2.5""#,
            r#""cache_write_30m_per_million":"2.5","cache_write_30m_per_million":"2.5""#,
            1,
        );
        assert_eq!(
            RunBudgetPolicyV2::decode(doubled.as_bytes(), WALLET),
            Err(PolicyV2Refusal::Malformed)
        );
    }

    #[test]
    fn not_json_and_an_oversized_file_are_refused() {
        assert_eq!(
            RunBudgetPolicyV2::decode(b"not json", WALLET),
            Err(PolicyV2Refusal::Malformed)
        );
        assert_eq!(
            RunBudgetPolicyV2::decode(b"[]", WALLET),
            Err(PolicyV2Refusal::Malformed)
        );
        let text = policy_v2_value().to_string();
        let exact = format!("{text}{}", " ".repeat((1 << 20) - text.len()));
        assert!(RunBudgetPolicyV2::decode(exact.as_bytes(), WALLET).is_ok());
        assert_eq!(
            RunBudgetPolicyV2::decode(format!("{exact} ").as_bytes(), WALLET),
            Err(PolicyV2Refusal::TooLarge)
        );
    }

    #[test]
    fn a_v1_policy_or_another_protocol_is_refused() {
        for protocol in ["run-budget.policy/v1", "run-budget.policy/v3", ""] {
            assert_eq!(
                refused(|v| v["protocol"] = json!(protocol)),
                PolicyV2Refusal::WrongProtocol,
                "{protocol}"
            );
        }
        // A whole v1 file is not a v2 one, and does not become one.
        let v1 = super::super::test_support::policy_value();
        assert!(RunBudgetPolicyV2::decode(v1.to_string().as_bytes(), "wallet-alpha").is_err());
        // And a v2 file is not a v1 one.
        assert!(
            super::super::policy::RunBudgetPolicy::decode(
                policy_v2_value().to_string().as_bytes(),
                WALLET
            )
            .is_err()
        );
    }

    #[test]
    fn the_accounting_basis_must_be_usage_tariff() {
        for basis in ["provider_billed", "Usage_Tariff", "usage_tariff ", ""] {
            assert_eq!(
                refused(|v| v["accounting_basis"] = json!(basis)),
                PolicyV2Refusal::WrongAccountingBasis,
                "{basis}"
            );
        }
        assert_eq!(
            refused(|v| v["accounting_basis"] = json!(null)),
            PolicyV2Refusal::Malformed
        );
    }

    #[test]
    fn a_wrong_type_is_refused() {
        for (pointer, bad) in [
            ("/limits/physical_sends", json!(-1)),
            ("/limits/physical_sends", json!(1.5)),
            ("/limits/physical_sends", json!(1.0)),
            ("/limits/physical_sends", json!("1")),
            ("/limits/cost_usd", json!(1)),
            ("/max_output_tokens", json!(null)),
            ("/input_bound/input_overhead", json!(-4)),
            ("/input_bound/provider_max_input_tokens", json!(null)),
            ("/input_bound/provider_max_input_tokens", json!("1050000")),
            ("/input_bound/provider_max_input_tokens", json!(1.5)),
            ("/route/response_models", json!("m-1")),
            ("/route/response_models", json!([1])),
            ("/tariff/tiers", json!({})),
            ("/tariff/tiers/0/above_input_tokens", json!(-1)),
            ("/tariff/tiers/0/input_per_million", json!(2)),
            ("/tariff/tiers/0/cache_write_30m_per_million", json!(null)),
            ("/tariff/tiers/1/cache_write_12h_per_million", json!(8)),
        ] {
            assert_eq!(
                refused(|v| *v.pointer_mut(pointer).unwrap() = bad.clone()),
                PolicyV2Refusal::Malformed,
                "{pointer} {bad}"
            );
        }
    }

    #[test]
    fn a_price_must_be_a_decimal_string() {
        for bad in ["-1", "abc", "", "1e99999", "1.", ".5", "01"] {
            for pointer in [
                "/limits/cost_usd",
                "/tariff/tiers/0/request_fee",
                "/tariff/tiers/1/cached_input_per_million",
                "/tariff/tiers/1/cache_write_12h_per_million",
            ] {
                assert_eq!(
                    refused(|v| *v.pointer_mut(pointer).unwrap() = json!(bad)),
                    PolicyV2Refusal::InvalidAmount,
                    "{pointer} {bad}"
                );
            }
        }
    }

    #[test]
    fn a_zero_input_or_output_rate_is_refused_in_every_tier() {
        for tier in ["0", "1"] {
            for key in ["input_per_million", "output_per_million"] {
                for zero in ["0", "0.0", "0e5"] {
                    assert_eq!(
                        refused(
                            |v| v["tariff"]["tiers"][tier.parse::<usize>().unwrap()][key] =
                                json!(zero)
                        ),
                        PolicyV2Refusal::InvalidTariff,
                        "{tier} {key} {zero}"
                    );
                }
            }
        }
        // A tiny rate is not zero, and free cache reads, free writes and no fee are allowed.
        let mut value = policy_v2_value();
        let tier = &mut value["tariff"]["tiers"][0];
        tier["input_per_million"] = json!("1e-400");
        tier["cached_input_per_million"] = json!("0");
        tier["cache_write_30m_per_million"] = json!("0");
        tier["cache_write_12h_per_million"] = json!("0");
        let policy = decode_v2(&value);
        assert_eq!(policy.tariff.tiers[0].input.units(), 1);
    }

    #[test]
    fn an_empty_revision_or_price_condition_is_refused() {
        assert_eq!(
            refused(|v| v["tariff"]["revision"] = json!(" ")),
            PolicyV2Refusal::InvalidTariff
        );
        for key in ["region", "regional_surcharge"] {
            assert_eq!(
                refused(|v| v["tariff"]["price_conditions"][key] = json!("")),
                PolicyV2Refusal::InvalidPriceConditions,
                "{key}"
            );
        }
    }

    #[test]
    fn the_cache_write_mode_is_one_of_two_spelled_exactly() {
        for mode in ["", "ttl", "TTL_30M_12H", "None", "auto", "ttl_30m"] {
            assert_eq!(
                refused(|v| v["tariff"]["cache_write_mode"] = json!(mode)),
                PolicyV2Refusal::InvalidCacheWriteMode,
                "{mode}"
            );
        }
        assert_eq!(
            refused(|v| {
                v["tariff"]
                    .as_object_mut()
                    .unwrap()
                    .remove("cache_write_mode");
            }),
            PolicyV2Refusal::Malformed
        );
    }

    #[test]
    fn ttl_mode_needs_both_write_prices_in_every_tier() {
        for tier in [0, 1] {
            for key in ["cache_write_30m_per_million", "cache_write_12h_per_million"] {
                assert_eq!(
                    refused(|v| {
                        v["tariff"]["tiers"][tier]
                            .as_object_mut()
                            .unwrap()
                            .remove(key);
                    }),
                    PolicyV2Refusal::CacheWritePricesDiffer,
                    "{tier} {key}"
                );
            }
        }
    }

    #[test]
    fn none_mode_carries_no_write_price_in_any_tier() {
        for tier in [0, 1] {
            for key in ["cache_write_30m_per_million", "cache_write_12h_per_million"] {
                for price in ["0", "1"] {
                    assert_eq!(
                        refused_none(|v| v["tariff"]["tiers"][tier][key] = json!(price)),
                        PolicyV2Refusal::CacheWritePricesDiffer,
                        "{tier} {key} {price}"
                    );
                }
                assert_eq!(
                    refused_none(|v| v["tariff"]["tiers"][tier][key] = json!(null)),
                    PolicyV2Refusal::Malformed,
                    "{tier} {key} null"
                );
            }
        }
    }

    #[test]
    fn tiers_start_at_zero_and_rise_strictly() {
        assert_eq!(
            refused(|v| v["tariff"]["tiers"] = json!([])),
            PolicyV2Refusal::InvalidTiers
        );
        assert_eq!(
            refused(|v| v["tariff"]["tiers"][0]["above_input_tokens"] = json!(1)),
            PolicyV2Refusal::InvalidTiers
        );
        // A second tier above as many tokens as the first is not above it.
        assert_eq!(
            refused(|v| v["tariff"]["tiers"][1]["above_input_tokens"] = json!(0)),
            PolicyV2Refusal::InvalidTiers
        );
        // Out of order.
        assert_eq!(
            refused(|v| {
                let tiers = v["tariff"]["tiers"].as_array_mut().unwrap();
                tiers.swap(0, 1);
            }),
            PolicyV2Refusal::InvalidTiers
        );
        // One tier is a tariff with no length price.
        let mut value = policy_v2_value();
        value["tariff"]["tiers"].as_array_mut().unwrap().truncate(1);
        assert_eq!(decode_v2(&value).tariff.tiers.len(), 1);
    }

    #[test]
    fn the_route_is_direct_responses_at_the_default_tier() {
        for api in ["chat_completions", "Responses", ""] {
            assert_eq!(
                refused(|v| v["route"]["api"] = json!(api)),
                PolicyV2Refusal::UnsupportedApi,
                "{api}"
            );
        }
        for tier in ["auto", "flex", "priority", "batch", "", "Default"] {
            assert_eq!(
                refused(|v| v["route"]["service_tier"] = json!(tier)),
                PolicyV2Refusal::UnsupportedServiceTier,
                "{tier}"
            );
        }
        for key in ["model", "effort"] {
            assert_eq!(
                refused(|v| v["route"][key] = json!(" ")),
                PolicyV2Refusal::InvalidRoute,
                "{key}"
            );
        }
    }

    /// F5: each part of the endpoint rule alone refuses it: a user with no password, a password
    /// with no user, and a plain http scheme, each with everything else of the URL as allowed.
    #[test]
    fn each_part_of_the_endpoint_rule_refuses_it_alone() {
        for bad in [
            "https://user@api.example.test/v1/responses",
            "https://:pw@api.example.test/v1/responses",
            "https://user:pw@api.example.test/v1/responses",
            "http://api.example.test/v1/responses",
        ] {
            assert_eq!(
                refused(|v| v["route"]["endpoint"] = json!(bad)),
                PolicyV2Refusal::InvalidEndpoint,
                "{bad}"
            );
        }
        // The control: the same URL with none of them.
        let mut value = policy_v2_value();
        value["route"]["endpoint"] = json!("https://api.example.test/v1/responses");
        assert!(decoded(&value).is_ok());
    }

    #[test]
    fn the_endpoint_is_one_absolute_https_url() {
        for bad in [
            "",
            "api.example.test/v1/responses",
            "http://api.example.test/v1/responses",
            "https:///v1/responses",
            "https://user:pw@api.example.test/v1/responses",
            "https://api.example.test/v1/responses?x=1",
            "https://api.example.test/v1/responses#x",
            "https://API.example.test/v1/responses",
            "https://api.example.test/v1/../responses",
            " https://api.example.test/v1/responses",
        ] {
            assert_eq!(
                refused(|v| v["route"]["endpoint"] = json!(bad)),
                PolicyV2Refusal::InvalidEndpoint,
                "{bad}"
            );
        }
    }

    #[test]
    fn response_models_are_a_set_of_exact_names() {
        for bad in [
            json!([]),
            json!([""]),
            json!([" "]),
            json!(["a", "a"]),
            json!(["a", "b", "a"]),
        ] {
            assert_eq!(
                refused(|v| v["route"]["response_models"] = bad.clone()),
                PolicyV2Refusal::InvalidResponseModels,
                "{bad}"
            );
        }
        let route = decode_v2(&policy_v2_value()).route;
        assert!(route.accepts_response_model("m-1-snapshot-a"));
        assert!(route.accepts_response_model("m-1-snapshot-b"));
        for other in [
            "m-1",
            "m-1-snapshot",
            "m-1-snapshot-a-2",
            "M-1-SNAPSHOT-A",
            "",
        ] {
            assert!(!route.accepts_response_model(other), "{other}");
        }
    }

    #[test]
    fn the_input_bound_is_declared_and_never_zero() {
        assert_eq!(
            refused(|v| v["input_bound"]["method"] = json!("body_bytes")),
            PolicyV2Refusal::InvalidInputBound
        );
        assert_eq!(
            refused(|v| v["input_bound"]["provider_max_input_tokens"] = json!(0)),
            PolicyV2Refusal::ZeroBound
        );
        let mut value = policy_v2_value();
        value["input_bound"]
            .as_object_mut()
            .unwrap()
            .remove("provider_max_input_tokens");
        assert_eq!(
            decode_v2(&value).input_bound.provider_max_input_tokens,
            None
        );
        let mut value = policy_v2_value();
        value["input_bound"]["provider_max_input_tokens"] = json!(1);
        assert_eq!(
            decode_v2(&value).input_bound.provider_max_input_tokens,
            Some(1)
        );
    }

    #[test]
    fn a_zero_bound_or_a_limit_past_counting_is_refused() {
        for key in ["max_output_tokens", "max_input_tokens"] {
            assert_eq!(
                refused(|v| v[key] = json!(0)),
                PolicyV2Refusal::ZeroBound,
                "{key}"
            );
        }
        for key in ["wall_seconds", "final_repair_seconds"] {
            assert_eq!(
                refused(|v| v["limits"][key] = json!(0)),
                PolicyV2Refusal::ZeroBound,
                "{key}"
            );
            assert_eq!(
                refused(|v| v["limits"][key] = json!(u64::MAX / 1000 + 1)),
                PolicyV2Refusal::LimitTooLarge,
                "{key}"
            );
        }
    }

    #[test]
    fn the_budget_id_and_the_ledger_dir_are_checked() {
        assert_eq!(
            refused(|v| v["budget_id"] = json!("wallet-other")),
            PolicyV2Refusal::BudgetIdDiffers
        );
        assert_eq!(
            RunBudgetPolicyV2::decode(policy_v2_value().to_string().as_bytes(), "../x"),
            Err(PolicyV2Refusal::InvalidBudgetId)
        );
        for bad in ["relative", "", "/operator/../elsewhere"] {
            assert_eq!(
                refused(|v| v["ledger_dir"] = json!(bad)),
                PolicyV2Refusal::InvalidLedgerDir,
                "{bad}"
            );
        }
    }

    #[test]
    fn no_price_is_chosen_by_a_model_name() {
        // The same prices decode for any model name: nothing in the decode reads it for a price.
        let base = decode_v2(&policy_v2_value()).tariff;
        for model in ["gpt-6-luna", "other-model", "m-2"] {
            let mut value = policy_v2_value();
            value["route"]["model"] = json!(model);
            value["route"]["response_models"] = json!([model]);
            assert_eq!(decode_v2(&value).tariff, base, "{model}");
        }
    }
}
