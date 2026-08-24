# Issue #356 implementation summary

## Outcome

Next.js canvas-game acceptance now distinguishes a changed logical contract
from a canvas that stopped redrawing. In that case the runtime failure remains
`browser_interaction_failed:input_state_change_missing_after_start`, the
evidence includes `canvas_not_redrawn_after_start`, and repair guidance leads
with restarting the render loop before the existing input-wiring checklist.
Restart evidence remains available, but a consequential restart gap no longer
displaces the application input failure as the primary reason.

## Changes

- The embedded interaction probe holds one movement/fire key while polling the
  contract marker, releases it after observation, and stops dispatching keys
  after the first state change. This lets held-key applications expose changes
  such as `player_x` without a later opposite key erasing the observation.
- Canvas snapshots now keep distinct `before_start`, `after_start`,
  `after_inputs`, and `after_recovery` samples. When logical contract state
  changed but the readable canvas pixels stayed identical across the started
  interaction window, the probe records
  `canvas_not_redrawn_after_start`, preserves the logical change separately as
  `input_contract_state_change`, and fails visible input honestly.
- Behavioral arbitration gives the observed input-failure family priority over
  consequential missing evidence such as restart/recovery. Probe
  infrastructure failures retain their prior unverified/partial behavior.
- Declarative Next.js knowledge and interaction repair selection add the exact
  canvas non-redraw finding and put its `requestAnimationFrame` remedy first.
- The state-binding scanner follows bounded, simple local initializer aliases,
  so `data-anvil-state={anvilState}` resolves through `JSON.stringify(...)` to
  its React `snapshot` dependency instead of being misclassified as
  `state_bound_to_ref`.
- The Next.js profile contract documents held-key input, canvas redraw
  evidence, and primary-reason ordering. A new Issue #356 corpus case freezes
  the source, evidence, and repair-guidance contract.

## Scope and compatibility

The evidence schema additions are backward compatible, historical run records
were not modified, and the live `.anvil/` namespace was untouched. No changes
were made to the `src/planner/runner.rs` or
`src/minimal_loop/loop_run.rs` growth tripwires. The existing generality
guardrail baseline was not raised.
