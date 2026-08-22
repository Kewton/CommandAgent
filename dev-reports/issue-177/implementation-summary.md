# Issues #177, #223, and #216 Implementation Summary

## Outcome

- GUI Gate 1 cards no longer show the CLI-only `/confirm <hash>` instruction.
  They explain `profile` as the selected profile's default preset, while the CLI
  renderer retains its previous command guidance and preset line.
- Both surfaces still render the same `ConfirmationIdentity` and canonical
  `card_hash`; no identity or hashing code changed.
- Every Gate 4 typed next action now includes a concrete operation. Existing
  supported flows are named explicitly: re-enter the request, `/resume`,
  restart with an elevated model, `/pack`, `/directive`, or `/exit`.
- The pre-Gate route classifier now renders as “classifying request before
  Gate 1” in breadcrumbs and the footer, then reports classification complete.
  It no longer looks like phase planning has started.

## Implementation

- Added a private Gate 1 surface distinction in
  `src/tui/boundary_shell/presentation.rs`, preserving `render_gate_one` as the
  CLI API and adding `render_gate_one_for_gui` for the GUI leaf handler.
- Centralized typed-action operation copy beside the existing action guidance,
  without changing action availability or Gate 3/Gate 4 decisions.
- Added a classifier-only display scope to the provider-call override used by
  `ambiguity.rs`. The underlying provider scope remains `planner_step`, so
  timeout behavior, `caller_scope` telemetry, event names, and event schemas
  remain unchanged.
- Added presentation, GUI integration, status/breadcrumb, and opt-in PTY
  regressions. The PTY regression drives an ambiguous plain-text request through
  a delayed fake classifier and proves the classifier label precedes Gate 1
  without pre-Gate `planning` text.

## Compatibility

- `ConfirmationIdentity`, canonical hash serialization, persisted confirmation
  records, `.anvil/`, and acceptance/evidence gates are unchanged.
- CLI Gate 1 `/confirm` output remains covered by the focused surface test.
- No corpus fixture was changed because the provider telemetry scope and event
  contract are byte-compatible; the full corpus regression passed.
