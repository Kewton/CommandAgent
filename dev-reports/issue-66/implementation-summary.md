# Issue 66 Implementation Summary

## Outcome

The Trial launch identity is now frozen from delegation through the terminal
result. Goal, Trial token, Profile, Provider, Executor model, Planner model,
and the contract-check action share one native disabled condition, preventing
their Gate 1 invalidation handlers from moving an active run back to DRAFT.

SESSION CLOSED now presents **Start a new run**. The action clears the previous
proposal, confirmation, created and polled session, directive, and error before
returning to an editable compose stage. It intentionally retains the in-memory
Trial token and launch spec for the next run.

## Changed Files

- `gui/app/try/page.tsx`: added the shared launch-identity lock and the explicit
  CLOSED-to-compose reset.
- `gui/app/globals.css`: added disabled-field feedback and bounded the new CLOSED
  action width.
- `gui/scripts/smoke.mjs`: verifies read-only controls and stable token focus at
  Gate 2, the terminal/CLOSED locks, cleared state on DRAFT recovery, and a
  second launched session through terminal for both supported base paths.
- `tests/gui_read_only_guard.rs`: pins the client lock and new-run reset contract.
- `docs/user/gui.md`: documents the read-only period, new-run workflow, and
  expanded smoke evidence.

No server source, Trial API, Gate 1 hash verification, Origin handling, Bearer
handling, event schema, or runtime namespace was changed.
