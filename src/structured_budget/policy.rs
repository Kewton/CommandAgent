//! The operator's declaration of one wallet.
//!
//! Only bytes come in here: finding and reading the file is the entry's concern. The product holds
//! no price, no limit, and no route of its own, and everything the wallet allows is written here.

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::reservation::Money;
use crate::planner::structured_stages::receipt::sha256_hex;

pub(crate) const POLICY_PROTOCOL: &str = "run-budget.policy/v1";
pub(crate) const ROUTE_API_CHAT_COMPLETIONS: &str = "chat_completions";
const MAX_POLICY_BYTES: usize = 1 << 20;
const MAX_BUDGET_ID_BYTES: usize = 64;

/// Fixed words, safe for a message: a decoder's own text could quote the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyRefusal {
    TooLarge,
    Malformed,
    WrongProtocol,
    InvalidBudgetId,
    BudgetIdDiffers,
    InvalidAmount,
    FallbacksAllowed,
    ReasoningNotInOutput,
    UnsupportedApi,
    InvalidRoute,
    InvalidTariff,
    ZeroBound,
    InvalidLedgerDir,
    LimitTooLarge,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    protocol: String,
    budget_id: String,
    limits: RawLimits,
    tariff: RawTariff,
    route: RawRoute,
    max_output_tokens: u64,
    max_input_tokens: u64,
    input_overhead: u64,
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
struct RawTariff {
    revision: String,
    input_per_million: String,
    cached_input_per_million: String,
    cache_write_per_million: String,
    output_per_million: String,
    request_fee: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRoute {
    api: String,
    model: String,
    upstream_only: Vec<String>,
    allow_fallbacks: bool,
    effort: String,
    reasoning_in_output: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Limits {
    pub(crate) cost: Money,
    pub(crate) physical_sends: u64,
    pub(crate) logical_global: u64,
    pub(crate) logical_per_stage: u64,
    pub(crate) tokens: u64,
    pub(crate) wall_ms: u64,
    pub(crate) final_repair_attempts: u64,
    pub(crate) final_repair_ms: u64,
}

/// Rates per million tokens, in the fixed revision the operator priced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tariff {
    pub(crate) revision: String,
    pub(crate) input: Money,
    pub(crate) cached_input: Money,
    pub(crate) cache_write: Money,
    pub(crate) output: Money,
    pub(crate) request_fee: Money,
}

/// What a request actually puts on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WireRoute {
    pub(crate) api: String,
    pub(crate) model: String,
    pub(crate) upstream_only: Vec<String>,
    pub(crate) allow_fallbacks: bool,
    pub(crate) effort: String,
}

/// The route the wallet allows. Reasoning counting inside the output is what the reservation
/// formula assumes, so a route that does not declare it was refused on the way here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Route {
    pub(crate) api: String,
    pub(crate) model: String,
    pub(crate) upstream_only: Vec<String>,
    pub(crate) effort: String,
}

