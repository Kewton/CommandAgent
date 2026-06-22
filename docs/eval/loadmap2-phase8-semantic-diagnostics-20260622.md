# Loadmap2 Phase 8 Semantic Diagnostics

Date: 2026-06-22

## Scope

Phase 8 implements the semantic failure report and verifier diagnostic pieces
of the legacy loadmap migration.

This change keeps the single minimal-loop execution model. It only enriches
deterministic failure evidence and the recovery-facing fields that existing
contract layers consume.

## Implemented

- Verifier diagnostic eval fields now include:
  - `diagnostic_failure_kind`
  - `source_of_truth`
  - `failure_signature`
  - `command`
  - `observed_expected`
  - `affected_cases`
  - `candidate_artifacts`
  - `unknown_diagnostic_count`
- Semantic failure reports can recover source-of-truth, observed/expected,
  affected cases, candidate targets, and preferred repair role from verifier
  diagnostic payloads.
- Recovery orchestration uses `preferred_repair_role` as deterministic active
  job evidence before falling back to broad diagnostic-code string heuristics.
- Eval report summaries include diagnostic failure kinds, semantic cluster
  source, observed/expected pairs, affected cases, candidate artifacts, and
  unknown diagnostic counts.

## Design Notes

- This is contract evidence, not a new planner.
- Unknown diagnostics stay visible through `unknown_diagnostic_count`.
- Weak verifier and command policy failures route to verifier-contract repair
  instead of generic source repair.
- Assertion-like verifier diagnostics with `preferred_repair_role=implementation`
  route to implementation repair even when the diagnostic code contains
  `test`.

## Verification Commands

Planned local checks:

```bash
cargo fmt --check
cargo test
cargo build --release
python3 tests/test_eval_report.py
bash scripts/eval_smoke.sh
bash scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase8-dry-run \
  --dry-run
```

## Remaining Limits

- The diagnostic parsers remain intentionally conservative.
- Full verifier attempt-flow parity with the legacy engine remains outside
  this phase.
- Live eval is still needed to prove model behavior after the improved
  diagnostic fields are rendered into repair packets.
