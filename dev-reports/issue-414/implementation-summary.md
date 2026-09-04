# Issue #414 implementation summary

## Outcome

Terminal Recovery Plan selection now follows the automatic Recovery
transaction outcome. A handoff emitted inside an isolated treatment is staged
until the existing promotion events resolve the transaction. Rejection or
control retention discards the staged handoff; promotion commits it. An
unresolved treatment cannot replace the last control handoff, and a successful
automatic Recovery still clears obsolete recovery fields.

## Changes

- Added `src/eval_events/recovery_resolution.rs` as the single leaf module for
  replaying Recovery handoff lineage from existing events.
- Updated completion snapshots and reads of `tui_command_stop` to overlay the
  resolved recovery fields. Terminal projection, run inventory, `/resume`, and
  boundary directive continuation therefore use the same plan.
- Updated GUI failure explanation projection to select its matched
  `recovery_prompt_saved` event from the same resolved lineage.
- Preserved terminal-only legacy streams as a fallback while preventing a
  projected terminal record from overriding an available source handoff.
- Added focused regression tests for rejected, promoted, unresolved,
  successful, and legacy sequences; terminal emission; GUI projection;
  `/resume`; and additional-request directive derivation.
- Added the `issue414-recovery-resolution` corpus case with rejected and
  promoted treatment event sequences.

## Compatibility

No existing event name or field changed. No existing fixture was modified. The
new resolver consumes the existing `recovery_plan_auto_run_start`,
`recovery_control_retained`, `recovery_treatment_promoted`,
`recovery_promotion_decision`, and successful auto-Recovery events.
