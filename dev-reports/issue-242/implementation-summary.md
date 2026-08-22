# Issue #242 implementation summary

## Change

- Added `src/planner/pipeline.rs` as the owner of speculative next-phase
  planning. It starts only the first provider-bound StepPlan reply while the
  current phase's profile-invariant verification is running.
- Kept parsing, linting, presentation, persistence, execution, and the ordinary
  plan events behind the existing phase-boundary Gate. An adopted raw reply is
  fed back through the existing plan-resolution path exactly once.
- Added exact phase/prompt keys, cancellation on verification failure, and
  synchronous fallback for stale input, worker failure, or provider failure.
  Fix, investigation, synthesized, deterministic, and promotion-eligible runs
  remain on the sequential path.
- Added additive `speculative_phase_plan_started`,
  `speculative_phase_plan_adopted`, and
  `speculative_phase_plan_discarded` lifecycle events.

## Wiring and coverage

- Limited production wiring to `src/planner/runner/phase/flow.rs`; the
  top-level `src/planner/runner.rs` is unchanged and the flow chokepoint grows
  by only two net lines.
- Added focused overlap, failed-Gate cancellation, stale-key, and single-use
  adoption tests in the new pipeline module.
- Updated the intermediate invariant-repair test to provide the replacement
  reply required after a speculative reply is correctly discarded.
- Added pass and failed-Gate event-order fixtures under
  `tests/corpus/apps/issue242-speculative-phase-pipeline/`.
