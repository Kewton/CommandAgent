# Issue 66 Design

## Problem

The Trial launch-identity controls remain editable after delegation. Their
existing change handlers invalidate Gate 1 and set the screen stage to
`compose`, so an in-flight poll can move the stage back to `gate_2` and make
the header and contract card flicker. After choosing **End without another
run**, the `closed` stage also has no transition back to a new composition.

## Design

- Derive one UI lock condition for `gate_2`, `terminal`, and `closed`, and apply it to
  Goal, Trial token, Profile, Provider, Executor model, and Planner model.
  Disable the contract-check action under the same condition. Native disabled
  controls reject Playwright `fill()` / `selectOption()` and cannot receive
  focus, so their existing Gate 1 invalidation handlers cannot run. Keeping the
  CLOSED form locked also makes its new action the explicit lifecycle boundary.
- Add an explicit **Start a new run** action to the CLOSED card. Reset only
  run-scoped proposal, confirmation, created-session, polled-session,
  directive, and error state before returning to `compose`. Preserve the
  entered launch spec and in-memory Trial token so the user can adjust and
  submit the next run without reloading.
- Keep all API requests, authorization headers, Gate 1 hash confirmation, and
  server modules unchanged.

## Verification

- Extend the existing static GUI guard with the lock and CLOSED reset
  contract.
- Extend the Playwright smoke flow to assert launch inputs are disabled during
  Gate 2 and terminal, a programmatic token focus does not produce DRAFT, and
  CLOSED can return to an editable DRAFT that reaches Gate 1 and launches a
  second session.
- Run GUI lint, TypeScript typecheck, static export build, the focused Rust GUI
  guard test, and the repository formatting/clippy/test checks required before
  production-code handoff. Run the real smoke when its local Playwright/model
  prerequisites are available; otherwise report it as blocked rather than
  claiming success.
