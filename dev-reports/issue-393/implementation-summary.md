# Issue 393 implementation summary

## Outcome

Phase 0 is frozen as an offline, reproducible goal-to-verify evaluation
contract. The committed fixture replay is Go for beginning Phase 1 contract
work; it does not authorize production rollout or claim that the model schema
yield gate has passed.

## Changes

- Added the `commandagent.goal_verify.corpus.v0` gold/adversarial corpus with
  12 reviewed cases, 16 required claims, deterministic oracle descriptions,
  create/fix/investigate positive and negative coverage, bilingual and
  cross-cutting adversarial cases, and explicit verdict partitions.
- Added a deterministic baseline runner with append-only output, corpus
  validation, seeded percentile-bootstrap 95% confidence intervals, complete
  aggregate and intent/profile/language/size partitions, and frozen numeric
  non-inferiority and improvement thresholds.
- Added the Phase 0 ADR covering intent semantics, `acceptance_claim` naming,
  unknown/composite handling, schema migration, oracle-repair limits,
  authority isolation, opt-in shadow rollout, rollback, sampling, and the
  one-time Phase 1 latency/token budget registration rule.
- Added focused Python tests and a Rust corpus-harness fixture. No production
  Rust, event schema, terminal/assurance logic, `.anvil/` state, or historical
  evidence changed.

## Frozen baseline

- Cases / required claims: 12 / 16
- Required-claim precision / recall / F1: 70.5882% / 50% / 58.5366%
- Strong / weak-only / unverified coverage: 50% / 12.5% / 37.5%
- False-full / false-fail / false-partial: 0 / 0 / 0
- Allowed-verdict task success / final acceptance: 100% / 33.3333%
- Wall time p50 / p95: 244.5s / 900s
- Verify runtime p50 / p95: 23.5s / 120s
- Input tokens p50 / p95: 11,450 / 28,900
- Output tokens p50 / p95: 3,100 / 7,100
- Flake rate: 2.7778%; retries / repairs: 3 / 3
- Default-model offline schema-yield proxy: 91.6667%

The 91.6667% schema proxy is below the frozen 95% Phase 1 shadow-generation
gate. This does not invalidate the Phase 0 contract Go decision; it means a
provider-backed shadow candidate may not advance until its separately measured
yield meets the gate. Full four-dimensional cells have one case each and are
reported as `insufficient_evidence`, requiring 30 replays per eligible cell for
a later comparative decision.

## Artifacts

- Contract: `docs/dev/goal-to-verify-v0.md`
- Corpus/config: `eval/goal_verify/v0/`
- Annotation record: `dev-reports/issue-393/annotation-review.md`
- Baseline run: `dev-reports/issue-393/runs/fixture-replay-e74b7113/`
