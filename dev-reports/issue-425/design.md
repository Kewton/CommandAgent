# Issue #425 Design

## Problem

Create runs can generate the authoritative `completion-contract-ultra-plan-run.json`
with an empty `verify_commands` list. A failed step still records its normalized
verification commands in the typed Recovery handoff. Recovery preflight can
therefore observe a genuine Next.js failure, but the later contract-command bind
rejects the empty run contract and stops before executing the Recovery Plan.

## Design

- At the Recovery contract-authority boundary, pass the failed candidate's typed
  handoff verification commands into the run-contract binder.
- Only when the selected contract is the host-generated run contract, its command
  list is empty, and its profile is `nextjs` or `generic`, validate and persist the
  handoff commands into that contract before Recovery preflight and candidate
  rebinding. These commands already came from the executed, normalized step plan;
  completion-contract validation remains the final admission gate.
- Never augment a user-configured contract or a generated data-profile contract.
  The existing rule that Recovery may use only commands registered by those
  contracts remains unchanged.
- Preserve existing event names and stop-code fields. Add a human-readable
  `recovery_plan_auto_run_stop_summary` field and include the readable summary in
  returned errors while retaining the stable machine code.

## Tests and verification

- Add focused unit tests for generated Next.js and generic command completion,
  configured-contract immutability, and data-profile non-augmentation.
- Add an auto-Recovery test proving a failed candidate reaches its first Recovery
  execution after generated-contract completion without emitting
  `contract_command_bind_failed`.
- Add a corpus fixture covering the additive event fields and generated-contract
  command provenance.
- Run focused Rust tests, corpus regression, formatting, Clippy, and the full Rust
  test suite because shared CLI Recovery/event behavior changes.
