# Issue #258 implementation summary

## Outcome

The GUI Trial hook and component chokepoints are split by stage and
responsibility while preserving the existing `TrialRun` entry point and the
flat value returned by `useTrialRun`.

## Hook split

- `use-trial-compose.ts` owns token persistence/rejection, request fields,
  options and pack preselection, workspace lease inspection, proposals, Gate 1
  confirmation, and launch requests.
- `use-trial-monitor.ts` owns created/polled sessions, conditional polling,
  bounded backoff, reconnect, elapsed time, current phase, and session-index
  observation.
- `use-trial-terminal.ts` owns artifacts, read-only evidence documents,
  terminal title behavior, and follow-up directive preparation/confirmation.
- `use-trial-run.ts` is the cross-stage coordinator for launch/reset wiring,
  measured duration/cost labels, mobile stage scrolling, and the unchanged
  public return shape.

## Component split

- `trial-compose.tsx` renders the request/token/lease/reconnect form.
- `trial-gate-one.tsx` renders the proposal and explicit launch confirmation.
- `trial-gate-two.tsx` renders monitoring, elapsed/mean feedback, phases, and
  Gate 2 evidence actions.
- `trial-terminal.tsx` renders the shared evidence workbench, terminal result,
  follow-up directive, close, and new-run states.
- `trial-run.tsx` now wires the stage rail, cross-stage error, extracted stage
  components, and session history.

All nine resulting Trial hook/component files are at or below 300 lines. The
literal `data-testid` multiset is unchanged, and the full GUI smoke suite
verified the existing Japanese copy and lifecycle behavior on both supported
base paths.

## Documentation and focused test contract

`docs/user/gui-help-map.md` now attributes the Gate 1 primer to
`gui/components/trial-compose.tsx`. The corresponding help-map expectation in
`gui/scripts/smoke.mjs` was updated to follow the moved source while retaining
the same copy and document owner.

The original refactor changed no Rust, Python, API schema, event schema,
corpus fixture, or live `.anvil/` runtime code.

## PR #293 CI follow-up

PR #293 CI run `32512103334` found that two `tests/doc_drift.rs`
expectations still attributed moved Trial content to the pre-split files. The
Gate 1 primer owner now points to `gui/components/trial-compose.tsx`, and the
canonical sample goal owner now points to `gui/hooks/use-trial-compose.ts`.

The required full Rust test run then found the same stale source-ownership
assumptions in `tests/gui_read_only_guard.rs`. Its Trial source aggregation and
direct owner checks now cover the extracted hook/component files, while the
existing assertions continue to pin the same token rejection, launch,
reconnect, stage, highlighting, date-time, and read-only behavior. No
production code or user-facing behavior changed in this follow-up.
