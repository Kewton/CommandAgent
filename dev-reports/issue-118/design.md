# Issue 118 Design

## Goal

Refactor the GUI Trial route without changing its observable behavior. The route
entrypoint should only wire the shared shell to the Trial screen, while Trial
state/effects, rendering, API access, and formatting live in focused modules.

## Design

- Keep `gui/app/try/page.tsx` as route wiring only: render `Shell` and the Trial
  screen component with the existing title and description.
- Move the Trial workflow state machine, polling lifecycle, token persistence,
  effects, and action callbacks into `gui/hooks/use-trial-run.ts`.
- Move the existing Trial markup, strings, API-facing test IDs, and stage
  presentation helpers into `gui/components/trial-run.tsx`. The component will
  consume the hook instead of owning workflow effects.
- Add `gui/lib/trial-api.ts` as the single location for Trial authorization
  headers and typed Trial fetch helpers. Reuse it from the Trial screen hook and
  session-index component.
- Add `gui/lib/format.ts` for shared byte and date/time formatting. Replace the
  duplicate byte/date helpers in the dashboard, run detail, Trial screen, and
  session index while preserving each caller's existing zero-value text.
- Make every `MonitorFailure` retain an HTTP-like `status` and error `code`
  (`0`/`null` when no HTTP response exists). Use the shared
  `isTrialTokenRejected` predicate for both ordinary request errors and monitor
  failures so a rejected token follows the existing clearing path.
- Update the Rust source-contract guard to inspect the new owning modules and to
  enforce that the route is wiring-only, helpers are centralized, monitor
  metadata is retained, and the shared rejection predicate is used.

## Compatibility

- Preserve all rendered Japanese copy, `data-testid` values, route/query shapes,
  request methods, authorization semantics, polling intervals, and session state
  transitions.
- Do not change Rust API/event schemas or the `.anvil/` runtime namespace.
- This is a structural refactor, so no corpus event fixture changes are needed.

## Verification

Run the focused GUI guard first, then the required GUI typecheck/lint/build,
`cargo test --features gui`, and both required smoke modes. Compare the smoke
JSON contracts with pre-refactor baselines after removing run-specific timing
fields if necessary.
