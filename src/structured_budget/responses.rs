//! A send of a `usage_tariff` wallet: direct, non-streaming Responses (Issue #1006, designs 2–3).
//!
//! What a send costs and where it goes are read off its final bytes, as in v1, and nothing a
//! caller says is taken instead, except the endpoint, which only the transport knows: it is
//! passed in by the transport and has to be the policy's.
//!
//! **Admission.** The body is one JSON object, no key twice, of `model`, `input`,
//! `max_output_tokens`, `reasoning`, `store`, `include`, `tools`, `service_tier` only. `model`,
//! `reasoning: {effort}` and `service_tier` are the policy's; `store` is `false`; `include` holds
//! `reasoning.encrypted_content` at most; a tool is a `function` tool; `max_output_tokens` is a
//! positive count no more than the policy's. Every input item is a message of text, a
//! `function_call`, a `function_call_output`, or a `reasoning` item; anything else is refused.
//!
//! **Input bound.** With no opaque item the bound is v1's, the body's bytes and
//! `input_overhead`. With a reasoning item it is the policy's `provider_max_input_tokens`, and a
//! policy that declares none sends no such request. A bound past `max_input_tokens` sends nothing.
//!
//! **Reservation.** The bound and the output at the highest rate of every tier and input class
//! the bound can reach, plus the highest fee (`usage_tariff::reservation_bound`), judged with
//! every other limit in one place; exactly at a cap passes.
//!
//! **Settlement.** A 2xx body's usage is read by the one reader the telemetry shares, the
//! response's `model` and `service_tier` are checked, and the tariff charges it. Anything that is
//! not known leaves the send unknown, with its reservation, and stops the wallet. The response
//! identity a submission takes comes only from the charge on disk.

use std::collections::BTreeMap;

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::clock::RunClock;
use super::identity::{FinalRepairClaim, FinalRepairId, PhysicalId, ResponseIdentity, SendPurpose};
use super::ledger::LedgerFailure;
use super::ledger_v2::{
    AttemptRecordV2, BudgetCharge, BudgetLedgerV2, Demand, InputBoundMethod, ProviderBilled,
    ResponseStop, UnknownCause, UsageCounts, WalletStateV2,
};
use super::policy_v2::RunBudgetPolicyV2;
use super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money};
use super::usage_tariff::{ChargeStop, charge, reservation_bound};
use crate::planner::structured_stages::receipt::sha256_hex;
use crate::providers::usage_observation::read_responses_usage;

/// Every top-level key a body may have. Any other could change what a send costs, so it is
/// refused, not skipped.
const BODY_KEYS: [&str; 8] = [
    "model",
    "input",
    "max_output_tokens",
    "reasoning",
    "store",
    "include",
    "tools",
    "service_tier",
];
const INCLUDE_ALLOWED: &str = "reasoning.encrypted_content";
const MESSAGE_ROLES: [&str; 4] = ["user", "assistant", "system", "developer"];
const INPUT_MESSAGE_KEYS: [&str; 3] = ["role", "content", "phase"];
const OUTPUT_MESSAGE_KEYS: [&str; 6] = ["type", "id", "status", "role", "content", "phase"];
const TEXT_PART_KEYS: [&str; 2] = ["type", "text"];
const OUTPUT_TEXT_KEYS: [&str; 4] = ["type", "text", "annotations", "logprobs"];
const FUNCTION_CALL_KEYS: [&str; 9] = [
    "type",
    "id",
    "call_id",
    "name",
    "arguments",
    "status",
    "caller",
    "async",
    "namespace",
];
const FUNCTION_CALL_OUTPUT_KEYS: [&str; 3] = ["type", "call_id", "output"];
const REASONING_KEYS: [&str; 6] = [
    "type",
    "id",
    "summary",
    "encrypted_content",
    "content",
    "status",
];
const TOOL_KEYS: [&str; 5] = ["type", "name", "description", "parameters", "strict"];

/// One send about to be made under a `usage_tariff` wallet.
#[derive(Debug, Clone)]
pub(crate) struct ResponsesReserveRequest<'a> {
    /// The final wire bytes. Only their length, digest and what they say are kept.
    pub(crate) body: &'a [u8],
    /// Where the transport posts them, from the transport.
    pub(crate) endpoint: &'a str,
    pub(crate) purpose: SendPurpose,
    pub(crate) final_repair: Option<FinalRepairClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResponsesReservation {
    pub(crate) physical: PhysicalId,
    pub(crate) final_repair: Option<FinalRepairId>,
    pub(crate) amount: Money,
    pub(crate) input_bound_method: InputBoundMethod,
    pub(crate) input_bound: u64,
}

/// What the bytes of an admitted body say about themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdmittedBody {
    pub(crate) input_bound_method: InputBoundMethod,
    pub(crate) input_bound: u64,
    pub(crate) output_tokens: u64,
}

/// How a response left a send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResponsesSettle {
    /// The budget charged it. The identity is what its calls are of.
    Charged {
        response: ResponseIdentity,
        charged: Money,
        latched: bool,
    },
    Unknown(UnknownCause),
}

/// The members of a JSON object, none read yet. A key twice is refused: this parser and the one
/// upstream could each take another of the two.
struct Unique<'a>(BTreeMap<String, &'a RawValue>);

impl<'de> Deserialize<'de> for Unique<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Members;
        impl<'de> Visitor<'de> for Members {
            type Value = Unique<'de>;

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
                Ok(Unique(members))
            }
        }
        deserializer.deserialize_map(Members)
    }
}

fn object(raw: &RawValue) -> Result<BTreeMap<String, &RawValue>, BudgetRefusal> {
    serde_json::from_str::<Unique<'_>>(raw.get())
        .map(|unique| unique.0)
        .map_err(|_| BudgetRefusal::BodyNotAdmitted)
}

fn array(raw: &RawValue) -> Result<Vec<&RawValue>, BudgetRefusal> {
    serde_json::from_str(raw.get()).map_err(|_| BudgetRefusal::BodyNotAdmitted)
}

fn string(raw: &RawValue) -> Option<String> {
    serde_json::from_str(raw.get()).ok()
}

fn only_keys(members: &BTreeMap<String, &RawValue>, allowed: &[&str]) -> bool {
    members.keys().all(|key| allowed.contains(&key.as_str()))
}

/// The route the body asks for is the policy's: model, effort (and nothing else in
/// `reasoning`), and the `default` service tier.
fn refuse_another_route(
    policy: &RunBudgetPolicyV2,
    members: &BTreeMap<String, &RawValue>,
) -> Result<(), BudgetRefusal> {
    let route = &policy.route;
    let model = members.get("model").and_then(|raw| string(raw));
    let tier = members.get("service_tier").and_then(|raw| string(raw));
    let effort = match members.get("reasoning") {
        Some(raw) => {
            let reasoning = object(raw).map_err(|_| BudgetRefusal::RouteNotAdmitted)?;
            if reasoning.len() != 1 {
                return Err(BudgetRefusal::RouteNotAdmitted);
            }
            reasoning.get("effort").and_then(|raw| string(raw))
        }
        None => None,
    };
    if model.as_deref() != Some(route.model.as_str())
        || effort.as_deref() != Some(route.effort.as_str())
        || tier.as_deref() != Some(route.service_tier.as_str())
    {
        return Err(BudgetRefusal::RouteNotAdmitted);
    }
    Ok(())
}

/// `store` is `false`, `include` asks for the encrypted reasoning at most, and every tool is a
/// function tool with a name.
fn refuse_other_options(members: &BTreeMap<String, &RawValue>) -> Result<(), BudgetRefusal> {
    if members.get("store").map(|raw| raw.get().trim()) != Some("false") {
        return Err(BudgetRefusal::BodyNotAdmitted);
    }
    if let Some(raw) = members.get("include") {
        let include = array(raw)?;
        if include.len() > 1
            || include
                .iter()
                .any(|item| string(item).as_deref() != Some(INCLUDE_ALLOWED))
        {
            return Err(BudgetRefusal::BodyNotAdmitted);
        }
    }
    if let Some(raw) = members.get("tools") {
        for tool in array(raw)? {
            let tool = object(tool)?;
            let is_function =
                tool.get("type").and_then(|raw| string(raw)).as_deref() == Some("function");
            let named = tool.get("name").and_then(|raw| string(raw)).is_some();
            if !is_function || !named || !only_keys(&tool, &TOOL_KEYS) {
                return Err(BudgetRefusal::BodyNotAdmitted);
            }
        }
    }
    Ok(())
}

/// The keys of an item are within `allowed`, every one of `strings` is there as a string, and
/// every one of `optional_strings` that is there is a string.
fn shaped(
    item: &BTreeMap<String, &RawValue>,
    allowed: &[&str],
    strings: &[&str],
    optional_strings: &[&str],
) -> bool {
    only_keys(item, allowed)
        && strings
            .iter()
            .all(|key| item.get(*key).is_some_and(|raw| string(raw).is_some()))
        && optional_strings
            .iter()
            .all(|key| item.get(*key).is_none_or(|raw| string(raw).is_some()))
}

fn empty_array(raw: &RawValue) -> bool {
    array(raw).is_ok_and(|items| items.is_empty())
}

/// The item statuses the reference lists, the one for a message, a call, and a reasoning item.
const STATUSES: [&str; 3] = ["in_progress", "completed", "incomplete"];
/// A message's documented phases; `null` is allowed beside them.
const PHASES: [&str; 2] = ["commentary", "final_answer"];

fn one_of(raw: &RawValue, values: &[&str]) -> bool {
    string(raw).is_some_and(|value| values.contains(&value.as_str()))
}

fn is_null(raw: &RawValue) -> bool {
    raw.get().trim() == "null"
}

/// `key` is absent, or holds one of `values` (or `null`, when `nullable`).
fn optional_one_of(
    item: &BTreeMap<String, &RawValue>,
    key: &str,
    values: &[&str],
    nullable: bool,
) -> bool {
    item.get(key)
        .is_none_or(|raw| one_of(raw, values) || (nullable && is_null(raw)))
}

/// An object of exactly `type` and a string `text`, its type `kind`.
fn is_typed_text(raw: &RawValue, kind: &str) -> bool {
    object(raw).is_ok_and(|part| {
        part.len() == 2
            && part.get("type").is_some_and(|raw| one_of(raw, &[kind]))
            && part.get("text").is_some_and(|raw| string(raw).is_some())
    })
}

/// One text part, as the builder writes it or as a replayed reply holds it: `input_text` with its
/// text, or `output_text` with its text and no annotation or logprob in it. A refusal part, an
/// annotation, a logprob, or a cache breakpoint is refused.
fn is_text_part(raw: &RawValue, kinds: &[&str]) -> bool {
    let Ok(part) = object(raw) else {
        return false;
    };
    match part.get("type").and_then(|raw| string(raw)).as_deref() {
        Some(kind) if !kinds.contains(&kind) => false,
        Some("input_text") => shaped(&part, &TEXT_PART_KEYS, &["type", "text"], &[]),
        Some("output_text") => {
            shaped(&part, &OUTPUT_TEXT_KEYS, &["type", "text"], &[])
                && ["annotations", "logprobs"]
                    .iter()
                    .all(|key| part.get(*key).is_none_or(|raw| empty_array(raw)))
        }
        _ => false,
    }
}

fn content_is_text(item: &BTreeMap<String, &RawValue>, kinds: &[&str]) -> bool {
    match item.get("content") {
        Some(raw) if string(raw).is_some() => true,
        Some(raw) => {
            array(raw).is_ok_and(|parts| parts.iter().all(|part| is_text_part(part, kinds)))
        }
        None => false,
    }
}

