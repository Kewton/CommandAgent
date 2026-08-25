# Issue 393 design: goal-to-verify Phase 0 contract

## Purpose

Freeze a reviewable, reproducible Phase 0 measurement contract before a
`VerificationSpec` implementation exists. This change measures the current
path by fixture replay; it does not generate a production artifact, emit an
event, alter terminal projection, or change assurance authority.

## Design

- Add a versioned `eval/goal_verify/v0/` corpus. Every case records intent,
  profile, language, size, polarity, deterministic required/optional
  acceptance claims, minimum oracle strength, allowed/forbidden verdicts, and
  a replay observation of the current path. Claim matching is by stable claim
  ID plus executed deterministic observation, never by an LLM judge.
- Keep the term `acceptance_claim` distinct from the data profile's existing
  `claims-binding` evidence.
- Add a leaf Python module and thin CLI that validate the corpus, aggregate
  required-claim precision/recall/F1, binding coverage, verdict errors, task
  success, latency/tokens/retries/repairs/flake/block rates, schema yield, and
  deterministic bootstrap 95% confidence intervals. The seed and bootstrap
  count live in a checked-in config and are copied into each result.
- Make output append-only by requiring a new/empty run directory. A checked-in
  fixture-replay baseline under `dev-reports/issue-393/runs/` records the
  current measurements; raw live-provider runs remain out of scope and would
  belong in the Issue-specific localwork directory.
- Add an ADR that freezes intent semantics, v0 schema/version compatibility,
  authority isolation, unknown/composite handling, shadow opt-in/isolation,
  rollout/rollback, sampling, non-inferiority budgets, improvement targets,
  and the one-time Phase 1 resource-budget registration rule.
- Add Python tests for schema rejection, deterministic reruns, metric
  calculation, confidence intervals, and corpus coverage. Add a lightweight
  Rust corpus fixture so the repository corpus harness guards the committed
  Phase 0 manifest without modifying production code.

## Baseline interpretation

The Phase 0 numbers are an offline fixture-replay baseline, not a claim about
new live-provider trials. Latency and token values are replayed measurements
and are reported with their provenance. Cells without enough independent
samples are explicitly `insufficient_evidence`; no confidence interval is
allowed to manufacture a Go decision.

## Compatibility and safety

- Schema identifier: `commandagent.goal_verify.corpus.v0`; additive fields are
  permitted within v0, while semantic or required-field changes require a new
  version and a dual-read migration window.
- The runner reads only corpus/config inputs and writes only a caller-selected
  new directory.
- Existing `workspace/management/runs/`, `docs/migration/`, `.anvil/`, event
  schemas, acceptance logic, and production Rust are untouched.

## Verification plan

Run the focused Python test, the new CLI twice and compare byte-identical
summaries, Ruff on changed Python files, the Rust corpus regression test, then
the required formatting, Clippy, and full Rust suite checks.
