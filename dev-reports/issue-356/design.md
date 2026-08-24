# Issue #356 design

## Observed failure

The failed GUI Trial reached a usable contract-mode page and observed the
primary transition from `start` to `playing`. Its final evidence nevertheless
reported `input_state_change_missing_after_start` while bounded repair kept
selecting `restart_or_recoverable_state_evidence` as the primary missing item.
The original artifacts expose three concrete causes:

- the interaction probe dispatched `keydown` and `keyup` without holding the
  key across an application update tick, then sent both horizontal directions;
- the generated restart path cancelled the drawing `requestAnimationFrame`
  loop and restarted only the logic interval, while identical non-blank canvas
  hashes were not classified as a stopped redraw loop; and
- `data-anvil-state={anvilState}` was diagnosed as `state_bound_to_ref` because
  the scanner treated the derived local variable as non-reactive instead of
  following its initializer to the `snapshot` React state.

## Design

1. Change the browser probe's game-input model to hold each key while polling
   the active marker. Stop after the first observed state change so an opposite
   direction cannot cancel the observation. Preserve existing dispatch labels
   and add step telemetry for the held-key model.
2. Preserve the pre-recovery `after_inputs` canvas sample and compare its
   readable pixel hashes with `before_start` and `after_start`. When contract
   state changed but the canvas remains byte-identical throughout the started
   interaction window, record `canvas_not_redrawn_after_start`, treat the
   visible input result as missing, and keep the changed contract dimensions so
   the evidence can show that logic state (for example `player_x`) changed
   while rendering did not.
3. After behavioral arbitration, prefer an application interaction failure as
   the runtime acceptance primary reason over the evidence keys that failed as
   consequences of that same observation. Infrastructure failures retain their
   existing partial/inconclusive handling.
4. Put the exact canvas finding and its render-loop remedy first in Next.js
   interaction repair guidance, followed by the existing input-wiring guidance.
5. Resolve simple local `data-anvil-state` aliases to their initializer
   dependencies before classifying bindings. Keep complex or unknown aliases
   conservative. This makes a derived alias backed by `useState` reactive
   without treating unrelated refs as React state.

## Compatibility and scope

The evidence schema changes are additive: existing fields and failure names
remain available, while `canvas_not_redrawn_after_start` and held-key steps add
diagnostic detail. Runner and minimal-loop chokepoints require no wiring change.
The original run records remain immutable.

## Verification

Add focused unit tests for held-key script content, non-redraw normalization and
guidance, interaction-primary arbitration, and derived state aliases. Add an
Issue #356 corpus fixture representing the evidence/source contract. Because
Next.js declarative guidance changes, run the Space/Breakout/Quiz corpus matrix
along with the focused tests, then run formatting, Clippy, and the full Rust
test suite.
