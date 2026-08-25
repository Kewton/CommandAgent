# Issue 399 implementation summary

## Outcome

Added a reproducible Phase 6 A/B UAT decision harness and produced the explicit
final decision `INSUFFICIENT-EVIDENCE`. This is the only honest decision for the
checked-in evidence: there is one Phase 0 baseline replay per target cell, no
candidate comparative replay, no exact-SHA CI evidence registered for Phase 6,
no approved live campaign, and the Phase 1 resource budgets remain unset.

## Changes

- Added `eval/goal_verify/v0/phase6-matrix.json`, freezing 12
  intent/profile/language/size cells, a 30-sample-per-side minimum, baseline and
  candidate identities, and distinct blind-review, CI, offline/local, and
  approved-live evidence lanes.
- Added `scripts/eval_lib/goal_verify_phase6.py` and the
  `scripts/eval-goal-verify-phase6.py` CLI. The aggregator validates every raw
  evidence reference, refuses non-empty output directories, computes paired
  bootstrap confidence intervals, applies frozen confidence-bound thresholds,
  lists every insufficient or failed case, and emits only `GO`, `NO-GO`, or
  `INSUFFICIENT-EVIDENCE`.
- Every registered safety, improvement, schema, and resource indicator reports
  baseline, candidate, delta, 95% confidence interval, threshold, and verdict.
  Missing candidate values and unregistered resource thresholds remain
  `insufficient_evidence`; measured threshold failures take `NO-GO` precedence.
- Recorded rollback rehearsal and flag-off compatibility references. These bind
  the additive opt-in shadow APIs and their existing authority-isolation tests;
  no production acceptance path, event schema, or `.anvil` state changed.
- Added focused Python tests and an Issue 399 corpus contract. The tests cover
  lane separation, result shape, deterministic output, missing evidence,
  unsubstantiated live authorization, and fail-closed `NO-GO` behavior.
- Generated `dev-reports/issue-399/runs/phase6-ab-uat-v0/phase6-report.json`
  and its raw `failure-cases.json` list from the checked-in manifest.

## Compatibility

The implementation is evaluation-only Python plus fixtures and documentation.
It does not wire `VerificationSpec` into execution or adjudication, add events,
modify runtime configuration, rewrite historical evidence, or require rollback
state migration. Disabling or omitting the Phase 6 harness leaves existing
behavior byte-compatible.