impl Route {
    pub(crate) fn admits(&self, wire: &WireRoute) -> bool {
        !wire.allow_fallbacks
            && wire.api == self.api
            && wire.model == self.model
            && wire.upstream_only == self.upstream_only
            && wire.effort == self.effort
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunBudgetPolicy {
    pub(crate) budget_id: String,
    /// Of the exact bytes admitted. The ledger holds a wallet to it.
    pub(crate) sha256: String,
    pub(crate) limits: Limits,
    pub(crate) tariff: Tariff,
    pub(crate) route: Route,
    pub(crate) max_output_tokens: u64,
    pub(crate) max_input_tokens: u64,
    pub(crate) input_overhead: u64,
    pub(crate) ledger_dir: PathBuf,
}

/// An opaque ID that is safe as one path component.
pub(crate) fn valid_budget_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_BUDGET_ID_BYTES
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn amount(text: &str) -> Result<Money, PolicyRefusal> {
    Money::parse_decimal_up(text).map_err(|_| PolicyRefusal::InvalidAmount)
}

fn milliseconds(seconds: u64) -> Result<u64, PolicyRefusal> {
    seconds
        .checked_mul(1000)
        .ok_or(PolicyRefusal::LimitTooLarge)
}

fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

fn route(raw: RawRoute) -> Result<Route, PolicyRefusal> {
    if raw.api != ROUTE_API_CHAT_COMPLETIONS {
        return Err(PolicyRefusal::UnsupportedApi);
    }
    if raw.allow_fallbacks {
        return Err(PolicyRefusal::FallbacksAllowed);
    }
    if !raw.reasoning_in_output {
        return Err(PolicyRefusal::ReasoningNotInOutput);
    }
    let mut upstreams = raw.upstream_only.iter().collect::<Vec<_>>();
    upstreams.sort();
    upstreams.dedup();
    if blank(&raw.model)
        || blank(&raw.effort)
        || raw.upstream_only.is_empty()
        || upstreams.len() != raw.upstream_only.len()
        || raw.upstream_only.iter().any(|upstream| blank(upstream))
    {
        return Err(PolicyRefusal::InvalidRoute);
    }
    Ok(Route {
        api: raw.api,
        model: raw.model,
        upstream_only: raw.upstream_only,
        effort: raw.effort,
    })
}

fn tariff(raw: RawTariff) -> Result<Tariff, PolicyRefusal> {
    if blank(&raw.revision) {
        return Err(PolicyRefusal::InvalidTariff);
    }
    Ok(Tariff {
        input: amount(&raw.input_per_million)?,
        cached_input: amount(&raw.cached_input_per_million)?,
        cache_write: amount(&raw.cache_write_per_million)?,
        output: amount(&raw.output_per_million)?,
        request_fee: amount(&raw.request_fee)?,
        revision: raw.revision,
    })
}

fn limits(raw: &RawLimits) -> Result<Limits, PolicyRefusal> {
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

fn ledger_dir(text: &str) -> Result<PathBuf, PolicyRefusal> {
    let path = Path::new(text);
    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PolicyRefusal::InvalidLedgerDir);
    }
    Ok(path.to_path_buf())
}

impl RunBudgetPolicy {
    /// The ID is the one the entry was asked to use: a file copied under another name is refused.
    pub(crate) fn decode(bytes: &[u8], expected_id: &str) -> Result<Self, PolicyRefusal> {
        if bytes.len() > MAX_POLICY_BYTES {
            return Err(PolicyRefusal::TooLarge);
        }
        if !valid_budget_id(expected_id) {
            return Err(PolicyRefusal::InvalidBudgetId);
        }
        let raw: RawPolicy = serde_json::from_slice(bytes).map_err(|_| PolicyRefusal::Malformed)?;
        if raw.protocol != POLICY_PROTOCOL {
            return Err(PolicyRefusal::WrongProtocol);
        }
        if raw.budget_id != expected_id {
            return Err(PolicyRefusal::BudgetIdDiffers);
        }
        if raw.max_output_tokens == 0 || raw.max_input_tokens == 0 {
            return Err(PolicyRefusal::ZeroBound);
        }
        Ok(Self {
            sha256: sha256_hex(bytes),
            limits: limits(&raw.limits)?,
            tariff: tariff(raw.tariff)?,
            route: route(raw.route)?,
            ledger_dir: ledger_dir(&raw.ledger_dir)?,
            budget_id: raw.budget_id,
            max_output_tokens: raw.max_output_tokens,
            max_input_tokens: raw.max_input_tokens,
            input_overhead: raw.input_overhead,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::super::reservation::Money;
    use super::super::test_support::{WALLET, policy, policy_value, route};
    use super::{PolicyRefusal, RunBudgetPolicy, valid_budget_id};

    fn refused(change: impl FnOnce(&mut Value)) -> PolicyRefusal {
        let mut value = policy_value();
        change(&mut value);
        RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap_err()
    }

    #[test]
    fn a_complete_policy_decodes_with_its_exact_values() {
        let decoded = policy();
        assert_eq!(decoded.budget_id, WALLET);
        assert_eq!(
            decoded.limits.cost,
            Money::parse_decimal_up("1000").unwrap()
        );
        assert_eq!(decoded.limits.physical_sends, 1000);
        assert_eq!(decoded.limits.wall_ms, 100_000_000);
        assert_eq!(decoded.limits.final_repair_ms, 100_000_000);
        assert_eq!(decoded.tariff.revision, "tariff-one");
        assert_eq!(
            decoded.tariff.output,
            Money::parse_decimal_up("10").unwrap()
        );
        assert_eq!(decoded.max_output_tokens, 50);
        assert_eq!(decoded.max_input_tokens, 1000);
        assert_eq!(decoded.input_overhead, 10);
        assert_eq!(
            decoded.ledger_dir,
            std::path::Path::new("/operator/ledgers")
        );
        assert_eq!(decoded.sha256.len(), 64);
    }

    #[test]
    fn the_digest_is_of_the_exact_bytes() {
        let bytes = policy_value().to_string();
        let spaced = format!("{bytes} ");
        let one = RunBudgetPolicy::decode(bytes.as_bytes(), WALLET).unwrap();
        let two = RunBudgetPolicy::decode(spaced.as_bytes(), WALLET).unwrap();
        assert_ne!(one.sha256, two.sha256);
    }

    #[test]
    fn not_json_and_an_oversized_file_are_refused() {
        assert_eq!(
            RunBudgetPolicy::decode(b"not json", WALLET),
            Err(PolicyRefusal::Malformed)
        );
        let huge = vec![b' '; (1 << 20) + 1];
        assert_eq!(
            RunBudgetPolicy::decode(&huge, WALLET),
            Err(PolicyRefusal::TooLarge)
        );
    }

    #[test]
    fn another_protocol_is_refused_and_the_profile_tool_protocol_is_not_accepted() {
        for protocol in ["profile-tool.descriptor/v1", "run-budget.policy/v2", ""] {
            assert_eq!(
                refused(|v| v["protocol"] = json!(protocol)),
                PolicyRefusal::WrongProtocol,
                "{protocol}"
            );
        }
    }

    #[test]
    fn an_unknown_field_is_refused_at_every_level() {
        assert_eq!(refused(|v| v["extra"] = json!(1)), PolicyRefusal::Malformed);
        for section in ["limits", "tariff", "route"] {
            assert_eq!(
                refused(|v| v[section]["extra"] = json!(1)),
                PolicyRefusal::Malformed,
                "{section}"
            );
        }
    }

    #[test]
    fn every_missing_field_is_refused() {
        let value = policy_value();
        for key in value.as_object().unwrap().keys() {
            assert_eq!(
                refused(|v| {
                    v.as_object_mut().unwrap().remove(key);
                }),
                PolicyRefusal::Malformed,
                "{key}"
            );
        }
        for section in ["limits", "tariff", "route"] {
            for key in value[section].as_object().unwrap().keys() {
                assert_eq!(
                    refused(|v| {
                        v[section].as_object_mut().unwrap().remove(key);
                    }),
                    PolicyRefusal::Malformed,
                    "{section}.{key}"
                );
            }
        }
    }

    #[test]
    fn a_negative_or_float_count_is_refused() {
        for bad in [json!(-1), json!(1.5), json!(1.0), json!(1e3), json!("1")] {
            assert_eq!(
                refused(|v| v["limits"]["physical_sends"] = bad.clone()),
                PolicyRefusal::Malformed,
                "{bad}"
            );
            assert_eq!(
                refused(|v| v["input_overhead"] = bad.clone()),
                PolicyRefusal::Malformed,
                "{bad}"
            );
        }
    }

    #[test]
    fn a_money_value_must_be_a_decimal_string() {
        for bad in [json!(1.5), json!(1)] {
            assert_eq!(
                refused(|v| v["limits"]["cost_usd"] = bad.clone()),
                PolicyRefusal::Malformed,
                "{bad}"
            );
            assert_eq!(
                refused(|v| v["tariff"]["output_per_million"] = bad.clone()),
                PolicyRefusal::Malformed,
                "{bad}"
            );
        }
        for bad in ["-1", "abc", "", "1e99999"] {
            assert_eq!(
                refused(|v| v["limits"]["cost_usd"] = json!(bad)),
                PolicyRefusal::InvalidAmount,
                "{bad}"
            );
            assert_eq!(
                refused(|v| v["tariff"]["request_fee"] = json!(bad)),
                PolicyRefusal::InvalidAmount,
                "{bad}"
            );
        }
    }

    #[test]
    fn a_route_that_allows_fallbacks_is_refused() {
        assert_eq!(
            refused(|v| v["route"]["allow_fallbacks"] = json!(true)),
            PolicyRefusal::FallbacksAllowed
        );
    }

    #[test]
    fn a_route_without_reasoning_in_the_output_is_refused() {
        assert_eq!(
            refused(|v| v["route"]["reasoning_in_output"] = json!(false)),
            PolicyRefusal::ReasoningNotInOutput
        );
        assert_eq!(
            refused(|v| {
                v["route"]
                    .as_object_mut()
                    .unwrap()
                    .remove("reasoning_in_output");
            }),
            PolicyRefusal::Malformed
        );
    }

    #[test]
    fn an_api_other_than_chat_completions_is_refused() {
        for api in ["responses", "gemini", ""] {
            assert_eq!(
                refused(|v| v["route"]["api"] = json!(api)),
                PolicyRefusal::UnsupportedApi,
                "{api}"
            );
        }
    }

    #[test]
    fn an_empty_route_part_is_refused() {
        assert_eq!(
            refused(|v| v["route"]["model"] = json!(" ")),
            PolicyRefusal::InvalidRoute
        );
        assert_eq!(
            refused(|v| v["route"]["effort"] = json!("")),
            PolicyRefusal::InvalidRoute
        );
        assert_eq!(
            refused(|v| v["route"]["upstream_only"] = json!([])),
            PolicyRefusal::InvalidRoute
        );
        assert_eq!(
            refused(|v| v["route"]["upstream_only"] = json!(["a", ""])),
            PolicyRefusal::InvalidRoute
        );
        assert_eq!(
            refused(|v| v["route"]["upstream_only"] = json!(["a", "a"])),
            PolicyRefusal::InvalidRoute
        );
    }

    #[test]
    fn an_empty_tariff_revision_is_refused() {
        assert_eq!(
            refused(|v| v["tariff"]["revision"] = json!(" ")),
            PolicyRefusal::InvalidTariff
        );
    }

    #[test]
    fn a_zero_output_or_input_bound_is_refused() {
        assert_eq!(
            refused(|v| v["max_output_tokens"] = json!(0)),
            PolicyRefusal::ZeroBound
        );
        assert_eq!(
            refused(|v| v["max_input_tokens"] = json!(0)),
            PolicyRefusal::ZeroBound
        );
    }

    #[test]
    fn seconds_that_overflow_as_milliseconds_are_refused() {
        let too_many = u64::MAX / 1000 + 1;
        assert_eq!(
            refused(|v| v["limits"]["wall_seconds"] = json!(too_many)),
            PolicyRefusal::LimitTooLarge
        );
        assert_eq!(
            refused(|v| v["limits"]["final_repair_seconds"] = json!(too_many)),
            PolicyRefusal::LimitTooLarge
        );
    }

    #[test]
    fn a_ledger_dir_that_is_relative_or_climbs_is_refused() {
        for bad in ["relative/dir", "", "/operator/../elsewhere"] {
            assert_eq!(
                refused(|v| v["ledger_dir"] = json!(bad)),
                PolicyRefusal::InvalidLedgerDir,
                "{bad}"
            );
        }
    }

    #[test]
    fn a_budget_id_is_one_safe_path_component() {
        for good in ["a", "wallet-1", "run_2.b", "A9"] {
            assert!(valid_budget_id(good), "{good}");
        }
        let long = "a".repeat(65);
        for bad in [
            "", ".", "..", ".hidden", "a/b", "a\\b", "a b", "é", "a\0", &long,
        ] {
            assert!(!valid_budget_id(bad), "{bad}");
        }
        assert!(valid_budget_id(&"a".repeat(64)));
    }

    #[test]
    fn an_id_in_the_file_other_than_the_selected_one_is_refused() {
        assert_eq!(
            refused(|v| v["budget_id"] = json!("wallet-beta")),
            PolicyRefusal::BudgetIdDiffers
        );
        let bytes = policy_value().to_string();
        assert_eq!(
            RunBudgetPolicy::decode(bytes.as_bytes(), "../wallet-alpha"),
            Err(PolicyRefusal::InvalidBudgetId)
        );
    }

    #[test]
    fn a_route_admits_only_what_it_declares() {
        let declared = policy().route;
        assert!(declared.admits(&route()));
        let mut other = route();
        other.model = "other".to_string();
        assert!(!declared.admits(&other));
        let mut other = route();
        other.api = "responses".to_string();
        assert!(!declared.admits(&other));
        let mut other = route();
        other.effort = "high".to_string();
        assert!(!declared.admits(&other));
        let mut other = route();
        other.upstream_only = vec!["other".to_string()];
        assert!(!declared.admits(&other));
        let mut other = route();
        other.upstream_only.clear();
        assert!(!declared.admits(&other));
        let mut other = route();
        other.allow_fallbacks = true;
        assert!(!declared.admits(&other));
    }
}
