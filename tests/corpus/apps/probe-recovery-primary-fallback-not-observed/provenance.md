# Fixture provenance

`src/app/page.tsx`, `src/lib/game-logic.ts` and `src/lib/types.ts` are the
generated app files of the L-111 GAME G run (G1 stage 3a, develop eadcaa42),
copied verbatim from `restart-overlay-only-partial`. The app has no
`data-anvil-action="restart"` hook while playing, so the contract probe fell
back to the primary control (`restart_hook_count_after_start: 0`).

`browser-interaction.json` is the `restart-overlay-only-partial` evidence as it
looks after IC-26b (Issue #867). The primary press moved `{score, shake}`, but a
primary press is not a restart, so `recovery_transition_status` is
`not_observed` and `recovery_control` is `primary_fallback`; the recorded run
had read the self-clearing `shake` as `observed`. No other field was changed.

The evidence-level qualification stays
`failed:contract_instrumentation_missing:restart`: it does not read the recovery
status. With `recovery_transition_status: not_observed` the behavioral decision
for `restart_or_recoverable_state_evidence` is
`unverified:terminal_state_not_reached` when the route-bound source carries the
restart hook, and the release gate is partial. That decision is asserted in
`src/minimal_loop/behavior_evidence.rs` and
`src/planner/interaction_qualification/restart_terminal_tests.rs`.