/// A message the builder writes, with no `type`: a role, and text content as a string or as text
/// parts, and the documented optional phase.
fn is_text_message(item: &BTreeMap<String, &RawValue>) -> bool {
    item.get("role")
        .is_some_and(|raw| one_of(raw, &MESSAGE_ROLES))
        && content_is_text(item, &["input_text", "output_text"])
        && shaped(item, &INPUT_MESSAGE_KEYS, &["role"], &[])
        && optional_one_of(item, "phase", &PHASES, true)
}

/// A reply's message, replayed as it came: the assistant's, with its ID, a documented status, its
/// text, and the documented optional phase.
fn is_output_message(item: &BTreeMap<String, &RawValue>) -> bool {
    item.get("role")
        .is_some_and(|raw| one_of(raw, &["assistant"]))
        && item.get("status").is_some_and(|raw| one_of(raw, &STATUSES))
        && content_is_text(item, &["output_text"])
        && shaped(item, &OUTPUT_MESSAGE_KEYS, &["type", "id", "role"], &[])
        && optional_one_of(item, "phase", &PHASES, true)
}

/// A call as the reply held it: its call ID, name, and arguments as a string, and the documented
/// optional keys with the values a direct call has: a documented status, `caller` exactly
/// `{"type":"direct"}`, `async` exactly `false`, and a string `namespace`.
fn is_function_call(item: &BTreeMap<String, &RawValue>) -> bool {
    shaped(
        item,
        &FUNCTION_CALL_KEYS,
        &["type", "call_id", "name", "arguments"],
        &["id", "namespace"],
    ) && optional_one_of(item, "status", &STATUSES, false)
        && item.get("caller").is_none_or(|raw| {
            object(raw).is_ok_and(|caller| {
                caller.len() == 1
                    && caller
                        .get("type")
                        .is_some_and(|raw| one_of(raw, &["direct"]))
            })
        })
        && item
            .get("async")
            .is_none_or(|raw| raw.get().trim() == "false")
}

/// A call's output as the builder writes it: the call ID and the output as a string. An array of
/// images or files is another input and is not this, and none of its optional keys is taken.
fn is_function_call_output(item: &BTreeMap<String, &RawValue>) -> bool {
    shaped(
        item,
        &FUNCTION_CALL_OUTPUT_KEYS,
        &["type", "call_id", "output"],
        &[],
    )
}

/// A reasoning item as the reply held it: its ID, its summary of `summary_text` parts, and the
/// documented optional `reasoning_text` content, encrypted content (a string or `null`), and
/// status. Its content is opaque, and it is bounded by the declared most input, not by its bytes.
fn is_reasoning(item: &BTreeMap<String, &RawValue>) -> bool {
    shaped(item, &REASONING_KEYS, &["type", "id"], &[])
        && item.get("summary").is_some_and(|raw| {
            array(raw)
                .is_ok_and(|parts| parts.iter().all(|part| is_typed_text(part, "summary_text")))
        })
        && item.get("content").is_none_or(|raw| {
            array(raw).is_ok_and(|parts| {
                parts
                    .iter()
                    .all(|part| is_typed_text(part, "reasoning_text"))
            })
        })
        && item
            .get("encrypted_content")
            .is_none_or(|raw| string(raw).is_some() || is_null(raw))
        && optional_one_of(item, "status", &STATUSES, false)
}

/// Whether the input holds an opaque item. An item of a kind not listed, or of a listed kind in
/// another shape than the wire's, is refused.
fn input_has_opaque_item(raw: &RawValue) -> Result<bool, BudgetRefusal> {
    let mut opaque = false;
    for item in array(raw)? {
        let item = object(item)?;
        let kind = match item.get("type") {
            None => None,
            Some(raw) => Some(string(raw).ok_or(BudgetRefusal::BodyNotAdmitted)?),
        };
        let admitted = match kind.as_deref() {
            None => is_text_message(&item),
            Some("message") => is_output_message(&item),
            Some("function_call") => is_function_call(&item),
            Some("function_call_output") => is_function_call_output(&item),
            Some("reasoning") => {
                opaque = true;
                is_reasoning(&item)
            }
            _ => false,
        };
        if !admitted {
            return Err(BudgetRefusal::BodyNotAdmitted);
        }
    }
    Ok(opaque)
}

/// Reads an admitted body: the route, the options, the input's kind, and the bounds.
pub(crate) fn admit_body(
    policy: &RunBudgetPolicyV2,
    bytes: &[u8],
) -> Result<AdmittedBody, BudgetRefusal> {
    let Unique(members) =
        serde_json::from_slice(bytes).map_err(|_| BudgetRefusal::BodyNotAdmitted)?;
    if !only_keys(&members, &BODY_KEYS) {
        return Err(BudgetRefusal::BodyNotAdmitted);
    }
    refuse_another_route(policy, &members)?;
    refuse_other_options(&members)?;
    let output_tokens = members
        .get("max_output_tokens")
        .and_then(|raw| serde_json::from_str::<u64>(raw.get()).ok())
        .filter(|count| *count > 0)
        .ok_or(BudgetRefusal::BodyNotAdmitted)?;
    let input = members.get("input").ok_or(BudgetRefusal::BodyNotAdmitted)?;
    let opaque = input_has_opaque_item(input)?;
    if output_tokens > policy.max_output_tokens {
        return Err(BudgetRefusal::OutputBoundExceeded);
    }
    let (input_bound_method, input_bound) = if opaque {
        (
            InputBoundMethod::ProviderMaxInputTokens,
            policy
                .input_bound
                .provider_max_input_tokens
                .ok_or(BudgetRefusal::InputBoundUndeclared)?,
        )
    } else {
        (
            InputBoundMethod::BodyBytesPlusOverhead,
            u64::try_from(bytes.len())
                .ok()
                .and_then(|length| length.checked_add(policy.input_bound.input_overhead))
                .ok_or(BudgetRefusal::Overflow)?,
        )
    };
    if input_bound > policy.max_input_tokens {
        return Err(BudgetRefusal::InputBoundExceeded);
    }
    Ok(AdmittedBody {
        input_bound_method,
        input_bound,
        output_tokens,
    })
}

/// The response's `model` is one of the declared names and its `service_tier` is `default`.
fn refuse_another_responder(policy: &RunBudgetPolicyV2, text: &str) -> Result<(), ResponseStop> {
    #[derive(Default)]
    struct Top<'a> {
        model: Vec<&'a RawValue>,
        tier: Vec<&'a RawValue>,
    }
    struct Reader;
    impl<'de> Visitor<'de> for Reader {
        type Value = Top<'de>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a JSON object")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut top = Top::default();
            while let Some(key) = map.next_key::<String>()? {
                let value: &'de RawValue = map.next_value()?;
                match key.as_str() {
                    "model" => top.model.push(value),
                    "service_tier" => top.tier.push(value),
                    _ => {}
                }
            }
            Ok(top)
        }
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    // The reader of the usage already found the body an object; one that is not is not this.
    let top = deserializer
        .deserialize_map(Reader)
        .map_err(|_| ResponseStop::ModelMissing)?;
    if top.model.len() > 1 || top.tier.len() > 1 {
        return Err(ResponseStop::DuplicateKey);
    }
    let model = top.model.first().ok_or(ResponseStop::ModelMissing)?.get();
    let model: String = serde_json::from_str(model).map_err(|_| ResponseStop::ModelMissing)?;
    if !policy.route.accepts_response_model(&model) {
        return Err(ResponseStop::ModelNotDeclared);
    }
    let tier = top
        .tier
        .first()
        .ok_or(ResponseStop::ServiceTierMissing)?
        .get();
    match serde_json::from_str::<String>(tier) {
        Ok(tier) if tier == policy.route.service_tier => Ok(()),
        _ => Err(ResponseStop::ServiceTierNotDefault),
    }
}

/// What a 2xx body comes to under the policy: a charge, or the first reason it is unknown, with
/// the usage kept when the usage itself was valid.
fn judge_body(
    policy: &RunBudgetPolicyV2,
    text: &str,
) -> Result<BudgetCharge, (UnknownCause, Option<UsageCounts>)> {
    let reading = read_responses_usage(text);
    // A usage the wallet cannot settle keeps what of it the reader could validate, as observed,
    // and is never charged from it.
    let observed = reading.observed.as_ref().map(UsageCounts::of);
    let usage = reading
        .wallet
        .map_err(|stop| (UnknownCause::Usage { stop }, observed))?;
    let counts = UsageCounts::of(&usage);
    refuse_another_responder(policy, text)
        .map_err(|stop| (UnknownCause::Response { stop }, Some(counts)))?;
    let charged = charge(&policy.tariff, &usage)
        .map_err(|stop| (UnknownCause::Charge { stop }, Some(counts)))?;
    Ok(BudgetCharge {
        usage: counts,
        tier: charged.tier as u64,
        charged: charged.charge,
        late: false,
    })
}

impl WalletStateV2 {
    /// A reservation of the request's bytes, on the record before anything is sent.
    pub(super) fn reserve(
        &mut self,
        policy: &RunBudgetPolicyV2,
        request: &ResponsesReserveRequest<'_>,
        clock: &mut RunClock,
    ) -> Result<ResponsesReservation, BudgetRefusal> {
        self.success_gate()?;
        if request.endpoint != policy.route.endpoint {
            return Err(BudgetRefusal::RouteNotAdmitted);
        }
        let body = admit_body(policy, request.body)?;
        let tokens = body
            .input_bound
            .checked_add(body.output_tokens)
            .ok_or(BudgetRefusal::Overflow)?;
        let amount = reservation_bound(&policy.tariff, body.input_bound, body.output_tokens)
            .map_err(|_| BudgetRefusal::Overflow)?;
        self.refuse_final_repair_claim(request.purpose, request.final_repair)?;
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
        self.attempts.push(AttemptRecordV2 {
            physical,
            purpose: request.purpose,
            final_repair,
            request_sha256: sha256_hex(request.body),
            input_bound_method: body.input_bound_method,
            input_bound: body.input_bound,
            output_bound: body.output_tokens,
            reserved: amount,
            state: AttemptState::Reserved,
            unknown: None,
            observed: None,
            charged: None,
            provider_billed: ProviderBilled::Unreported,
        });
        Ok(ResponsesReservation {
            physical,
            final_repair,
            amount,
            input_bound_method: body.input_bound_method,
            input_bound: body.input_bound,
        })
    }

