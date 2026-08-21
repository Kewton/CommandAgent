# Issue #168 design: run selection request ownership

## Current behavior

The repository-run page stores a run detail and selected evidence independently
from the run ID that produced them. Changing the select value updates `runId`
before the next detail effect clears any state, so React can render the previous
run's acceptance sheet under the new selection. An older aborted request also
runs an unconditional `finally` callback and can clear the loading state owned
by a newer request. Selecting the empty option returns early from the effect
without clearing the previous detail, leaving its evidence list visible.

Evidence reads have the same ownership gap: a read started for one run can
settle after another run is selected and overwrite the visible document state.

## Design

- Associate every loaded detail and evidence document with the run ID that
  requested it. Derive renderable state only when that owner matches the
  current `runId`, so a selection change makes old content ineligible during
  the same render.
- Give detail and evidence requests separate abort controllers and current-run
  guards. Only a request still owned by the selected run may publish data,
  errors, or loading completion. Abort outstanding evidence work whenever the
  run changes and whenever a newer evidence read begins.
- Clear detail, evidence, error, and loading state when the empty option is
  selected. The empty selection must render neither the evidence list nor a
  document viewer.
- Preserve the existing routes, response schemas, history URL behavior, and
  read-only server contract.

## Verification strategy

- Extend the existing source guard to pin run-owned detail/evidence state,
  cancellation guards, and explicit empty-selection clearing.
- Extend the read-only Playwright smoke with delayed synthetic run-detail
  responses. It will load one run, switch while another response is pending,
  prove the first acceptance marker disappears immediately and cannot return,
  then select the empty option and prove both the evidence list and document
  viewer are absent.
- Run GUI lint, typecheck, and both root/proxy builds, the focused Rust source
  guard, and the focused read-only browser smoke. Run repository formatting,
  Clippy, and the full Rust suite because the checked-in GUI contract test is
  shared CI surface.
