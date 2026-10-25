# Fixture provenance

`src/app/page.tsx`, `src/lib/game-logic.ts` and `src/lib/types.ts` are the
generated app files of the L-111 GAME G run (G1 stage 3a, develop eadcaa42),
copied verbatim. The app's only `data-anvil-action="restart"` hook is inside
the WON/GAMEOVER overlay, so the interaction probe never reached it
(`restart_hook_count_after_start: 0`) and the release gate reported
`contract_instrumentation_missing:restart` as failed.

`browser-interaction.json` keeps only the fields that the qualification reads
plus the status fields of the measured evidence; the Playwright resolution,
probe paths and canvas snapshots (which carried host paths) were dropped. No
behavioral field was changed.

The evidence-level qualification of this evidence stays
`failed:contract_instrumentation_missing:restart`. The release gate maps it to
partial (`interaction_unverified:terminal_state_not_reached`) only when the
route-bound source carries the restart hook and its handler replaces state with
a closure factory that returns a score/entity reset. That gate-level expectation
lives in `src/planner/interaction_qualification/restart_terminal_tests.rs`.
