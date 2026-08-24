# Issue #258 design

## Goal

Split the GUI Trial hook and component chokepoints by responsibility without
changing behavior, rendered copy, `data-testid` values, API calls, or the
public value returned by `useTrialRun`.

## Current surface

- `gui/hooks/use-trial-run.ts` owns compose input and options, Gate 1,
  polling/reconnection, terminal evidence, directives, and cross-stage reset
  behavior in 679 lines.
- `gui/components/trial-run.tsx` renders every Trial stage and the shared
  session-file workbench in 703 lines.
- `docs/user/gui-help-map.md` attributes the Gate 1 primer to the monolithic
  component.

## Proposed split

### Hooks

- `use-trial-compose.ts`: Trial token persistence/rejection, request fields,
  options and pack preselection, proposal creation, lease inspection, Gate 1
  confirmation, and launch request.
- `use-trial-monitor.ts`: created/polled session state, conditional polling,
  bounded backoff, reconnect, elapsed time, current phase, and session-index
  observation.
- `use-trial-terminal.ts`: evidence/artifact loading, document selection,
  terminal title, follow-up directive preparation/confirmation, and terminal
  reset state.
- `use-trial-run.ts`: stage ownership and composition only. It wires launch,
  follow-up, new-run resets, mobile focus scrolling, and the existing flat
  return shape.

### Components

- `trial-compose.tsx`: request fields, token, workspace lease, reconnect form,
  and compose errors.
- `trial-gate-one.tsx`: confirmation card, estimates, workspace boundary, and
  explicit launch confirmation.
- `trial-gate-two.tsx`: monitor status, elapsed/mean duration, phases, and
  Gate 2 evidence actions.
- `trial-terminal.tsx`: shared read-only session files, terminal result,
  follow-up directive, close, and new-run UI.
- `trial-run.tsx`: stage rail, cross-stage error, stage component wiring, and
  session history.

Components receive the composed `TrialRunState` object, avoiding duplicate
view-model types and preserving the existing hook contract. JSX moves
mechanically; no wrappers, labels, attributes, callback ordering, or strings
are intentionally changed.

## Compatibility and risks

- Preserve all existing API functions, polling constants, retry-stop rules,
  token rejection behavior, query-string updates, and session-index revision
  increments.
- Preserve the exact reset sets for launch, follow-up confirmation, and new
  run; moving state must not accidentally clear additional fields.
- Keep every resulting hook/component file at or below 300 lines.
- Update only the Gate primer source entry in `gui-help-map.md` to point to
  `trial-compose.tsx`.

## Verification

Run the narrowest static checks first, then all required GUI suites:

1. `npm run typecheck`
2. `npm run lint`
3. a line-count check for all resulting Trial hook/component files
4. `npm run smoke`
5. `npm run smoke:errors`
6. `npm run smoke:session-index`
7. `npm run smoke:storage`

No Rust or Python production code is in scope, so Rust and Python checks are
required only if implementation unexpectedly touches those surfaces.
