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

## Issue #162 follow-up propagation

Propagate exact follow-up commit
`ea8f8fbdc0d0a7fc9e23cdff38fa30b874e95e6d` by cherry-pick. Its Trial token
retry fix defers automatic session-index revalidation while a reconnect ID is
waiting for an explicit token, clears a rejected token, and keeps the explicit
retry button usable. Use `git range-diff` to prove that the cherry-picked patch
is unchanged even though its commit ID is rewritten on the Issue #169 branch.

The overlap is confined to Trial UI wiring and the two browser smoke scripts.
Combine the contracts rather than choosing between them: retain #162's rejected
token and retry assertions, and retain #169's frozen goal, profile, model, and
pack identity in synthetic status responses and in Gate 2, reconnect, and
terminal browser assertions. Rebuild the GUI, run both root/proxy smoke suites,
and repeat the shared Rust checks because the propagated commit changes common
Trial components.
