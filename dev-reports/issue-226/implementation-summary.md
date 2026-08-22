# Implementation Summary: Issue #226

## Outcome

- Added a process-scoped, nonblocking workspace lock at
  `.commandagent/lock`. The lock records schema version, PID, and run ID,
  reports bounded owner metadata on contention, and is released by the OS if a
  process exits unexpectedly.
- Wired the lock around execution-capable top-level actions while keeping
  read-only run inspection, model probing, diagnostics, and the UX demo
  available. Workflow children reuse the outer lock rather than acquiring a
  nested lock.
- Added typed formal-run duration samples to the existing band catalog. Gate 1
  and deterministic direct commands consume those typed values; bands without
  duration evidence are explicitly shown as unmeasured.
- Expanded the live footer token display to show cumulative prompt,
  generation, and total counts independently, retaining `n/a` for unknown
  telemetry.
- Clarified saved-plan handoff guidance: edit when needed, validate next, and
  run only after successful validation.

## Scope controls

- Fast-forwarded to predecessor commit `e5b0bbca` before implementation so the
  frozen state/config/run contract is present.
- Did not edit `src/runs.rs`, parse GUI evidence at runtime, change event names
  or schemas, modify `.anvil` runtime behavior, or rewrite historical evidence.
- No corpus fixture changed because this implementation does not change an
  event, recovery, or corpus contract.

## Tests

- Added focused tests for lock metadata, bounded contention, and reacquisition.
- Added catalog integrity and measured/unmeasured duration tests.
- Added Gate 1 and direct-command estimate coverage.
- Added prompt/generation/total footer coverage and both plan-kind guidance
  coverage.
- Added action classification coverage for execution versus read-only locking.
