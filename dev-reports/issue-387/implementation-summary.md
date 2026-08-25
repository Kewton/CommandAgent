# Issue 387 Implementation Summary

## Outcome

Implemented bounded Recovery Plan auto-run for every configured UltraPlan
execution entry point and the GUI Trial flow. The new CLI option is
`--recovery-plan-auto-runs <0..=20>`, defaults to zero, and excludes the initial
run from its count.

## Rust execution

- Added bounded CLI parsing for `0` through `20`; negative, out-of-range, and
  non-integer values are rejected by Clap.
- Stored the confirmed limit in shared `Config` and routed both top-level
  UltraPlan actions, REPL `/ultra-plan-run`, `/run-ultra-plan`, and UltraPlan
  `/resume` through the same configured controller.
- Added the typed `planner::auto_recovery::AutoRecoveryController` leaf. The
  zero branch calls the existing runner directly and emits no new events. A
  positive branch stops on initial success, first Recovery Plan success,
  interruption, missing/non-recoverable handoff, unsafe resume, workspace
  drift, invalid or review-required YAML, path escape, repeated-plan cycle, or
  the configured limit.
- Replaced event-file discovery and rendered-error classification with a
  per-attempt typed recovery candidate captured at the common Recovery Plan
  save seam. Interruption is taken from the typed UI state, and recovery
  decisions do not parse suggested commands, stop-reason strings, or arbitrary
  errors.
- Added typed Recovery YAML validation and reused `runs::prepare_resume` plus
  workspace drift checks before every automatic execution. Cycle detection
  compares normalized parsed `UltraPlan` content, excluding volatile recovery
  metadata, path text, comments, and formatting. Existing
  verification, acceptance, evidence, local-repair, and iteration limits were
  not changed.
- Added additive `recovery_plan_auto_run_*` events carrying current/used/limit
  and stop reason. Successful automatic recovery resets prior failure recovery
  fields in the final completion projection so stale documents and reasons do
  not leak into the success result. A later manual continuation remains newer
  than an earlier auto-success boundary and is not erased.

## GUI contract

- Added a `0..20` Gate 1 input and rendered the confirmed recovery count, the
  maximum total plan executions (`1 + N`), and the equivalent duration/cost
  upper-bound multiplier. The value is part of confirmation identity and its
  hash only when nonzero; omitted and explicit zero retain the legacy
  serialized identity and hash.
- The server validates the proposed value and delegates a nonzero
  `--recovery-plan-auto-runs` only from the persisted confirmed identity. A
  stale hash is rejected after the value changes.
- Added recovery current/used/limit/stop reason to the session projection and
  shared identity display used by Gate 2, terminal, and history detail.
- Kept recovery-document endpoints read-only; no execution control was added
  to document view/copy actions.

## Tests and documentation

- Added CLI boundary tests; typed controller tests for initial success,
  fail-then-success, exact retry caps, invalid/non-recoverable handoffs, and
  normalized cycles; all entry-point routing tests; confirmation hash tests;
  manual-after-auto-success boundary coverage; GUI API
  boundary/hash/delegation/status tests; and GUI contract guard updates.
- Added `tests/corpus/apps/issue387-auto-recovery/` for the additive event
  contract and extended browser smoke to verify the field in explicit and
  edited proposals at both supported base paths.
- Updated the CLI references, shell design, mechanism ledger, and UAT scenario
  checklist.
