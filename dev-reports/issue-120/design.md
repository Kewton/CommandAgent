# Issue 120 design

## Scope

Add a first-visit “はじめに” panel to the dashboard, backed by the existing
runtime-status request. The panel is the first dashboard content, can be closed
for the lifetime of the current browser tab, summarizes actionable Trial
prerequisites, links to concise term help, and offers a sample-goal action.

The sample action navigates to the Trial route with a stable query marker. The
Trial workflow owns interpretation of that marker and fills only a safe draft
goal/profile preset; exact planner and executor model IDs remain empty and Gate
1 confirmation remains mandatory. A short Gate 1 primer on the compose screen
explains those remaining steps.

## Runtime contract

Extend `GET api/runtime-status` additively with a `prerequisites` object. Each
entry has a stable status (`ready`, `unconfigured`, or `action_required`) and a
human-readable detail. Report the execution root, delegated commandagent
binary, and Trial authentication mode. Preserve all existing fields and event
schemas.

The execution-root check reuses the workspace policy's current validation. The
binary check is a bounded filesystem/executable check and does not spawn a
process on each status poll. Authentication reflects whether a per-tab token is
required, without exposing token values.

## UI and persistence

- Render the guide before dashboard metrics.
- Use `sessionStorage`, namespaced by GUI base path, so dismissal survives
  same-tab navigation but does not leak to another tab or deployment prefix.
- Treat unavailable storage as non-fatal and keep the guide usable.
- Keep Japanese labels and an inline terminology disclosure for Gate 1,
  execution root, and pack.
- Use base-path helpers for the sample link; the smoke probe covers `/` and
  `/proxy/commandagent/`.

## Dependency integration

Issue 112 already contains the committed Issue 106, 110, 111, 118 dependency
line. Integrate that branch, then integrate the Issue 121 tip on top so this
change targets the actual Trial refactor, pack selector, delegate contract,
and GUI setup/preflight behavior. Resolve only overlapping dependency wiring;
do not duplicate predecessor implementations.

## Tests and verification

- Extend the Rust GUI contract/guard tests for the additive runtime fields,
  sample preset test IDs, and preserved Gate 1 confirmation controls.
- Extend browser smoke to cover landing guide → sample action → Trial preset and
  Gate 1 primer for both configured base paths, plus same-tab dismissal.
- Run focused GUI Rust tests, GUI typecheck/lint/build, and the relevant smoke.
- Because shared Rust GUI-server contracts and predecessor integration are
  touched, also run formatting, clippy for all targets, and the full Rust test
  suite.