    /// `reserved` to `started`, for the same bytes, inside the deadline and a final repair's
    /// window as the ledger last observed them.
    pub(super) fn start(
        &mut self,
        policy: &RunBudgetPolicyV2,
        physical: PhysicalId,
        body: &[u8],
    ) -> Result<(), BudgetRefusal> {
        let past_deadline = self.past_the_deadline(policy);
        let past_window = self.past_the_final_repair_window(policy);
        let attempt = self.attempt_in(physical, AttemptState::Reserved, AttemptState::Started)?;
        if attempt.request_sha256 != sha256_hex(body) {
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

    /// Takes one 2xx body. A charge leaves the send settled, once, from `started` or late from
    /// `unknown`, the latch staying; anything else leaves a started send unknown and changes
    /// nothing of one already unknown.
    pub(super) fn settle_body(
        &mut self,
        policy: &RunBudgetPolicyV2,
        physical: PhysicalId,
        text: &str,
    ) -> Result<ResponsesSettle, BudgetRefusal> {
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
        let mut record = match judge_body(policy, text) {
            Ok(record) => record,
            Err((cause, observed)) => {
                if from == AttemptState::Started {
                    self.mark_unknown(physical, cause, observed)?;
                }
                return Ok(ResponsesSettle::Unknown(cause));
            }
        };
        // What the wallet would then have spent has to be a number before anything changes.
        if self.spent()?.checked_add(record.charged).is_none() {
            let cause = UnknownCause::Charge {
                stop: ChargeStop::Overflow,
            };
            if from == AttemptState::Started {
                self.mark_unknown(physical, cause, Some(record.usage))?;
            }
            return Ok(ResponsesSettle::Unknown(cause));
        }
        record.late = from == AttemptState::Unknown;
        let (charged, input, output) = (
            record.charged,
            record.usage.input_tokens,
            record.usage.output_tokens,
        );
        let attempt = self
            .attempt_mut(physical)
            .ok_or(BudgetRefusal::NoSuchSend)?;
        attempt.state = AttemptState::Settled;
        attempt.charged = Some(record);
        if !record.late {
            attempt.observed = None;
        }
        if charged > reserved {
            self.set_latch(LatchReason::OverReservation, physical);
        }
        if input > input_bound {
            self.set_latch(LatchReason::InputOverBound, physical);
        }
        if output > output_bound {
            self.set_latch(LatchReason::OutputOverBound, physical);
        }
        if self.past_the_deadline(policy) {
            self.set_latch(LatchReason::ResponseAfterDeadline, physical);
        }
        if is_final_repair && self.past_the_final_repair_window(policy) {
            self.set_latch(LatchReason::ResponseAfterFinalRepairWindow, physical);
        }
        Ok(ResponsesSettle::Charged {
            response: ResponseIdentity::of_settled(physical),
            charged,
            latched: self.latch().is_some(),
        })
    }
}

/// The ledger of this basis takes each step on disk before it takes effect.
impl BudgetLedgerV2 {
    pub(crate) fn reserve(
        &mut self,
        request: &ResponsesReserveRequest<'_>,
    ) -> Result<ResponsesReservation, LedgerFailure> {
        self.apply(|wallet, policy, clock| wallet.reserve(policy, request, clock))
    }

    /// One 2xx body's usage, which leaves the send charged or unknown on disk. The identity of a
    /// charged response is given only once the charge is written.
    pub(crate) fn settle_body(
        &mut self,
        physical: PhysicalId,
        text: &str,
    ) -> Result<ResponsesSettle, LedgerFailure> {
        self.apply(|wallet, policy, _| wallet.settle_body(policy, physical, text))
    }
}

/// The words a v1 caller reads for a send of this basis that was not charged. The cause itself is
/// on disk.
pub(crate) fn unknown_reason_of(cause: UnknownCause) -> super::settlement::UnknownReason {
    use super::settlement::UnknownReason;
    use crate::providers::usage_observation::UsageStop;
    match cause {
        UnknownCause::Transport { reason } => reason,
        UnknownCause::Usage {
            stop: UsageStop::ResponseMalformed,
        } => UnknownReason::ParseFailed,
        UnknownCause::Usage {
            stop: UsageStop::UsageMissing,
        } => UnknownReason::UsageMissing,
        UnknownCause::Usage { .. }
        | UnknownCause::Response { .. }
        | UnknownCause::Charge { .. } => UnknownReason::UsageInconsistent,
    }
}

/// Which basis a wallet counts on, for a caller that admits clients by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccountingBasis {
    /// v1: the provider's billed amount, generic Chat Completions.
    ProviderBilled,
    /// v2: the fixed tariff on validated usage, direct Responses.
    UsageTariff,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::super::identity::{FinalRepairClaim, LogicalKind, PhysicalId, SendPurpose};
    use super::super::ledger::{
        BudgetLedger, CommandWrites, LedgerEntry, LedgerFailure, LedgerOpen, LedgerRefusal,
    };
    use super::super::ledger_v2::{
        BudgetLedgerV2, InputBoundMethod, LedgerOpenV2, ProviderBilled, ResponseStop, UnknownCause,
    };
    use super::super::policy::RunBudgetPolicy;
    use super::super::policy_v2::RunBudgetPolicyV2;
    use super::super::policy_v2::test_support::{WALLET, policy_v2_none_value, policy_v2_value};
    use super::super::reservation::{AttemptState, BudgetRefusal, LatchReason, Money};
    use super::super::settlement::UnknownReason;
    use super::super::test_support::{FakeClock, policy_value};
    use super::super::transport::{
        BudgetedBody, RunBudgetHandle, SandboxView, SendRefusal, settle_body_identified,
    };
    use super::super::usage_tariff::{ChargeStop, reservation_bound};
    use super::{ResponsesReserveRequest, ResponsesSettle, admit_body};
    use crate::providers::usage_observation::UsageStop;
    use crate::tools::os_sandbox::SandboxPolicy;

    const ENDPOINT: &str = "https://api.example.test/v1/responses";
    const UNIT: u128 = 1_000_000_000_000_000_000;

    /// A wallet's directories, a declaration in its own ledger directory, and a fake clock.
    struct World {
        _root: tempfile::TempDir,
        base: PathBuf,
        workspace: PathBuf,
        ledgers: PathBuf,
        value: Value,
        clock: Arc<FakeClock>,
    }

    impl World {
        fn new() -> Self {
            Self::with(|_| {})
        }

        fn with(change: impl FnOnce(&mut Value)) -> Self {
            Self::of(policy_v2_value(), change)
        }

        fn of(mut value: Value, change: impl FnOnce(&mut Value)) -> Self {
            let root = tempfile::tempdir().unwrap();
            let base = fs::canonicalize(root.path()).unwrap();
            let workspace = base.join("workspace");
            fs::create_dir_all(&workspace).unwrap();
            fs::create_dir_all(base.join("run-tmp")).unwrap();
            let ledgers = base.join("ledgers");
            value["ledger_dir"] = json!(ledgers.to_str().unwrap());
            change(&mut value);
            Self {
                _root: root,
                base,
                workspace,
                ledgers,
                value,
                clock: FakeClock::at(7_000),
            }
        }

        fn writes(&self) -> CommandWrites {
            CommandWrites::of(SandboxPolicy::with_home(
                &self.workspace,
                &self.base.join("run-tmp"),
                self.base.join("home"),
            ))
        }

        fn policy_of(value: &Value) -> Arc<RunBudgetPolicyV2> {
            Arc::new(RunBudgetPolicyV2::decode(value.to_string().as_bytes(), WALLET).unwrap())
        }

        fn policy(&self) -> Arc<RunBudgetPolicyV2> {
            Self::policy_of(&self.value)
        }

        fn open_with(
            &self,
            policy: Arc<RunBudgetPolicyV2>,
            entry: LedgerEntry,
        ) -> Result<BudgetLedgerV2, LedgerRefusal> {
            BudgetLedgerV2::open(LedgerOpenV2 {
                policy,
                workspace: &self.workspace,
                explicit_profile: None,
                entry,
                clock: self.clock.clone(),
                command_writes: Some(self.writes()),
            })
        }

        fn open(&self) -> BudgetLedgerV2 {
            self.open_with(self.policy(), LedgerEntry::Fresh).unwrap()
        }

        /// A v1 wallet of the same ID in the same directory.
        fn open_v1(&self, entry: LedgerEntry) -> Result<BudgetLedger, LedgerRefusal> {
            let mut value = policy_value();
            value["budget_id"] = json!(WALLET);
            value["ledger_dir"] = self.value["ledger_dir"].clone();
            let policy = RunBudgetPolicy::decode(value.to_string().as_bytes(), WALLET).unwrap();
            BudgetLedger::open(LedgerOpen {
                policy: Arc::new(policy),
                workspace: &self.workspace,
                explicit_profile: None,
                entry,
                clock: self.clock.clone(),
                command_writes: Some(self.writes()),
            })
        }

        fn file(&self) -> PathBuf {
            self.ledgers.join(WALLET).join("ledger.json")
        }

        fn on_disk(&self) -> Value {
            let envelope: Value = serde_json::from_slice(&fs::read(self.file()).unwrap()).unwrap();
            envelope["body"].clone()
        }

        fn handle(&self) -> Arc<RunBudgetHandle> {
            let handle = RunBudgetHandle::open_responses(LedgerOpenV2 {
                policy: self.policy(),
                workspace: &self.workspace,
                explicit_profile: None,
                entry: LedgerEntry::Fresh,
                clock: self.clock.clone(),
                command_writes: Some(self.writes()),
            })
            .unwrap()
            .with_sandbox_view(SandboxView {
                applied_on: Some(self.workspace.clone()),
            });
            Arc::new(handle)
        }
    }

    fn text_input() -> Value {
        json!([
            {"role": "system", "content": "be brief"},
            {"role": "user", "content": [{"type": "input_text", "text": "hi"}]},
            {"type": "message", "id": "msg_1", "status": "completed", "role": "assistant",
             "content": [{"type": "output_text", "text": "calling"}]},
            {"type": "function_call", "call_id": "c1", "name": "f", "arguments": "{}"},
            {"type": "function_call_output", "call_id": "c1", "output": "ok"}
        ])
    }

    fn opaque_input() -> Value {
        let mut input = text_input();
        input.as_array_mut().unwrap().push(json!(
            {"type": "reasoning", "id": "rs_1", "encrypted_content": "QUJD", "summary": []}
        ));
        input
    }

    fn body_value(input: Value, max_output: u64) -> Value {
        json!({
            "model": "m-1",
            "input": input,
            "max_output_tokens": max_output,
            "reasoning": {"effort": "high"},
            "store": false,
            "include": ["reasoning.encrypted_content"],
            "tools": [{"type": "function", "name": "f", "description": "d",
                       "parameters": {"type": "object"}, "strict": false}],
            "service_tier": "default"
        })
    }

    fn text_body() -> Vec<u8> {
        body_value(text_input(), 50).to_string().into_bytes()
    }

    fn opaque_body() -> Vec<u8> {
        body_value(opaque_input(), 50).to_string().into_bytes()
    }

    fn request(body: &[u8]) -> ResponsesReserveRequest<'_> {
        ResponsesReserveRequest {
            body,
            endpoint: ENDPOINT,
            purpose: SendPurpose::Executor,
            final_repair: None,
        }
    }

    /// A response of a declared model at the default tier: 100 in, 20 cached, 10 written for
    /// 30m, 5 for 12h, 30 out of which 10 reasoning.
    fn usage_value() -> Value {
        json!({
            "input_tokens": 100,
            "input_tokens_details": {
                "cached_tokens": 20,
                "cache_write_tokens": 15,
                "cache_write_tokens_details": {"ttl_30m_tokens": 10, "ttl_12h_tokens": 5}
            },
            "output_tokens": 30,
            "output_tokens_details": {"reasoning_tokens": 10},
            "total_tokens": 130
        })
    }

    fn response_with(change: impl FnOnce(&mut Value)) -> String {
        let mut value = json!({
            "id": "resp_1",
            "model": "m-1-snapshot-a",
            "service_tier": "default",
            "output": [],
            "usage": usage_value()
        });
        change(&mut value);
        value.to_string()
    }

    fn response() -> String {
        response_with(|_| {})
    }

    /// 65 ordinary at 2, 20 cached at 0.2, 10 at 2.5, 5 at 4, 30 out at 8, per million: 130 + 4 +
    /// 25 + 20 + 240 = 419 micro-dollars.
    const CHARGE_UNITS: u128 = 419 * UNIT / 1_000_000;

    fn decimal(units: u128) -> String {
        format!("{}.{:018}", units / UNIT, units % UNIT)
    }

    fn reserved_for(world: &World, body: &[u8]) -> Money {
        let policy = world.policy();
        let admitted = admit_body(&policy, body).unwrap();
        reservation_bound(&policy.tariff, admitted.input_bound, admitted.output_tokens).unwrap()
    }

