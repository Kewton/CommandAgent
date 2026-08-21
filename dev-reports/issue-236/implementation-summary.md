# Issue #236 Implementation Summary

## Changes

- Added a verifier failure-classification leaf module that recognizes exit 127,
  exit 126, missing-interpreter diagnostics, and execution-permission
  diagnostics as deterministic environment failures.
- Routed those failures into the existing verifier-command false-negative
  category. Its existing repair reachability is false, so plan-run emits the
  normal unreachable handoff and does not invoke the repair model.
- Kept `src/planner/verify.rs` to minimal wiring and stayed within its existing
  production growth budget without changing the guardrail baseline.
- Preserved ordinary nonzero test failures as artifact command failures and
  preserved all existing timeout handling, including Issue #204's
  `python3 -m compileall -q src` fallback.

## Tests

- Added an integration test that executes a plan with an exit 127 verifier and
  asserts exactly one provider call for initial implementation, meaning zero
  repair-model calls.
- Added focused coverage for permission-denied and unavailable-interpreter
  commands, plus a regression that keeps ordinary exit 1 test failures in the
  artifact-repair path.
- Added unit coverage for the exact environment signatures and for keeping a
  syntax error outside the environment classifier.

## Compatibility

- No event names or schemas changed.
- No runner, historical evidence, live `.anvil/` state, Issue #208 files, or
  other lane-owned production files changed.
