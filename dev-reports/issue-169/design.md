# Issue #169 Design

## Problem

The Trial form and Gate 1 card show the requested goal, profile, model pins,
and pack, but the form is intentionally removed once execution starts. Gate 2
and the terminal Gate 3/4 result therefore identify the session only by its ID
and status. An operator cannot confirm what that session is executing without
leaving the current screen or opening lower-level evidence.

## Source of truth

Use the persisted Gate 1 confirmation identity as the display source. The GUI
server already loads and validates that record for every session status request
before projecting events. It is immutable for the confirmed run and remains
available after a browser reload or an explicit reconnect, unlike the browser
form state.

## Change

- Add the validated confirmation `identity` to the existing session status JSON
  as a backward-compatible response field.
- Add the matching field to the GUI `PolledSession` type.
- Render a shared, read-only run identity summary in Gate 2 and the terminal
  result. Show the goal, profile, executor and planner provider/model pins, and
  the exact `id@version` pack selector (or `選択なし`).
- During the short interval between launch acceptance and the first status
  poll, render the same frozen identity from the accepted Gate 1 proposal.
- Update the GUI Trial guide to document the persistent identity summary.

## Verification

- Extend the GUI server integration assertion to cover the new response field
  and its exact frozen identity values.
- Extend the browser smoke flow to assert the identity summary at Gate 2, at
  the terminal result, and after a terminal reconnect.
- Run GUI type/lint/build checks and the focused GUI server test, then the
  repository formatting, Clippy, and full Rust test checks because the shared
  GUI session response contract changes.

No event schema, confirmation record, runtime namespace, or acceptance gate is
changed.