    fn started(ledger: &mut BudgetLedgerV2, body: &[u8]) -> PhysicalId {
        let id = ledger.reserve(&request(body)).unwrap().physical;
        let _permit = ledger.start(id, body).unwrap();
        id
    }

    fn charged(ledger: &mut BudgetLedgerV2, body: &[u8]) -> PhysicalId {
        let id = started(ledger, body);
        assert!(matches!(
            ledger.settle_body(id, &response()).unwrap(),
            ResponsesSettle::Charged { .. }
        ));
        id
    }

    fn budget(refusal: BudgetRefusal) -> LedgerFailure {
        LedgerFailure::Budget(refusal)
    }

    // ---- admission and the input bound ----

    #[test]
    fn a_text_body_is_bounded_by_its_bytes_and_an_opaque_one_by_the_declared_most() {
        let world = World::new();
        let mut ledger = world.open();
        let text = text_body();
        let reservation = ledger.reserve(&request(&text)).unwrap();
        assert_eq!(
            reservation.input_bound_method,
            InputBoundMethod::BodyBytesPlusOverhead
        );
        assert_eq!(reservation.input_bound, text.len() as u64 + 4);
        ledger.release(reservation.physical).unwrap();
        let opaque = opaque_body();
        let reservation = ledger.reserve(&request(&opaque)).unwrap();
        assert_eq!(
            reservation.input_bound_method,
            InputBoundMethod::ProviderMaxInputTokens
        );
        assert_eq!(reservation.input_bound, 1_050_000);
        // On disk, as the ledger keeps it.
        let disk = world.on_disk();
        assert_eq!(
            disk["attempts"][0]["input_bound_method"],
            json!("body_bytes_plus_overhead")
        );
        assert_eq!(disk["attempts"][0]["input_bound"], json!(text.len() + 4));
        assert_eq!(
            disk["attempts"][1]["input_bound_method"],
            json!("provider_max_input_tokens")
        );
        assert_eq!(disk["attempts"][1]["input_bound"], json!(1_050_000));
        assert_eq!(
            disk["attempts"][1]["request_sha256"],
            json!(crate::planner::structured_stages::receipt::sha256_hex(
                &opaque
            ))
        );
    }

