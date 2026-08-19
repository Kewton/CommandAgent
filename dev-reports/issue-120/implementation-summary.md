# Issue 120 implementation summary

## Implemented

- Integrated the required predecessor line through Issue 112, then applied the
  Issue 121 setup/preflight commit. The only overlaps retained both exact pack
  delegation and extension-root propagation; pinned admitted packs continue to
  take precedence over ambient/local extension content.
- Added `GettingStarted` as the first dashboard content. It presents structured
  runtime prerequisites, a base-path-safe sample action, inline term help, and
  tab-scoped dismissal keyed by GUI deployment prefix.
- Extended `GET api/runtime-status` additively with execution-root,
  commandagent-binary, and Trial-authentication prerequisite records. Statuses
  distinguish `ready`, `unconfigured`, and `action_required` without spawning a
  process on the three-second status poll or exposing credentials.
- Added a query-driven Python CLI sample preset to the refactored Trial hook.
  It fills the goal, profile, and admitted `cli-assist@1.0.0` pack while leaving
  environment-specific model IDs empty.
- Added a Gate 1 primer to the compose view. Existing proposal, checkbox, card
  hash, and launch-confirmation gates are unchanged.
- Extended browser smoke for `/` and `/proxy/commandagent/` to cover initial
  guide rendering, runtime prerequisite states, sample navigation/preset,
  primer display, and dismissal persistence. The smoke saves JSON and first
  landing screenshots under `dev-reports/issue-120/smoke/`.
- Added focused Rust response and source-guard assertions, including the
  requested `data-testid` protection in
  `trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface`.
- Added the Japanese `はじめに` user-guide section with the planned E-23
  `getting-started-gui` relocation note.

## Compatibility and safety

- Existing runtime-status fields remain unchanged; the prerequisite object is
  additive.
- No event schema, `.anvil` namespace, historical run evidence, or confirmation
  semantics changed.
- The sample cannot launch a run by itself and cannot bypass Gate 1.
