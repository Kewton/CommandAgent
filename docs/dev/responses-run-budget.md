# Run budget for direct Responses (`usage_tariff`, Issue #1006)

A run budget (wallet, Issue #976) can count direct, non-streaming OpenAI Responses sends. The
provider reports token usage but no billed amount, so this basis charges a fixed, operator-declared
tariff on validated usage. It is a separate, versioned basis beside v1 (which settles on the provider's
billed amount for generic Chat Completions). Neither is ever read as the other.

## Policy: `run-budget.policy/v2`

The operator's declaration, read from `<operator root>/run-budgets/<id>.json` like v1. A file whose
`protocol` is v2 goes to the v2 decoder; every other file goes to the v1 decoder as before. The
decode refuses unknown fields, repeated keys, wrong types, and zero bounds.

| field | meaning |
| --- | --- |
| `protocol` | `run-budget.policy/v2` |
| `accounting_basis` | `usage_tariff`, required |
| `budget_id` | the wallet's ID, as selected by `COMMANDAGENT_RUN_BUDGET` |
| `limits` | v1's eight limits: `cost_usd`, `physical_sends`, `logical_global`, `logical_per_stage`, `tokens`, `wall_seconds`, `final_repair_attempts`, `final_repair_seconds` |
| `route.api` | `responses` |
| `route.endpoint` | the direct endpoint, an absolute `https` URL, compared with where the transport posts |
| `route.model` | the request model |
| `route.response_models` | the response model names accepted, by exact match (non-empty, unique) |
| `route.effort` | the reasoning effort every request must carry |
| `route.service_tier` | `default` only |
| `tariff.revision` | the price version |
| `tariff.price_conditions` | `region` and `regional_surcharge`: declarations the host cannot verify |
| `tariff.cache_write_mode` | `ttl_30m_12h` (writes priced per TTL) or `none` (no write priced) |
| `tariff.tiers[]` | `above_input_tokens` (first 0, strictly rising) and decimal-string prices per million: `input`, `cached_input`, `cache_write_30m`/`cache_write_12h` (required in ttl mode, forbidden in none mode), `output`, and a `request_fee` |
| `input_bound.method` | `body_bytes_plus_overhead_or_provider_max` |
| `input_bound.input_overhead` | added to a text-only body's byte count |
| `input_bound.provider_max_input_tokens` | optional; the bound of a request that replays an opaque reasoning item |
| `max_output_tokens`, `max_input_tokens` | per-send ceilings |
| `ledger_dir` | where the wallet lives, as v1 |

No price depends on a model name.

## Charge

`I` = input tokens (cached and written included), `C` cached, `W30`/`W12` written per TTL, `O`
output (reasoning included).

- The tier is the one `I` falls in, and its prices apply to the whole request.
- Ordinary input is `I − C − W30 − W12`.
- The terms are summed, rounded up once to Money's unit (10⁻¹⁸ USD), and the tier's fee is added.
- Reasoning has no term of its own.
- In none mode a reported write must be zero.
- A sent body that reads no input, or comes to zero, is unknown.

Usage is read from the raw body by one reader that the telemetry shares
(`providers::usage_observation`):
- it detects duplicate keys at four levels and accepts allowlisted keys only;
- an unknown key passes only as the integer 0;
- counts must be digits within u64;
- it checks that the TTL split sums to the total, that C+W ≤ I, that reasoning ≤ O, and that total = I+O.

In ttl mode the total and both TTLs are required, zeros included.

## Reservation and input bound

- **Text-only request** (text messages, calls, call outputs, tool schemas): the bound is the final
  body's bytes plus `input_overhead`.
- **Request with a reasoning item:** the bound is `provider_max_input_tokens`. Without that declaration the request is not sent (`InputBoundUndeclared`).
- **Ceiling:** a bound past `max_input_tokens`, or one that overflows, is not sent.
- **Reservation:**
  `R = ceil((B × rI + M × rO) / 1,000,000) + fee`, where:
  - `rI` is the highest input price of any class in every tier the bound can reach;
  - `rO` is the highest output price there;
  - `fee` is the highest fee there.
  A send goes only if `spent + held + R ≤ cap` (exactly at the cap passes), judged together with the
  token, physical, logical, wall and final-repair limits.
- **Reasoning replay:** every request that replays reasoning reserves the whole declared maximum,
  however short its actual input. A small budget can therefore let the first, text-only send out and
  refuse the replay after it. A smaller bound for opaque items is another Issue.

## Ledger: `run-budget.ledger/v2`

The ledger lives at the same `<ledger_dir>/<budget_id>`, under the same lock and store as v1 (PM decision
U1).
- **Mixing:** a v1 record is refused by v2 and a v2 record by v1 (`AccountingBasisChanged`). There is no
  migration.
- **Fixed policy:** a record holds the basis and the hash of the policy's full bytes. Any change to
  the declaration, including its mode, is `PolicyChanged`.
- **Per send:** the request digest, the input-bound method and bound, the reservation and the state.
- **Two cost fields, never mixed:**
  - **budget charged** (`charged`: usage, tier, amount, late) is what the limits count, and the only
    source of a response identity;
  - **provider billed** is always `unreported`: never a zero, never `KnownBilling`, never written as
    `provider_billed_cost_usd`.
- **Unknown:** timeout, cancel after the send, HTTP error, cut body, redirect, invalid usage, a
  response of another model or tier, and an uncountable charge each leave the send unknown. Its
  reservation is held and the latch is set for good. The cause is recorded in fixed words, with the
  validated counts kept as observed when there are any.
- **Process end:** a reservation that never left goes back on reopen. A send that was started becomes
  `interrupted_after_start` and stays held.
- **Late reply:** a late valid reply settles the same send once, and the latch stays.

## Client, factory, events

- **Factory:** under a v2 wallet it builds only the direct Responses client of an admitted role
  (`admission::admit_responses_client`); every other client is refused before it exists.
- **Direct client:** under a v2 wallet it:
  - follows no redirect;
  - adds `service_tier: "default"`;
  - lowers the output to the policy's;
  - reserves and starts each try on disk before its POST, with the exact bytes and the URL it posts
    to;
  - charges the raw body before the reply is parsed.

  With no wallet its requests and retries are unchanged.
- **Events:** `provider_turn_duration`, and the same response's `provider_response`, carry
  `provider_cache_write_input_tokens`, `_30m` and `_12h` only when a valid value was reported (corpus
  `tests/corpus/apps/responses-run-budget`). A reply that fails to parse still hands its observation
  to the turn, on a path apart from the response metadata.

## Wallet-off control

`tests/responses_run_budget.rs` compares the binary's wallet-off runs with a recording of the base
ff920681 (`tests/corpus/apps/responses-run-budget/fixtures/off-golden.json`). It compares requests,
the connection count, files, the outcome, and each event line byte for byte.

Only these are normalized, each shown to vary between two raw runs of the base or to name the build:
- the temporary root and loopback address;
- the wall-clock integers `duration_ms`, `occurred_at_epoch_ms`, `executor_ms`, `provider_ms`,
  `total_ms`, `time_profile_provider_ms` and `time_profile_total_ms`;
- the build strings `build_commit` and `build_timestamp`, and the build flag `build_dirty`;
- the lock file's `pid` and `run_id`.

A value of another type, or a line that is not JSON, is a difference. The three new cache-write fields
are removed by name before an event line is compared.

## Decisions

- **U1:** v1 and v2 share the wallet's path and lock; the decode branches on schema and basis; a change of
  version or mode is refused; there is no migration.
- **U2:** the "independent check" of the Issue is the existing host judgement and every existing send
  path (planner, execution, final repair, existing child paths) under the one wallet. No new LLM call.
- **C1:** v2 has its own record and mode branch, and the identity is issued from the durable charge. No fake billed amount, no `KnownBilling`.
- **C2:** the factory passes the shared handle and role. Reserve and start happen before each POST.
  The endpoint is the transport's, and "direct" is judged from the role's resolved provider and the
  absence of a compatible endpoint.
- **C3:** the request check, output clamp and digest of the final bytes are wallet-only. Replayed items are checked against the documented shapes, and the shared builder is unchanged.
- **C4:** usage is read before call IDs, G2 and tools. The observation of an error turn travels apart
  from the metadata, and `None` metadata stays `None`.
- **C5:** settled by U2.

## Review 2 notes

- **N1 (PM: as v1):** a 2xx reply whose usage is unknown still reaches the caller. Its tools may run,
  but the next send and the run's success stop, and no further cost is incurred.
- **N2:** a cancel between the durable start and the POST keeps the reservation (conservative).
- **N3:** the direct client never delivers a late reply; the late-usage path is ledger-level and tested there.
- **N4:** every role must use the policy's one model and effort. A planner on another model sends nothing.

## Item allowlist (wallet-on admission only)

- **Allowed, with the documented values only:**
  - `message.phase`: `commentary`, `final_answer` or `null`;
  - `function_call.caller`: exactly `{"type":"direct"}`;
  - `function_call.async`: exactly `false`;
  - `function_call.namespace`: a string;
  - `reasoning.encrypted_content`: a string or `null`.
- **Still refused:**
  - refusal parts;
  - non-empty `annotations` or `logprobs`;
  - `prompt_cache_breakpoint`;
  - image and file input;
  - the optional keys of `function_call_output` (`id`, `caller`, `name`, `namespace`, `status`);
  - anything not listed.
- **Tightened to the reference:**
  - `status` may only be `in_progress`, `completed` or `incomplete`;
  - a replayed message needs its `id`, a `status` and the `assistant` role;
  - a reasoning item needs its `id`, and a `summary` of `summary_text` parts;
  - reasoning `content` must be `reasoning_text` parts.

## Limits

- **Item shapes are not observed:** real API item keys were checked against the API reference
  with synthetic fixtures, not against observed responses. They are to be confirmed in the pack's
  wallet-on run after merge.
- **Operator declarations are trusted:** the tariff and `provider_max_input_tokens` are the
  operator's declarations, and the host does not verify them. An observed input past the reserved
  bound is charged and stops the wallet.
- **Regional surcharge:** the host cannot verify it from a response, and a matching `service_tier`
  or model does not confirm the region.
- **Billing vs budget:** the budget being used up is not the provider's bill being confirmed. A
  comparison of real cost stays open.