    #[test]
    fn an_opaque_body_with_no_declared_most_or_a_bound_past_the_most_sends_nothing() {
        let world = World::with(|v| {
            v["input_bound"]
                .as_object_mut()
                .unwrap()
                .remove("provider_max_input_tokens");
        });
        let mut ledger = world.open();
        let before = fs::read(world.file()).unwrap();
        assert_eq!(
            ledger.reserve(&request(&opaque_body())).map(|r| r.physical),
            Err(budget(BudgetRefusal::InputBoundUndeclared))
        );
        assert_eq!(fs::read(world.file()).unwrap(), before);
        // Text alone still goes.
        assert!(ledger.reserve(&request(&text_body())).is_ok());

        let world = World::with(|v| v["max_input_tokens"] = json!(1_049_999));
        let mut ledger = world.open();
        assert_eq!(
            ledger.reserve(&request(&opaque_body())).map(|r| r.physical),
            Err(budget(BudgetRefusal::InputBoundExceeded))
        );
        let text = text_body();
        let world = World::with(|v| v["max_input_tokens"] = json!(text.len() + 3));
        let mut ledger = world.open();
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::InputBoundExceeded))
        );
        let world = World::with(|v| v["max_input_tokens"] = json!(text.len() + 4));
        assert!(world.open().reserve(&request(&text)).is_ok());
    }

    #[test]
    fn a_body_the_route_does_not_allow_is_refused_before_anything_is_reserved() {
        let world = World::new();
        let policy = world.policy();
        type Change = fn(&mut Value);
        let rows: Vec<(&str, Change, BudgetRefusal)> = vec![
            (
                "prompt_cache_options",
                |v| v["prompt_cache_options"] = json!({}),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "context_management",
                |v| v["context_management"] = json!([]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "chat messages",
                |v| v["messages"] = json!([]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "stream",
                |v| v["stream"] = json!(false),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "previous_response_id",
                |v| v["previous_response_id"] = json!("r"),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "temperature",
                |v| v["temperature"] = json!(0),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "model",
                |v| v["model"] = json!("m-2"),
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "no model",
                |v| {
                    v.as_object_mut().unwrap().remove("model");
                },
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "effort",
                |v| v["reasoning"]["effort"] = json!("low"),
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "no reasoning",
                |v| {
                    v.as_object_mut().unwrap().remove("reasoning");
                },
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "reasoning summary",
                |v| v["reasoning"]["summary"] = json!("auto"),
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "auto tier",
                |v| v["service_tier"] = json!("auto"),
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "flex tier",
                |v| v["service_tier"] = json!("flex"),
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "no tier",
                |v| {
                    v.as_object_mut().unwrap().remove("service_tier");
                },
                BudgetRefusal::RouteNotAdmitted,
            ),
            (
                "stored",
                |v| v["store"] = json!(true),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "no store",
                |v| {
                    v.as_object_mut().unwrap().remove("store");
                },
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "include other",
                |v| v["include"] = json!(["file_search_call.results"]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "include twice",
                |v| {
                    v["include"] =
                        json!(["reasoning.encrypted_content", "reasoning.encrypted_content"])
                },
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "hosted tool",
                |v| v["tools"] = json!([{"type": "web_search"}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "mcp tool",
                |v| v["tools"] = json!([{"type": "mcp", "server_label": "x"}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "tool extra key",
                |v| v["tools"][0]["cache_control"] = json!({}),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "no output",
                |v| {
                    v.as_object_mut().unwrap().remove("max_output_tokens");
                },
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "zero output",
                |v| v["max_output_tokens"] = json!(0),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "string output",
                |v| v["max_output_tokens"] = json!("50"),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "output past the policy",
                |v| v["max_output_tokens"] = json!(51),
                BudgetRefusal::OutputBoundExceeded,
            ),
            (
                "string input",
                |v| v["input"] = json!("hi"),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "image part",
                |v| v["input"] = json!([{"role": "user", "content": [{"type": "input_image", "image_url": "x"}]}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "file part",
                |v| {
                    v["input"] = json!([{"role": "user", "content": [{"type": "input_file", "file_id": "f"}]}])
                },
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "unknown item",
                |v| v["input"] = json!([{"type": "file_search_call", "id": "x"}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "item reference",
                |v| v["input"] = json!([{"type": "item_reference", "id": "x"}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "tool role",
                |v| v["input"] = json!([{"role": "tool", "content": "x"}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "message extra key",
                |v| v["input"] = json!([{"role": "user", "content": "x", "cache_control": {}}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
            (
                "message no content",
                |v| v["input"] = json!([{"role": "user"}]),
                BudgetRefusal::BodyNotAdmitted,
            ),
        ];
        let mut ledger = world.open();
        let before = fs::read(world.file()).unwrap();
        for (name, change, expected) in rows {
            let mut value = body_value(text_input(), 50);
            change(&mut value);
            let bytes = value.to_string().into_bytes();
            assert_eq!(admit_body(&policy, &bytes), Err(expected), "{name}");
            assert_eq!(
                ledger.reserve(&request(&bytes)).map(|r| r.physical),
                Err(budget(expected)),
                "{name}"
            );
        }
        // A key twice, at the top, in reasoning, in an item.
        let text = String::from_utf8(text_body()).unwrap();
        for (once, twice) in [
            (r#""store":false"#, r#""store":false,"store":false"#),
            (r#""effort":"high""#, r#""effort":"high","effort":"high""#),
            (
                r#""call_id":"c1","name""#,
                r#""call_id":"c1","call_id":"c2","name""#,
            ),
        ] {
            assert_eq!(text.matches(once).count(), 1, "{once}");
            let doubled = text.replace(once, twice);
            assert!(admit_body(&policy, doubled.as_bytes()).is_err(), "{twice}");
            assert!(
                ledger.reserve(&request(doubled.as_bytes())).is_err(),
                "{twice}"
            );
        }
        // Another endpoint than the policy's.
        let bytes = text_body();
        for endpoint in [
            "https://api.example.test/v1/chat/completions",
            "https://other.test/v1/responses",
            "",
        ] {
            assert_eq!(
                ledger
                    .reserve(&ResponsesReserveRequest {
                        endpoint,
                        ..request(&bytes)
                    })
                    .map(|r| r.physical),
                Err(budget(BudgetRefusal::RouteNotAdmitted)),
                "{endpoint}"
            );
        }
        assert_eq!(fs::read(world.file()).unwrap(), before);
        assert!(ledger.wallet().attempts.is_empty());
        // The smallest body passes: no tools, no include.
        let mut value = body_value(json!([]), 1);
        value.as_object_mut().unwrap().remove("tools");
        value.as_object_mut().unwrap().remove("include");
        assert!(admit_body(&policy, value.to_string().as_bytes()).is_ok());
    }

    // ---- the limits ----

    #[test]
    fn exactly_the_cost_cap_passes_and_one_unit_less_does_not() {
        let text = text_body();
        let probe = World::new();
        let amount = reserved_for(&probe, &text).units();
        let world = World::with(|v| v["limits"]["cost_usd"] = json!(decimal(amount)));
        assert!(world.open().reserve(&request(&text)).is_ok());
        let world = World::with(|v| v["limits"]["cost_usd"] = json!(decimal(amount - 1)));
        let mut ledger = world.open();
        let before = fs::read(world.file()).unwrap();
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::CostExceeded))
        );
        assert_eq!(fs::read(world.file()).unwrap(), before);
    }

    #[test]
    fn spent_and_held_count_toward_the_cap_with_the_next_request() {
        // One charged send leaves the cap at exactly what the next reservation needs.
        let text = text_body();
        let amount = reserved_for(&World::new(), &text).units();
        let world =
            World::with(|v| v["limits"]["cost_usd"] = json!(decimal(CHARGE_UNITS + amount)));
        let mut ledger = world.open();
        charged(&mut ledger, &text);
        assert_eq!(ledger.wallet().spent().unwrap().units(), CHARGE_UNITS);
        assert!(ledger.reserve(&request(&text)).is_ok());
        let world =
            World::with(|v| v["limits"]["cost_usd"] = json!(decimal(CHARGE_UNITS + amount - 1)));
        let mut ledger = world.open();
        charged(&mut ledger, &text);
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::CostExceeded))
        );
    }

    #[test]
    fn the_opaque_reservation_is_larger_and_a_budget_for_text_alone_stops_the_replay() {
        let text = text_body();
        let opaque = opaque_body();
        let probe = World::new();
        let small = reserved_for(&probe, &text).units();
        let large = reserved_for(&probe, &opaque).units();
        assert!(large > small);
        // 1050000 is past the 272000 boundary: the high tier's 8 and 12 per million.
        assert_eq!(large, (1_050_000 * 8 + 50 * 12) * UNIT / 1_000_000);
        // Enough for the first send and its charge, not for a replay of its reasoning.
        let world =
            World::with(|v| v["limits"]["cost_usd"] = json!(decimal(CHARGE_UNITS + large - 1)));
        let mut ledger = world.open();
        charged(&mut ledger, &text);
        assert_eq!(
            ledger.reserve(&request(&opaque)).map(|r| r.physical),
            Err(budget(BudgetRefusal::CostExceeded))
        );
        // Enough for it, the same rule lets it go.
        let world = World::with(|v| v["limits"]["cost_usd"] = json!(decimal(CHARGE_UNITS + large)));
        let mut ledger = world.open();
        charged(&mut ledger, &text);
        assert!(ledger.reserve(&request(&opaque)).is_ok());
    }

    #[test]
    fn the_token_physical_logical_and_wall_limits_hold_at_their_edges() {
        let text = text_body();
        let tokens = text.len() as u64 + 4 + 50;
        let world = World::with(|v| v["limits"]["tokens"] = json!(tokens));
        assert!(world.open().reserve(&request(&text)).is_ok());
        let world = World::with(|v| v["limits"]["tokens"] = json!(tokens - 1));
        assert_eq!(
            world.open().reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::TokensExceeded))
        );

        let world = World::with(|v| v["limits"]["physical_sends"] = json!(1));
        let mut ledger = world.open();
        charged(&mut ledger, &text);
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::PhysicalExceeded))
        );

        let world = World::with(|v| {
            v["limits"]["logical_global"] = json!(2);
            v["limits"]["logical_per_stage"] = json!(1);
        });
        let mut ledger = world.open();
        let stage = || LogicalKind::Stage {
            stage: "s".to_string(),
        };
        ledger.record_submission(stage(), None).unwrap();
        assert_eq!(
            ledger.record_submission(stage(), None),
            Err(budget(BudgetRefusal::LogicalStageExceeded))
        );
        ledger
            .record_submission(LogicalKind::General, None)
            .unwrap();
        assert_eq!(
            ledger.record_submission(LogicalKind::General, None),
            Err(budget(BudgetRefusal::LogicalGlobalExceeded))
        );

        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(10));
        let mut ledger = world.open();
        charged(&mut ledger, &text);
        world.clock.advance(10_000);
        let id = ledger.reserve(&request(&text)).unwrap().physical;
        ledger.release(id).unwrap();
        world.clock.advance(1);
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::WallExceeded))
        );
    }

    #[test]
    fn a_start_past_the_deadline_is_refused_and_a_response_past_it_is_charged_and_latched() {
        let text = text_body();
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(10));
        let mut ledger = world.open();
        let id = ledger.reserve(&request(&text)).unwrap().physical;
        world.clock.advance(10_001);
        assert_eq!(
            ledger.start(id, &text).map(|permit| permit.physical()),
            Err(budget(BudgetRefusal::WallExceeded))
        );
        let world = World::with(|v| v["limits"]["wall_seconds"] = json!(10));
        let mut ledger = world.open();
        let id = started(&mut ledger, &text);
        world.clock.advance(10_001);
        assert!(matches!(
            ledger.settle_body(id, &response()).unwrap(),
            ResponsesSettle::Charged { latched: true, .. }
        ));
        assert_eq!(
            ledger.wallet().latch().unwrap().reason,
            LatchReason::ResponseAfterDeadline
        );
        assert_eq!(ledger.wallet().spent().unwrap().units(), CHARGE_UNITS);
    }

    #[test]
    fn final_repairs_are_counted_and_held_to_their_window() {
        let text = text_body();
        let world = World::with(|v| {
            v["limits"]["final_repair_attempts"] = json!(1);
            v["limits"]["final_repair_seconds"] = json!(5);
        });
        let mut ledger = world.open();
        let repair = |claim| ResponsesReserveRequest {
            purpose: SendPurpose::FinalRepair,
            final_repair: Some(claim),
            ..request(&text)
        };
        let first = ledger.reserve(&repair(FinalRepairClaim::New)).unwrap();
        let id = first.final_repair.unwrap();
        ledger.release(first.physical).unwrap();
        assert_eq!(
            ledger
                .reserve(&repair(FinalRepairClaim::New))
                .map(|r| r.physical),
            Err(budget(BudgetRefusal::FinalRepairAttemptsExceeded))
        );
        let again = ledger
            .reserve(&repair(FinalRepairClaim::Continue(id)))
            .unwrap();
        ledger.release(again.physical).unwrap();
        world.clock.advance(5_001);
        assert_eq!(
            ledger
                .reserve(&repair(FinalRepairClaim::Continue(id)))
                .map(|r| r.physical),
            Err(budget(BudgetRefusal::FinalRepairWallExceeded))
        );
        // A final repair claim needs its purpose, both ways.
        assert_eq!(
            ledger
                .reserve(&ResponsesReserveRequest {
                    final_repair: Some(FinalRepairClaim::New),
                    ..request(&text)
                })
                .map(|r| r.physical),
            Err(budget(BudgetRefusal::PurposeDiffers))
        );
    }

    // ---- settlement: charged, unknown, late, latched ----

    #[test]
    fn a_complete_usage_is_charged_by_the_tariff_and_nothing_is_billed() {
        let world = World::new();
        let mut ledger = world.open();
        let text = text_body();
        let id = started(&mut ledger, &text);
        let ResponsesSettle::Charged {
            response,
            charged,
            latched,
        } = ledger.settle_body(id, &response()).unwrap()
        else {
            panic!("not charged");
        };
        assert_eq!(response.physical(), id);
        assert_eq!(charged.units(), CHARGE_UNITS);
        assert!(!latched);
        let attempt = ledger.wallet().attempt(id).unwrap();
        assert_eq!(attempt.state, AttemptState::Settled);
        assert_eq!(attempt.provider_billed, ProviderBilled::Unreported);
        let record = attempt.charged.unwrap();
        assert_eq!(record.tier, 0);
        assert_eq!(record.usage.cache_write_30m_tokens, Some(10));
        assert!(!record.late);
        assert_eq!(ledger.wallet().spent().unwrap().units(), CHARGE_UNITS);
        assert_eq!(ledger.wallet().spent_tokens(), 130);
        assert_eq!(ledger.success_gate(), Ok(()));
        // On disk: the charge apart, and nothing that reads as a billed amount.
        let disk = world.on_disk();
        let attempt = &disk["attempts"][0];
        assert_eq!(attempt["provider_billed"], json!("unreported"));
        assert_eq!(
            attempt["charged"]["charged"],
            json!(CHARGE_UNITS.to_string())
        );
        let text = disk.to_string();
        for billed in [
            "billed_usd",
            "provider_billed_cost_usd",
            "\"billed\"",
            "consumed",
        ] {
            assert!(!text.contains(billed), "{billed}");
        }
        // Its identity is what a submission of its calls takes, once per call.
        ledger
            .record_submission(LogicalKind::General, Some(response.call(0)))
            .unwrap();
        ledger
            .record_submission(LogicalKind::General, Some(response.call(1)))
            .unwrap();
        assert_eq!(
            ledger.record_submission(LogicalKind::General, Some(response.call(1))),
            Err(budget(BudgetRefusal::DuplicateSubmission))
        );
        assert_eq!(ledger.wallet().physical_used(), 1);
    }

    #[test]
    fn every_unknown_response_keeps_the_reservation_and_stops_the_wallet() {
        type Change = fn(&mut Value);
        let rows: Vec<(&str, Change, UnknownCause, bool)> = vec![
            (
                "no usage",
                |v| {
                    v.as_object_mut().unwrap().remove("usage");
                },
                UnknownCause::Usage {
                    stop: UsageStop::UsageMissing,
                },
                false,
            ),
            (
                "null usage",
                |v| v["usage"] = json!(null),
                UnknownCause::Usage {
                    stop: UsageStop::UsageNotObject,
                },
                false,
            ),
            (
                "cost in usage",
                |v| v["usage"]["cost"] = json!(0.001),
                UnknownCause::Usage {
                    stop: UsageStop::UnknownField,
                },
                true,
            ),
            (
                "total only",
                |v| {
                    v["usage"]["input_tokens_details"]
                        .as_object_mut()
                        .unwrap()
                        .remove("cache_write_tokens_details");
                },
                UnknownCause::Charge {
                    stop: ChargeStop::CacheWriteFieldMissing,
                },
                true,
            ),
            (
                "zero input",
                |v| {
                    v["usage"] = json!({"input_tokens": 0, "input_tokens_details": {"cached_tokens": 0, "cache_write_tokens": 0,
                    "cache_write_tokens_details": {"ttl_30m_tokens": 0, "ttl_12h_tokens": 0}}, "output_tokens": 0});
                },
                UnknownCause::Charge {
                    stop: ChargeStop::InputZero,
                },
                true,
            ),
            (
                "no model",
                |v| {
                    v.as_object_mut().unwrap().remove("model");
                },
                UnknownCause::Response {
                    stop: ResponseStop::ModelMissing,
                },
                true,
            ),
            (
                "another model",
                |v| v["model"] = json!("m-1-snapshot"),
                UnknownCause::Response {
                    stop: ResponseStop::ModelNotDeclared,
                },
                true,
            ),
            (
                "no tier",
                |v| {
                    v.as_object_mut().unwrap().remove("service_tier");
                },
                UnknownCause::Response {
                    stop: ResponseStop::ServiceTierMissing,
                },
                true,
            ),
            (
                "null tier",
                |v| v["service_tier"] = json!(null),
                UnknownCause::Response {
                    stop: ResponseStop::ServiceTierNotDefault,
                },
                true,
            ),
            (
                "flex tier",
                |v| v["service_tier"] = json!("flex"),
                UnknownCause::Response {
                    stop: ResponseStop::ServiceTierNotDefault,
                },
                true,
            ),
        ];
        let text = text_body();
        for (name, change, cause, observed) in rows {
            let world = World::new();
            let mut ledger = world.open();
            let id = started(&mut ledger, &text);
            let reserved = ledger.wallet().attempt(id).unwrap().reserved;
            assert_eq!(
                ledger.settle_body(id, &response_with(change)).unwrap(),
                ResponsesSettle::Unknown(cause),
                "{name}"
            );
            let attempt = ledger.wallet().attempt(id).unwrap();
            assert_eq!(attempt.state, AttemptState::Unknown, "{name}");
            assert_eq!(attempt.unknown, Some(cause), "{name}");
            assert_eq!(attempt.observed.is_some(), observed, "{name}");
            assert_eq!(attempt.charged, None, "{name}");
            assert_eq!(ledger.wallet().held().unwrap(), reserved, "{name}");
            assert_eq!(ledger.wallet().spent().unwrap(), Money::ZERO, "{name}");
            assert_eq!(
                ledger.reserve(&request(&text)).map(|r| r.physical),
                Err(budget(BudgetRefusal::Latched)),
                "{name}"
            );
            assert_eq!(
                ledger.success_gate(),
                Err(budget(BudgetRefusal::Latched)),
                "{name}"
            );
            // Nothing of it is a source of a call.
            assert_eq!(
                ledger.record_submission(
                    LogicalKind::General,
                    Some(super::super::identity::ResponseIdentity::of_settled(id).call(0))
                ),
                Err(budget(BudgetRefusal::Latched)),
                "{name}"
            );
            // The next process reads it as it was.
            drop(ledger);
            let ledger = world.open();
            assert_eq!(
                ledger.wallet().attempt(id).unwrap().unknown,
                Some(cause),
                "{name}"
            );
        }
    }

    #[test]
    fn a_none_mode_wallet_charges_no_write_and_stops_on_a_written_one() {
        let world = World::of(policy_v2_none_value(), |_| {});
        let mut ledger = world.open();
        let text = text_body();
        let id = started(&mut ledger, &text);
        let body = response_with(|v| {
            let details = v["usage"]["input_tokens_details"].as_object_mut().unwrap();
            details.remove("cache_write_tokens");
            details.remove("cache_write_tokens_details");
        });
        // 80 ordinary at 2, 20 cached at 0.2, 30 out at 8: 160 + 4 + 240 = 404 micro-dollars.
        assert_eq!(
            ledger.settle_body(id, &body).unwrap(),
            ResponsesSettle::Charged {
                response: super::super::identity::ResponseIdentity::of_settled(id),
                charged: Money::parse_decimal_up("0.000404").unwrap(),
                latched: false,
            }
        );
        let id = started(&mut ledger, &text);
        assert_eq!(
            ledger.settle_body(id, &response()).unwrap(),
            ResponsesSettle::Unknown(UnknownCause::Charge {
                stop: ChargeStop::CacheWriteInNoneMode
            })
        );
    }

    #[test]
    fn a_transport_failure_is_unknown_and_only_a_send_not_started_goes_back() {
        let text = text_body();
        for reason in [
            UnknownReason::Timeout,
            UnknownReason::Cancelled,
            UnknownReason::HttpError,
            UnknownReason::BodyTruncated,
            UnknownReason::SendError,
        ] {
            let world = World::new();
            let mut ledger = world.open();
            let id = started(&mut ledger, &text);
            ledger.mark_unknown(id, reason).unwrap();
            let attempt = ledger.wallet().attempt(id).unwrap();
            assert_eq!(attempt.unknown, Some(UnknownCause::Transport { reason }));
            assert_eq!(ledger.wallet().held().unwrap(), attempt.reserved);
            // A started send does not go back.
            assert!(ledger.release(id).is_err(), "{reason:?}");
            assert_eq!(
                ledger.reserve(&request(&text)).map(|r| r.physical),
                Err(budget(BudgetRefusal::Latched))
            );
        }
        // A reservation that never left does.
        let world = World::new();
        let mut ledger = world.open();
        let id = ledger.reserve(&request(&text)).unwrap().physical;
        ledger.release(id).unwrap();
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
        assert_eq!(ledger.wallet().physical_used(), 0);
        assert_eq!(ledger.success_gate(), Ok(()));
    }

    #[test]
    fn a_late_valid_usage_is_charged_once_and_the_latch_stays() {
        let world = World::new();
        let mut ledger = world.open();
        let text = text_body();
        let id = started(&mut ledger, &text);
        ledger.mark_unknown(id, UnknownReason::Timeout).unwrap();
        let ResponsesSettle::Charged { latched, .. } = ledger.settle_body(id, &response()).unwrap()
        else {
            panic!("not charged");
        };
        assert!(latched);
        let attempt = ledger.wallet().attempt(id).unwrap();
        assert!(attempt.charged.unwrap().late);
        assert_eq!(ledger.wallet().spent().unwrap().units(), CHARGE_UNITS);
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
        assert_eq!(
            ledger.wallet().latch().unwrap().reason,
            LatchReason::UnknownBilling
        );
        // Once: the same response again is no second charge.
        assert!(matches!(
            ledger.settle_body(id, &response()),
            Err(LedgerFailure::Budget(
                BudgetRefusal::IllegalTransition { .. }
            ))
        ));
        assert_eq!(ledger.wallet().spent().unwrap().units(), CHARGE_UNITS);
        assert_eq!(ledger.success_gate(), Err(budget(BudgetRefusal::Latched)));
        // A late usage that is not known changes nothing of an unknown send.
        let world = World::new();
        let mut ledger = world.open();
        let id = started(&mut ledger, &text);
        ledger.mark_unknown(id, UnknownReason::Timeout).unwrap();
        let before = ledger.wallet().clone();
        assert!(matches!(
            ledger.settle_body(id, "{}").unwrap(),
            ResponsesSettle::Unknown(_)
        ));
        assert_eq!(ledger.wallet(), &before);
        // And the latch stays through the next process.
        drop(ledger);
        assert_eq!(
            world.open().reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::Latched))
        );
    }

    #[test]
    fn a_usage_past_its_bounds_or_its_reservation_is_charged_and_stops_the_wallet() {
        let text = text_body();
        let bound = text.len() as u64 + 4;
        for (name, input, output, latch) in [
            ("input", bound + 1, 30, LatchReason::InputOverBound),
            ("output", 100, 51, LatchReason::OutputOverBound),
        ] {
            let world = World::new();
            let mut ledger = world.open();
            let id = started(&mut ledger, &text);
            let body = response_with(|v| {
                v["usage"]["input_tokens"] = json!(input);
                v["usage"]["output_tokens"] = json!(output);
                v["usage"]["total_tokens"] = json!(input + output);
            });
            assert!(
                matches!(
                    ledger.settle_body(id, &body).unwrap(),
                    ResponsesSettle::Charged { latched: true, .. }
                ),
                "{name}"
            );
            assert_eq!(ledger.wallet().latch().unwrap().reason, latch, "{name}");
            assert!(ledger.wallet().spent().unwrap() > Money::ZERO, "{name}");
        }
        // A charge past the reservation is charged at its value, and its latch is the first.
        let world = World::new();
        let mut ledger = world.open();
        let id = started(&mut ledger, &text);
        let reserved = ledger.wallet().attempt(id).unwrap().reserved;
        let input = 10 * bound;
        let body = response_with(|v| {
            v["usage"] = json!({
                "input_tokens": input,
                "input_tokens_details": {"cached_tokens": 0, "cache_write_tokens": input,
                    "cache_write_tokens_details": {"ttl_30m_tokens": 0, "ttl_12h_tokens": input}},
                "output_tokens": 30
            });
        });
        let ResponsesSettle::Charged {
            charged, latched, ..
        } = ledger.settle_body(id, &body).unwrap()
        else {
            panic!("not charged");
        };
        assert!(charged > reserved);
        assert!(latched);
        assert_eq!(ledger.wallet().spent().unwrap(), charged);
        assert_eq!(
            ledger.wallet().latch().unwrap().reason,
            LatchReason::OverReservation
        );
    }

    /// Review 1, should 1: a usage stopped for one unknown non-zero meter keeps the counts it
    /// could validate, and the fixed reason, on disk; never the meter or the body.
    #[test]
    fn a_usage_stopped_for_an_unknown_meter_keeps_its_validated_counts() {
        let world = World::new();
        let mut ledger = world.open();
        let text = text_body();
        let id = started(&mut ledger, &text);
        let reserved = ledger.wallet().attempt(id).unwrap().reserved;
        let body = response_with(|v| v["usage"]["future_meter"] = json!(1));
        assert_eq!(
            ledger.settle_body(id, &body).unwrap(),
            ResponsesSettle::Unknown(UnknownCause::Usage {
                stop: UsageStop::UnknownField
            })
        );
        assert_eq!(ledger.wallet().held().unwrap(), reserved);
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(budget(BudgetRefusal::Latched))
        );
        let disk = world.on_disk();
        let attempt = &disk["attempts"][0];
        assert_eq!(attempt["state"], json!("unknown"));
        assert_eq!(
            attempt["unknown"],
            json!({"kind": "usage", "stop": "unknown_field"})
        );
        let observed = &attempt["observed"];
        assert_eq!(observed["input_tokens"], json!(100));
        assert_eq!(observed["output_tokens"], json!(30));
        assert_eq!(observed["cached_tokens"], json!(20));
        assert_eq!(observed["cache_write_tokens"], json!(15));
        assert_eq!(observed["cache_write_30m_tokens"], json!(10));
        assert_eq!(observed["cache_write_12h_tokens"], json!(5));
        assert_eq!(attempt["charged"], json!(null));
        let text = disk.to_string();
        assert!(!text.contains("future_meter"));
        assert!(!text.contains("resp_1"));
        // A stop for counts that do not hold together keeps nothing: a partial observation is not
        // one.
        let world = World::new();
        let mut ledger = world.open();
        let id = started(&mut ledger, &text_body());
        let body =
            response_with(|v| v["usage"]["output_tokens_details"]["reasoning_tokens"] = json!(31));
        assert!(matches!(
            ledger.settle_body(id, &body).unwrap(),
            ResponsesSettle::Unknown(UnknownCause::Usage { .. })
        ));
        assert_eq!(ledger.wallet().attempt(id).unwrap().observed, None);
    }

    /// Review 1, should 2: an item of a known type is admitted only in the shape the wire uses.
    #[test]
    fn an_item_of_a_known_type_in_another_shape_is_refused_before_anything_is_reserved() {
        let world = World::new();
        let policy = world.policy();
        let mut ledger = world.open();
        let before = fs::read(world.file()).unwrap();
        type Change = fn(&mut Value);
        let rows: Vec<(&str, Change)> = vec![
            ("file output", |v| {
                v["input"][4]["output"] = json!([{"type": "input_file", "file_id": "f"}])
            }),
            ("image output", |v| {
                v["input"][4]["output"] = json!([{"type": "input_image", "image_url": "x"}])
            }),
            ("number output", |v| v["input"][4]["output"] = json!(1)),
            ("output extra key", |v| v["input"][4]["extra"] = json!("x")),
            ("output no call id", |v| {
                v["input"][4].as_object_mut().unwrap().remove("call_id");
            }),
            ("text not a string", |v| {
                v["input"][1]["content"][0]["text"] = json!(1)
            }),
            ("text part extra key", |v| {
                v["input"][1]["content"][0]["cache_control"] = json!({})
            }),
            ("text part no text", |v| {
                v["input"][1]["content"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("text");
            }),
            ("annotation", |v| {
                v["input"][2]["content"][0]["annotations"] = json!([{"type": "url_citation"}])
            }),
            ("call extra key", |v| v["input"][3]["extra"] = json!("x")),
            ("call arguments object", |v| {
                v["input"][3]["arguments"] = json!({})
            }),
            ("call no name", |v| {
                v["input"][3].as_object_mut().unwrap().remove("name");
            }),
            ("call id number", |v| v["input"][3]["call_id"] = json!(1)),
            ("message id number", |v| v["input"][2]["id"] = json!(1)),
            ("content number", |v| v["input"][0]["content"] = json!(1)),
            ("reasoning extra key", |v| {
                v["input"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"type": "reasoning", "id": "rs", "summary": [], "extra": 1}));
            }),
            ("reasoning content image", |v| {
                v["input"].as_array_mut().unwrap().push(
                    json!({"type": "reasoning", "id": "rs", "summary": [], "encrypted_content": 1}),
                );
            }),
        ];
        for (name, change) in rows {
            let mut value = body_value(text_input(), 50);
            change(&mut value);
            let bytes = value.to_string().into_bytes();
            assert_eq!(
                admit_body(&policy, &bytes),
                Err(BudgetRefusal::BodyNotAdmitted),
                "{name}"
            );
            assert_eq!(
                ledger.reserve(&request(&bytes)).map(|r| r.physical),
                Err(budget(BudgetRefusal::BodyNotAdmitted)),
                "{name}"
            );
        }
        assert_eq!(fs::read(world.file()).unwrap(), before);
        assert!(ledger.wallet().attempts.is_empty());
        // The control: a replay as #1005 makes it, raw function and reasoning items with their
        // ids and statuses, and a message as the API returned it.
        let replay = json!([
            {"role": "user", "content": [{"type": "input_text", "text": "hi"}]},
            {"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": "s"}],
             "encrypted_content": "QUJD"},
            {"type": "function_call", "id": "fc_1", "call_id": "c1", "name": "f",
             "arguments": "{}", "status": "completed"},
            {"type": "message", "id": "msg_1", "status": "completed", "role": "assistant",
             "content": [{"type": "output_text", "text": "t", "annotations": [], "logprobs": []}]},
            {"type": "function_call_output", "call_id": "c1", "output": "ok"}
        ]);
        let bytes = body_value(replay, 50).to_string().into_bytes();
        assert_eq!(
            admit_body(&policy, &bytes).map(|body| body.input_bound_method),
            Ok(InputBoundMethod::ProviderMaxInputTokens)
        );
        assert!(ledger.reserve(&request(&bytes)).is_ok());
    }

    /// Review 2 S2, PM decision: shapes the client never builds stay refused, and the items as
    /// #1005's replay tests hold them (`tests/profile_tool_schema_responses.rs` `message`,
    /// `function_call`, `reasoning`) are admitted.
    #[test]
    fn what_the_builder_never_writes_is_refused_and_1005s_replayed_items_are_admitted() {
        let world = World::new();
        let policy = world.policy();
        type Change = fn(&mut Value);
        let rows: Vec<(&str, Change)> = vec![
            ("breakpoint", |v| {
                v["input"][1]["content"][0]["prompt_cache_breakpoint"] = json!({"mode": "explicit"})
            }),
            ("output id", |v| v["input"][4]["id"] = json!("fco_1")),
            ("output caller", |v| {
                v["input"][4]["caller"] = json!({"type": "direct"})
            }),
            ("output name", |v| v["input"][4]["name"] = json!("f")),
            ("output namespace", |v| {
                v["input"][4]["namespace"] = json!("n")
            }),
            ("output status", |v| {
                v["input"][4]["status"] = json!("completed")
            }),
            ("input refusal part", |v| {
                v["input"][1]["content"] = json!([{"type": "refusal", "refusal": "no"}])
            }),
            ("input phase other", |v| {
                v["input"][1]["phase"] = json!("analysis")
            }),
            ("typed user message", |v| {
                v["input"][1]["type"] = json!("message")
            }),
            ("input_text in a reply's message", |v| {
                v["input"][2]["content"][0]["type"] = json!("input_text")
            }),
        ];
        for (name, change) in rows {
            let mut value = body_value(text_input(), 50);
            change(&mut value);
            assert_eq!(
                admit_body(&policy, value.to_string().as_bytes()),
                Err(BudgetRefusal::BodyNotAdmitted),
                "{name}"
            );
        }
        // An input message may carry a documented phase.
        let mut value = body_value(text_input(), 50);
        value["input"][1]["phase"] = json!("commentary");
        assert!(admit_body(&policy, value.to_string().as_bytes()).is_ok());
        // #1005's items, as its replay tests write them.
        let replay = json!([
            {"role": "user", "content": [{"type": "input_text", "text": "hi"}]},
            {"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": "s"}],
             "encrypted_content": "dummy-encrypted"},
            {"type": "function_call", "id": "fc_1", "call_id": "c1", "name": "f",
             "arguments": "{}", "status": "completed"},
            {"type": "message", "id": "msg_1", "role": "assistant", "status": "completed",
             "content": [{"type": "output_text", "text": "t", "annotations": []}]},
            {"type": "function_call_output", "call_id": "c1", "output": "ok"}
        ]);
        assert_eq!(
            admit_body(&policy, body_value(replay, 50).to_string().as_bytes())
                .map(|body| body.input_bound_method),
            Ok(InputBoundMethod::ProviderMaxInputTokens)
        );
    }

    /// F5: a tool is admitted only as a named function; either part missing alone refuses it.
    #[test]
    fn a_tool_is_refused_unless_it_is_a_named_function() {
        let world = World::new();
        let policy = world.policy();
        for (name, tool) in [
            (
                "named, not a function",
                json!({"type": "custom", "name": "x"}),
            ),
            ("a function with no name", json!({"type": "function"})),
        ] {
            let mut value = body_value(text_input(), 50);
            value["tools"] = json!([tool]);
            assert_eq!(
                admit_body(&policy, value.to_string().as_bytes()).map(|_| ()),
                Err(BudgetRefusal::BodyNotAdmitted),
                "{name}"
            );
        }
        let mut value = body_value(text_input(), 50);
        value["tools"] = json!([{"type": "function", "name": "x"}]);
        assert!(admit_body(&policy, value.to_string().as_bytes()).is_ok());
    }

    /// F5: a response's `model` or `service_tier` twice, each alone, is a repeated key.
    #[test]
    fn a_model_or_a_tier_twice_in_a_response_is_a_repeated_key() {
        let text = text_body();
        let once = response();
        for (name, body) in [
            (
                "model twice",
                once.replacen(
                    r#""model":"m-1-snapshot-a""#,
                    r#""model":"m-1-snapshot-a","model":"m-1-snapshot-a""#,
                    1,
                ),
            ),
            (
                "tier twice",
                once.replacen(
                    r#""service_tier":"default""#,
                    r#""service_tier":"default","service_tier":"default""#,
                    1,
                ),
            ),
        ] {
            assert_ne!(body, once, "{name}");
            let world = World::new();
            let mut ledger = world.open();
            let id = started(&mut ledger, &text);
            assert_eq!(
                ledger.settle_body(id, &body).unwrap(),
                ResponsesSettle::Unknown(UnknownCause::Response {
                    stop: ResponseStop::DuplicateKey
                }),
                "{name}"
            );
        }
    }

    /// F5: a charge that would carry what the wallet spent past counting leaves a started send
    /// unknown, its reservation held and the latch set, and changes nothing of one already
    /// unknown. The spent amount near the top of the range is set on the records directly: no
    /// reservation can be made beside it.
    #[test]
    fn a_charge_past_counting_leaves_the_send_unknown_once() {
        use super::super::clock::RunClock;
        use super::super::ledger_v2::{BudgetCharge, UsageCounts};
        let world = World::new();
        let policy = world.policy();
        let text = text_body();
        let mut wallet =
            super::super::ledger_v2::WalletStateV2::new(&policy, "w".to_string(), None);
        let mut clock = RunClock::new(world.clock.clone());
        let first = wallet
            .reserve(&policy, &request(&text), &mut clock)
            .unwrap()
            .physical;
        wallet.start(&policy, first, &text).unwrap();
        wallet.settle_body(&policy, first, &response()).unwrap();
        // What was spent, now the largest amount there is.
        let usage = UsageCounts::of(
            &crate::providers::usage_observation::read_responses_usage(&response())
                .wallet
                .unwrap(),
        );
        wallet.attempts[0].charged = Some(BudgetCharge {
            usage,
            tier: 0,
            charged: Money::from_units(u128::MAX),
            late: false,
        });
        // A second send started beside it, as the records would hold it.
        let mut started_one = wallet.attempts[0].clone();
        let second = PhysicalId(2);
        started_one.physical = second;
        started_one.state = AttemptState::Started;
        started_one.charged = None;
        wallet.attempts.push(started_one);
        let overflow = ResponsesSettle::Unknown(UnknownCause::Charge {
            stop: ChargeStop::Overflow,
        });
        assert_eq!(
            wallet.settle_body(&policy, second, &response()).unwrap(),
            overflow
        );
        let attempt = wallet.attempt(second).unwrap();
        assert_eq!(attempt.state, AttemptState::Unknown);
        assert_eq!(
            attempt.unknown,
            Some(UnknownCause::Charge {
                stop: ChargeStop::Overflow
            })
        );
        assert_eq!(attempt.observed, Some(usage));
        assert_eq!(attempt.charged, None);
        assert_eq!(wallet.latch().unwrap().reason, LatchReason::UnknownBilling);
        let before = wallet.clone();
        // Late, from unknown: the same overflow, and nothing changes.
        assert_eq!(
            wallet.settle_body(&policy, second, &response()).unwrap(),
            overflow
        );
        assert_eq!(wallet, before);
    }

    /// F5: a normal charge leaves no observation; a late one keeps what was observed when the
    /// send went unknown.
    #[test]
    fn a_late_charge_keeps_what_was_observed_and_a_normal_one_has_none() {
        let text = text_body();
        let world = World::new();
        let mut ledger = world.open();
        let id = started(&mut ledger, &text);
        ledger.settle_body(id, &response()).unwrap();
        assert_eq!(ledger.wallet().attempt(id).unwrap().observed, None);
        // Unknown for another model, with its usage observed; then the late valid reply.
        let world = World::new();
        let mut ledger = world.open();
        let id = started(&mut ledger, &text);
        let other = response_with(|v| v["model"] = json!("m-1-other"));
        assert!(matches!(
            ledger.settle_body(id, &other).unwrap(),
            ResponsesSettle::Unknown(UnknownCause::Response { .. })
        ));
        let observed = ledger.wallet().attempt(id).unwrap().observed;
        assert!(observed.is_some());
        assert!(matches!(
            ledger.settle_body(id, &response()).unwrap(),
            ResponsesSettle::Charged { .. }
        ));
        let attempt = ledger.wallet().attempt(id).unwrap();
        assert!(attempt.charged.unwrap().late);
        assert_eq!(attempt.observed, observed);
    }

    /// F5: each latch of a charge holds exactly at its bound and is set one past it.
    #[test]
    fn the_latches_of_a_charge_hold_exactly_at_their_bounds() {
        let text = text_body();
        let bound = text.len() as u64 + 4;
        // The whole bound written for 12h, the dearest input class, and the whole output: the
        // charge is exactly the reservation.
        let usage = |input: u64, cached: u64, written_12h: u64, output: u64| {
            move |v: &mut Value| {
                v["usage"] = json!({
                    "input_tokens": input,
                    "input_tokens_details": {"cached_tokens": cached, "cache_write_tokens": written_12h,
                        "cache_write_tokens_details": {"ttl_30m_tokens": 0, "ttl_12h_tokens": written_12h}},
                    "output_tokens": output
                });
            }
        };
        let settle = |change: &dyn Fn(&mut Value)| {
            let world = World::new();
            let mut ledger = world.open();
            let id = started(&mut ledger, &text);
            let reserved = ledger.wallet().attempt(id).unwrap().reserved;
            let ResponsesSettle::Charged { charged, .. } = ledger
                .settle_body(id, &response_with(|v| change(v)))
                .unwrap()
            else {
                panic!("not charged");
            };
            (
                charged,
                reserved,
                ledger.wallet().latch().map(|latch| latch.reason),
            )
        };
        let (charged, reserved, latch) = settle(&usage(bound, 0, bound, 50));
        assert_eq!(charged, reserved, "at the reservation");
        assert_eq!(latch, None, "at every bound");
        let (charged, reserved, latch) = settle(&usage(bound + 1, 0, bound + 1, 50));
        assert!(charged > reserved);
        assert_eq!(latch, Some(LatchReason::OverReservation));
        // Input and output at and past their bounds, at the cheapest class: no other latch.
        assert_eq!(settle(&usage(bound, bound, 0, 1)).2, None);
        assert_eq!(
            settle(&usage(bound + 1, bound + 1, 0, 1)).2,
            Some(LatchReason::InputOverBound)
        );
        assert_eq!(settle(&usage(10, 10, 0, 50)).2, None);
        assert_eq!(
            settle(&usage(10, 10, 0, 51)).2,
            Some(LatchReason::OutputOverBound)
        );
    }

    // ---- processes, versions, and the disk ----

    #[test]
    fn a_send_started_when_a_process_ended_is_unknown_on_reopen_and_keeps_its_reservation() {
        let world = World::new();
        let text = text_body();
        let mut first = world.open();
        let id = started(&mut first, &text);
        let reserved = first.wallet().attempt(id).unwrap().reserved;
        drop(first);
        for entry in [LedgerEntry::Resume, LedgerEntry::Fresh] {
            let mut ledger = world.open_with(world.policy(), entry).unwrap();
            let attempt = ledger.wallet().attempt(id).unwrap();
            assert_eq!(attempt.state, AttemptState::Unknown, "{entry:?}");
            assert_eq!(
                attempt.unknown,
                Some(UnknownCause::Transport {
                    reason: UnknownReason::InterruptedAfterStart
                })
            );
            assert_eq!(ledger.wallet().held().unwrap(), reserved);
            assert_eq!(
                ledger.wallet().latch().unwrap().reason,
                LatchReason::InterruptedAfterStart
            );
            assert_eq!(
                ledger.reserve(&request(&text)).map(|r| r.physical),
                Err(budget(BudgetRefusal::Latched))
            );
            assert_eq!(ledger.success_gate(), Err(budget(BudgetRefusal::Latched)));
        }
        let disk = world.on_disk();
        assert_eq!(disk["attempts"][0]["state"], json!("unknown"));
        assert_eq!(disk["attempts"].as_array().unwrap().len(), 1);
        // A late valid usage of it is charged once, and the latch stays.
        let mut ledger = world.open();
        assert!(matches!(
            ledger.settle_body(id, &response()).unwrap(),
            ResponsesSettle::Charged { latched: true, .. }
        ));
        assert_eq!(
            ledger.wallet().latch().unwrap().reason,
            LatchReason::InterruptedAfterStart
        );
    }

    #[test]
    fn a_reservation_left_by_a_process_goes_back_and_a_resume_with_no_ledger_is_refused() {
        let world = World::new();
        let text = text_body();
        let mut first = world.open();
        let id = first.reserve(&request(&text)).unwrap().physical;
        drop(first);
        let ledger = world.open();
        assert_eq!(
            ledger.wallet().attempt(id).unwrap().state,
            AttemptState::Released
        );
        assert_eq!(ledger.wallet().held().unwrap(), Money::ZERO);
        let fresh = World::new();
        assert_eq!(
            fresh
                .open_with(fresh.policy(), LedgerEntry::Resume)
                .map(|_| ()),
            Err(LedgerRefusal::Missing)
        );
        assert!(!fresh.file().exists());
    }

    #[test]
    fn a_v1_wallet_is_not_opened_as_v2_and_a_v2_wallet_is_not_opened_as_v1() {
        // v1 first.
        let world = World::new();
        drop(world.open_v1(LedgerEntry::Fresh).unwrap());
        let v1_bytes = fs::read(world.file()).unwrap();
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            assert_eq!(
                world.open_with(world.policy(), entry).map(|_| ()),
                Err(LedgerRefusal::AccountingBasisChanged),
                "{entry:?}"
            );
        }
        assert_eq!(fs::read(world.file()).unwrap(), v1_bytes);
        assert!(world.open_v1(LedgerEntry::Resume).is_ok());

        // v2 first, with something spent.
        let world = World::new();
        let mut ledger = world.open();
        charged(&mut ledger, &text_body());
        drop(ledger);
        let v2_bytes = fs::read(world.file()).unwrap();
        for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
            assert_eq!(
                world.open_v1(entry).map(|_| ()),
                Err(LedgerRefusal::AccountingBasisChanged),
                "{entry:?}"
            );
        }
        assert_eq!(fs::read(world.file()).unwrap(), v2_bytes);
        // What it spent is still spent.
        assert_eq!(world.open().wallet().spent().unwrap().units(), CHARGE_UNITS);
    }

    #[test]
    fn a_changed_declaration_or_mode_is_refused_and_no_balance_comes_back() {
        let world = World::new();
        let mut ledger = world.open();
        charged(&mut ledger, &text_body());
        drop(ledger);
        let bytes = fs::read(world.file()).unwrap();
        let mut none_mode = policy_v2_none_value();
        none_mode["ledger_dir"] = world.value["ledger_dir"].clone();
        let mut more_money = world.value.clone();
        more_money["limits"]["cost_usd"] = json!("2000");
        let spaced = format!("{} ", world.value);
        let spaced = Arc::new(RunBudgetPolicyV2::decode(spaced.as_bytes(), WALLET).unwrap());
        for (name, policy) in [
            ("none mode", World::policy_of(&none_mode)),
            ("more money", World::policy_of(&more_money)),
            ("same values, other bytes", spaced),
        ] {
            for entry in [LedgerEntry::Fresh, LedgerEntry::Resume] {
                assert_eq!(
                    world.open_with(policy.clone(), entry).map(|_| ()),
                    Err(LedgerRefusal::PolicyChanged),
                    "{name} {entry:?}"
                );
            }
            assert_eq!(fs::read(world.file()).unwrap(), bytes, "{name}");
        }
        let ledger = world.open();
        assert_eq!(ledger.wallet().spent().unwrap().units(), CHARGE_UNITS);
        assert_eq!(ledger.wallet().physical_used(), 1);
    }

    #[test]
    fn a_record_whose_charge_or_basis_disagrees_is_refused() {
        type Change = fn(&mut Value);
        let rows: Vec<(&str, Change, LedgerRefusal)> = vec![
            (
                "charge",
                |v| v["attempts"][0]["charged"]["charged"] = json!("1"),
                LedgerRefusal::Corrupt,
            ),
            (
                "tier",
                |v| v["attempts"][0]["charged"]["tier"] = json!(1),
                LedgerRefusal::Corrupt,
            ),
            (
                "usage",
                |v| v["attempts"][0]["charged"]["usage"]["input_tokens"] = json!(99),
                LedgerRefusal::Corrupt,
            ),
            (
                "reservation",
                |v| v["attempts"][0]["reserved"] = json!("1"),
                LedgerRefusal::Corrupt,
            ),
            (
                "method",
                |v| v["attempts"][0]["input_bound_method"] = json!("provider_max_input_tokens"),
                LedgerRefusal::Corrupt,
            ),
            (
                "billed",
                |v| v["attempts"][0]["provider_billed"] = json!("0"),
                LedgerRefusal::Corrupt,
            ),
            (
                "basis",
                |v| v["accounting_basis"] = json!("provider_billed"),
                LedgerRefusal::AccountingBasisChanged,
            ),
            (
                "schema",
                |v| v["schema"] = json!("run-budget.ledger/v1"),
                LedgerRefusal::AccountingBasisChanged,
            ),
        ];
        for (name, change, expected) in rows {
            let world = World::new();
            let mut ledger = world.open();
            charged(&mut ledger, &text_body());
            drop(ledger);
            let mut body = world.on_disk();
            change(&mut body);
            let body = body.to_string();
            let sha256 = crate::planner::structured_stages::receipt::sha256_hex(body.as_bytes());
            fs::write(
                world.file(),
                format!("{{\"sha256\":\"{sha256}\",\"body\":{body}}}"),
            )
            .unwrap();
            assert_eq!(
                world
                    .open_with(world.policy(), LedgerEntry::Fresh)
                    .map(|_| ()),
                Err(expected),
                "{name}"
            );
        }
    }

    #[test]
    fn a_change_that_cannot_reach_the_disk_sends_nothing_and_refuses_everything_after() {
        let world = World::new();
        let text = text_body();
        let mut ledger = world.open();
        let id = ledger.reserve(&request(&text)).unwrap().physical;
        let before = ledger.wallet().clone();
        let temporary = world.ledgers.join(WALLET).join("ledger.json.tmp");
        fs::create_dir(&temporary).unwrap();
        // The start does not reach the disk, so no permit is given: no POST.
        assert_eq!(
            ledger.start(id, &text).map(|permit| permit.physical()),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(ledger.wallet(), &before);
        fs::remove_dir(&temporary).unwrap();
        assert_eq!(
            ledger.start(id, &text).map(|permit| permit.physical()),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(
            ledger.reserve(&request(&text)).map(|r| r.physical),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(
            ledger.success_gate(),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        // The next process finds the reservation, which never left, and gives it back.
        drop(ledger);
        assert_eq!(world.open().wallet().held().unwrap(), Money::ZERO);
    }

    #[test]
    fn a_charge_that_cannot_reach_the_disk_gives_no_identity() {
        let world = World::new();
        let text = text_body();
        let mut ledger = world.open();
        let id = started(&mut ledger, &text);
        let before = ledger.wallet().clone();
        let temporary = world.ledgers.join(WALLET).join("ledger.json.tmp");
        fs::create_dir(&temporary).unwrap();
        assert_eq!(
            ledger.settle_body(id, &response()),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        assert_eq!(ledger.wallet(), &before);
        fs::remove_dir(&temporary).unwrap();
        assert_eq!(
            ledger.record_submission(
                LogicalKind::General,
                Some(super::super::identity::ResponseIdentity::of_settled(id).call(0))
            ),
            Err(LedgerFailure::Ledger(LedgerRefusal::Unavailable))
        );
        // The next process finds the send started, and so unknown, holding its reservation.
        drop(ledger);
        let ledger = world.open();
        let attempt = ledger.wallet().attempt(id).unwrap();
        assert_eq!(attempt.state, AttemptState::Unknown);
        assert_eq!(ledger.wallet().held().unwrap(), attempt.reserved);
    }

    // ---- through the run's handle ----

    #[test]
    fn a_handle_of_this_basis_puts_the_start_on_disk_before_a_request_exists() {
        let world = World::new();
        let handle = world.handle();
        let value = body_value(text_input(), 50);
        let budgeted =
            BudgetedBody::for_responses(&handle, &value, SendPurpose::Executor, ENDPOINT).unwrap();
        let client = reqwest::Client::new();
        let (request, send) = budgeted.begin(client.post(ENDPOINT)).unwrap();
        let disk = world.on_disk();
        assert_eq!(disk["attempts"][0]["state"], json!("started"));
        assert_eq!(
            disk["attempts"][0]["request_sha256"],
            json!(crate::planner::structured_stages::receipt::sha256_hex(
                budgeted.bytes()
            ))
        );
        let built = request.build().unwrap();
        assert_eq!(built.body().unwrap().as_bytes().unwrap(), budgeted.bytes());
        let response = settle_body_identified(&mut Some(send), &response()).unwrap();
        assert_eq!(response.map(|r| r.physical()), Some(PhysicalId(1)));
        assert_eq!(handle.success_gate(), Ok(()));
        let wallet = handle.responses_wallet().unwrap();
        assert_eq!(wallet.spent().unwrap().units(), CHARGE_UNITS);
        // A v1 body on this wallet, or a v1 view of it, is a send it cannot count.
        let chat = BudgetedBody::new(&handle, &value, SendPurpose::Executor).unwrap();
        let refused = SendRefusal::Wallet(budget(BudgetRefusal::RouteNotAdmitted));
        assert_eq!(
            chat.begin(client.post(ENDPOINT)).map(|_| ()).unwrap_err(),
            refused
        );
        assert_eq!(handle.wallet().map(|_| ()), Err(refused));
        assert_eq!(handle.responses_wallet().unwrap().attempts.len(), 1);
    }

    #[test]
    fn a_send_dropped_unresolved_is_unknown_and_the_next_try_gets_no_permit() {
        let world = World::new();
        let handle = world.handle();
        let value = body_value(text_input(), 50);
        let budgeted =
            BudgetedBody::for_responses(&handle, &value, SendPurpose::Executor, ENDPOINT).unwrap();
        let client = reqwest::Client::new();
        let (_request, send) = budgeted.begin(client.post(ENDPOINT)).unwrap();
        drop(send);
        let wallet = handle.responses_wallet().unwrap();
        assert_eq!(
            wallet.attempts[0].unknown,
            Some(UnknownCause::Transport {
                reason: UnknownReason::Cancelled
            })
        );
        // A retry of the same request is refused before anything is reserved.
        assert!(budgeted.begin(client.post(ENDPOINT)).is_err());
        assert_eq!(handle.responses_wallet().unwrap().attempts.len(), 1);
        assert!(handle.success_gate().is_err());
    }

    #[test]
    fn a_v1_handle_refuses_a_responses_request() {
        let wallet = super::super::test_support::TestWallet::open(|_| {});
        let value = body_value(text_input(), 50);
        let budgeted =
            BudgetedBody::for_responses(&wallet.handle, &value, SendPurpose::Executor, ENDPOINT)
                .unwrap();
        assert_eq!(
            budgeted
                .begin(reqwest::Client::new().post(ENDPOINT))
                .map(|_| ())
                .unwrap_err(),
            SendRefusal::Wallet(budget(BudgetRefusal::RouteNotAdmitted))
        );
        assert_eq!(wallet.sends(), 0);
    }
}
