# Issue 48 implementation summary

## Outcome

Planner provider responses no longer stream their raw machine-formatted payloads
into terminal scrollback when streaming is enabled. Provider transport remains
streaming, so final response assembly, timing, cancellation, and telemetry keep
their existing behavior.

## Changes

- Added provider-scope stream-rendering policy in `src/provider_call.rs`:
  `PlannerStep` and `PlannerUltra` chunks are drained without invoking the
  terminal callback, while `Executor` and `Repair` keep their existing callback
  path.
- Added focused provider-call coverage proving both planner scopes still use
  streaming transport without forwarding chunks. The existing executor test
  continues to prove incremental callback delivery.
- Updated `tests/tui_pty.rs` to cover `/plan-steps` and `/ultra-plan-run` with
  streaming enabled. The test asserts raw planner JSON is absent while planner
  breadcrumbs, spinner clearing, footer restoration, and the REPL prompt remain
  intact.
- Added a PTY Esc regression that interrupts an in-flight streamed planner turn
  and verifies the interruption block plus spinner/footer cleanup.
- Clarified the English and Japanese `--stream` CLI documentation: visible
  executor/repair output streams, while planner machine output stays hidden.

## Compatibility

- No event emission site, name, key, or value changed.
- No planner runner chokepoint changes were needed.
- No `.anvil/` runtime state or historical evidence was modified.
- No corpus contract changed, so no corpus fixture update was required.
