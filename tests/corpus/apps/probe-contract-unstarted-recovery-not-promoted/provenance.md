# Fixture provenance

This fixture is synthetic. It is derived from the S-118b and S-119a
investigations and the probe at develop 46d99b14 (IC-26d, Issue #874); no
generated app or recorded run was copied.

`src/app/page.tsx` is a minimal contract-mode app: a `data-anvil-action="primary"`
control, then a small `Go` button that moves the state (second in the DOM, smallest
area, so it is outside the four ranked fallback buttons), then four `Tap` buttons
that leave the exported state untouched. The app never starts on the primary
press, so the probe observes no start.

`browser-interaction.json` is the evidence the probe writes for that app after
IC-26d. The legacy recovery is not tried in contract mode when no start was
observed, so `recovery_transition_status` is `not_observed`,
`recovery_control` is `none`, there is no `start_transition` step, and the run
fails with `start_transition_missing`. Before IC-26d the legacy recovery
pressed `Go` (the first button whose text differs from the primary's), counted the change as the start, and the run
reached `ok: true` with a full release gate for an app that never showed a start.

No restart is required, and `state_dimensions_changed` is empty because no input
was dispatched after a start. The evidence-level qualification therefore is not
full (`failed:contract_instrumentation_missing:state_change`).
