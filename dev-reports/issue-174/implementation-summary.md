# Issues 174, 175, 176, 179, 180, 196, 197, 198, and 202 implementation summary

## Implemented

- Fast-forwarded this branch to the verified Issue 176 predecessor and consumed
  its shared Japanese Trial gate, session, phase-status, and phase-stage
  formatters without modifying the shared contract.
- Reordered Gate 1 composition so access setup precedes one uninterrupted goal,
  profile, pack, provider, executor-model, and planner-model flow. Lease
  inspection and manual reconnect now live together below the request action.
- Added a compact non-idle lease notice with a native reconnect action. A
  non-idle lease disables proposal through a native disabled fieldset, and both
  proposal and launch recheck the lease before issuing their POST. The server
  409 remains intact as a race-condition backstop.
- Added an additive phase-zero plan-generation projection for running,
  completed, failed, and interrupted planning. Gate 2 now reports the exact
  early event count when numbered phases have not begun.
- Replaced visible raw Trial session, phase status, phase stage, and directive
  gate values with the predecessor formatters. The request action, Gate 1
  estimate heading, validation guidance, and stale-confirmation guidance now
  consistently name `契約と見積りを確認`.
- Added a dedicated atomic live region for Trial monitor connection changes,
  with assertive priority only after monitoring is lost. Added an explicit
  label for the additional-request textarea.
- Rebuilt the stage rail as an ordered list with current-step semantics and a
  polite current-stage announcement. Replaced reconnect anchors that cancelled
  navigation with native buttons, preserving Enter and Space activation.
- Added programmatic focus targets for compose, Gate 1, Gate 2, terminal, and
  closed stages. Focus moves on every actual transition, retains the mobile
  scroll behavior, and has an explicit visible outline.
- Added focused server tests for plan-generation projection and updated the
  existing `sessions.rs` event-projection unit assertion for the additive
  planning row. No GUI smoke harness file was edited.

## Scope control

Row-owned production changes are limited to `src/bin/gui_server/sessions.rs`,
the assigned Trial components and hooks, and `gui/app/globals.css`. No page,
wizard, shared formatter/error module, corpus fixture, integration harness, or
historical evidence file was edited. This row's only additional files are the
three required reports under `dev-reports/issue-174/`.

## Verification result

Focused server tests, the repository GUI guard, GUI lint/typecheck/build, Rust
formatting, GUI-enabled Clippy, and the complete GUI-enabled Rust test suite all
pass. The full suite reports 2,124 passing library tests, 20 passing GUI-server
unit tests, and all integration/doc-test targets green.
