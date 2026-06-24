# Loadmap2 Phase 2 Failure Observation - 2026-06-22

Base commit: `4caef03`

## Scope

Implemented the Phase 2 failure-observation boundary from the local loadmap2
planning bundle.

This slice adds a shared terminal-state taxonomy and projects deterministic
failure evidence into a normalized `FailureObservation` record before repair
packets, evidence envelopes, or eval reports consume it. The change is
attribution-only: it does not add retry authority, active job arbitration,
target selection, setup execution, or provider/model-specific behavior.

## Runtime Changes

- Added `src/agent/step_runner/failure_observation.rs`.
- Added `scripts/failure_observation_taxonomy.tsv` as the Rust/Python taxonomy
  alignment fixture.
- `ContractEvidence::render()` now includes a compact
  `failure_observation` line for failed evidence.
- `EvidenceEnvelope::from_contract_evidence()` now carries the same observation
  projection.
- Eval field extraction and reports now include producer, guard, actionability,
  failure signature, and unknown/raw coverage defects.

## Verification

Local checks run before commit:

```text
cargo test
python3 tests/test_eval_report.py
```

Both passed locally. Release build, formatting, clippy, smoke, commit, push,
and GitHub Actions verification are tracked in the enclosing task result.

## Interpretation

This phase makes the first failure classification authoritative and reusable.
It should make later active-job, repair-action, and target-priority work easier
to evaluate because reports can distinguish planning, provider, tool protocol,
step policy, setup, profile, verifier, completion evidence, evidence binding,
repair exhaustion, explicit stop, and unknown boundaries with one vocabulary.

It does not claim that large `/ultra-plan-run` workflows now converge. If an
eval still fails, the expected improvement is clearer terminal attribution and
coverage-defect reporting, not hidden continuation.
