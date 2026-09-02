# ADR: goal-to-verify VerificationSpec v0 evaluation contract

- Status: accepted for Phase 0
- Date: 2026-08-25
- Scope: evaluation and shadow artifacts only

## Decision

`VerificationSpec v0` is an evaluation-only, non-authoritative shadow concept.
The current production chain remains the authority:

`goal -> intent/profile -> StepPlan.verify -> CompletionContract -> profile/runtime probe -> adjudication/terminal projection`

The v0 corpus calls its units `acceptance_claim` to avoid collision with the
data profile's existing `claims-binding` artifact. An acceptance claim is a
stable ID bound to a deterministic observation. Natural-language similarity
and an LLM judge are not valid bindings.

## Intent semantics

For all intents, a required claim is strong only when its registered oracle ran
and met or exceeded `min_strength`. `weak` evidence can explain a partial
result but cannot satisfy a deterministic or runtime minimum. Missing or
under-strength required claims remain visible; they cannot be silently dropped.

- `create`: claims describe goal-observable behavior or state. Build success
  proves only buildability. UI copy/style uses DOM/computed-style observation;
  CLI values and multiple inputs use a command matrix; port/path and functional
  behavior require runtime observations; negative conditions require evidence
  that the forbidden effect did not occur.
- `fix`: the baseline and after observation must use the same hash-bound
  reproducer. A non-reproduced baseline, substituted reproducer, unexecuted
  after step, shrunken regression set, or old-tests-only result forbids `full`.
- `investigate`: claims separate observation from causal inference. Paths,
  lines, snippets, and errors must exist in the captured source/runtime
  snapshot. A defective reproducer may itself be the supported conclusion.
  Correlation without an intervention is not a causal binding.

`unknown` and composite intents are outside the v0 three-intent enum. They are
not coerced to `create`; they project to `unverified`, or to `partial` only when
individual reported observations are bound. Adding an intent is a new schema
version, not a v0 label edit.

## Metrics

The frozen runner reports required-claim precision, recall, F1, strong-binding
coverage, weak-only coverage, unverified rate, false-full/false-fail/
false-partial, task success, final acceptance, planner calls, token totals and
p50/p95, wall and verification time p50/p95, retry/repair counts, flake rate,
policy rejection, dependency blocked, and structured-schema yield. Results are
partitioned by intent, profile, language, and size.

Confidence intervals use deterministic percentile bootstrap with the seed and
sample count in `baseline-config.json`. The full intent/profile/language/size
cells currently contain one replay case and are therefore explicitly
`insufficient_evidence`. A later comparison must use paired bootstrap deltas;
the 95% lower bound must be no worse than each numeric non-inferiority budget.
Missing evidence never counts as improvement.

## Sampling and cost boundary

The checked-in 12-case corpus is the review corpus, not a claim of adequately
powered live inference. Comparative evaluation targets at least 30 independent
replays in every eligible intent/profile/language/size cell. At 12 current
cells this is 360 runs. Provider-backed execution at several minutes per run is
not authorized by Phase 0, so deterministic fixture replay supplies contract
testing; Phase 1 separately samples at least 30 schema-generation attempts per
target model. Any cell below its target remains `INSUFFICIENT-EVIDENCE` in
Phase 6 rather than being pooled into a favorable result.

Latency/token deltas cannot be defensibly fixed before a VerificationSpec
exists. Phase 1 must register p50/p95 latency and total-token budgets exactly
once after shadow measurement and before Phase 2. The required fields and the
immutability rule are pre-registered in the baseline config.

## Schema and compatibility

The corpus schema is `commandagent.goal_verify.corpus.v0`; the report schema is
`commandagent.goal_verify.baseline.v0`.

- Additive optional fields are compatible within v0.
- Changing intent meaning, required fields, strength ordering, claim identity,
  or verdict semantics requires v1 plus a dual-read migration window.
- Existing event names/schemas and `.anvil/` state remain unchanged.
- Existing oracle repair may normalize syntax or platform-equivalent command
  spelling, but must not change a required claim, expected observation, minimum
  strength, reproducer identity, or regression membership. Such a semantic
  change requires a new reviewed corpus version.
- `VerificationSpec` coexists with `StepPlan.verify` through shadow rollout.
  Replacement is a separate post-Phase-6 decision and schema migration.

## Authority and shadow isolation

Shadow generation is opt-in and off by default. It receives a read-only
snapshot and writes only to its caller-selected run directory. Its failures,
timeouts, malformed output, and policy/dependency blocks are measurements only:
they cannot change verification commands, tool authority, retries, completion,
event authority, final acceptance, terminal projection, or assurance verdict.
No new event is authorized by v0.

## Rollout, rollback, and go/no-go

Rollout order is offline replay, opt-in shadow generation, comparative shadow,
then a separately approved authority experiment. Advance from Phase 0 only
when corpus/schema review, annotation review, reproducible baseline, thresholds,
and compatibility policy are present. Unstable labels, unresolved intent
semantics, non-reproducible output, or mutable thresholds are No-Go.

Rollback disables shadow generation and stops reading the shadow artifact. No
production or `.anvil/` migration is needed because shadow state has no
authority. A false-full increase above zero, schema yield below 95%, breach of
a non-inferiority bound, or authority leakage triggers rollback. Thresholds
cannot be weakened after viewing a candidate result; changing them starts a new
version and review.
