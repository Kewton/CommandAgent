# Issue #375 design

## Scope

Add an additive JSONL event contract around each `StepPlan.steps` execution. Existing event names,
payloads, session files, verification behavior, bounded repair, and acceptance/release gates remain
unchanged.

## Contract

Each invocation of the StepPlan runner creates a fresh `plan_execution_id`. Each entered step gets a
fresh `step_execution_id` shared by exactly one `plan_step_started` event and exactly one terminal
`plan_step_completed` or `plan_step_failed` event. All records also carry `session_id`, `mode`,
optional `phase_id`, the one-based `step_index`, `total_steps`, `step_id`, and normalized
`step_kind`. These identities keep additional requests, retries, and duplicate step IDs distinct
even when they reuse one session or the same plan content.

The completed or failed event records:

- `terminal_status`: `completed`, `skipped`, `failed`, or `interrupted`;
- `outcome`: `completed`, `completed_after_rollback`, `short_circuited`,
  `verification_failed`, `bounded_repair_failed`, `execution_failed`, or `interrupted`;
- explicit `completion_count_delta`, `failed_step_id`, changed paths, and verification status/count;
- repair attempt count and a bounded, redacted failure summary.

An already-satisfied verification is a successful `skipped` terminal with outcome
`short_circuited`. An interruption observed before the model turn still closes the just-started
step interval with an `interrupted` terminal. Normal and error returns from `run_step` are converted
without changing their control flow.

## Boundaries

The new event payload omits prompts, instructions, command bodies, model output, and file contents.
Changed paths and verification summaries use the existing event redaction/truncation helper and
fixed list caps, with explicit truncation booleans. Focused schema tests will assert event pairing,
identity uniqueness, outcome distinctions, bounded arrays/text, and secret redaction. A corpus
fixture under `tests/corpus/apps/issue375-plan-step-events/` will pin the public additive shape.

## Files

- Add a leaf lifecycle module under `src/planner/runner/phase/`.
- Add only module wiring in `src/planner/runner/phase.rs` and lifecycle calls in
  `src/planner/runner/phase/step_plan_execution.rs`.
- Add focused schema tests in the leaf module and runner-path assertions to existing test modules.
- Add the Issue #375 corpus fixture and required development reports.
