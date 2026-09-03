# Issue #414 design

## Problem

Automatic Recovery executes a candidate in an isolated treatment workspace. A
failed treatment can emit a newer `recovery_prompt_saved` record before the
transaction records `recovery_control_retained` and a rejected promotion
decision. Terminal projection currently scans recovery fields in reverse event
order, so it selects that discarded treatment record instead of the recovery
plan that remains valid in the control workspace. GUI failure projection also
selects the newest matching `recovery_prompt_saved` independently, allowing its
display and follow-up source to disagree with the terminal event used by
`/resume` and directive continuation.

## Decision

Add a leaf recovery-resolution module under `src/eval_events/`. It will replay
only the existing event names and fields, treating a
`recovery_plan_auto_run_start` as the treatment boundary. Recovery handoff
records emitted inside that boundary are staged until the existing promotion
decision is observed:

- rejected or retained-control treatments discard staged recovery records;
- promoted treatments commit staged recovery records as the current lineage;
- an unresolved treatment does not supersede the last control record;
- successful automatic Recovery clears the prior handoff, preserving current
  behavior;
- streams without a treatment boundary keep their existing newest-record
  behavior.

No event names, fields, schemas, or existing fixtures change.

## Integration

`latest_completion_snapshot` will obtain its recovery fields from the resolved
event lineage. `failure_explanation::project` will select its phase/step-matched
recovery record from that same lineage. The terminal event therefore carries
the same recovery plan shown by the GUI. Run inventory and `/resume` already
consume the terminal/completion projection, while directive continuation
already derives from `tui_command_stop`, so no separate continuation policy is
introduced.

## Verification

Add focused unit coverage for rejected, promoted, unresolved, and successful
Recovery transaction sequences, including GUI failure projection. Add an
Issue #414 corpus fixture that preserves the existing schema while recording
both rejected and promoted lineages. Run the focused tests first, followed by
formatting, Clippy, and the full Rust test suite because shared event projection
is touched.
