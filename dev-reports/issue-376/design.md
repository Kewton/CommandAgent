# Issue #376 design: task-level Trial progress and results

## Baseline and scope

Issue #370 supplies the split Trial status and result-detail routes, and Issue
#375 supplies additive `plan_step_started`, `plan_step_completed`, and
`plan_step_failed` events. Integrate those committed predecessors first. Keep
history rows compact and put task detail only on status and result detail.

## Projection contract

- Extend the existing read-only session status response with one bounded-field
  task projection. Preserve every `plan_execution_id` as a separate execution
  interval and every `step_execution_id` as a separate task, so continuations
  and duplicate Step IDs cannot be merged.
- Accept only Issue #375 schema-version-1 `plan_step_*` records. A started task
  remains running until its matching typed terminal record arrives. Derive
  completed, short-circuited, failed, and interrupted only from typed terminal
  fields; do not infer success from later events, phase completion, or stream
  position.
- Report `pending` before a running session emits its first typed task event,
  `supported` when a consistent typed projection exists, and `unsupported` for
  a terminal or inconsistent stream that cannot provide trustworthy task
  results. Unsupported projections expose no guessed counts.
- Return only display-safe typed fields already bounded and redacted by #375:
  identities, position, kind, terminal outcome, verification summary, changed
  paths, repair count, and failure summary. Do not add raw events to polling.
  Existing ETag/304 polling keeps unchanged responses body-free; roughly 100
  tasks therefore grow payload and rendering only linearly.

## GUI behavior

- Add one shared task-progress component below the phase view on status and
  below the verdict on result detail. During execution it announces the current
  phase, task ID, and one-based task/total position.
- Group tasks under execution intervals and phases. Give each terminal state a
  text label and symbol in addition to color. Failed task disclosures start
  expanded and show the failure summary, verification failures, changed paths,
  and a button to open `events.jsonl` as related evidence.
- Use native disclosure controls with synchronized `aria-expanded`, ordered
  headings, and keyboard-operable buttons. Keep history unchanged.

## Verification

- Add focused Rust projection tests for typed-only aggregation, duplicate Step
  IDs across execution intervals, all terminal outcomes, invalid/incomplete
  terminal contracts, and legacy unsupported sessions.
- Extend the GUI route smoke for running and terminal task views under `/` and
  `/proxy/commandagent/`, direct reload/reconnect, failure auto-expansion,
  evidence navigation, keyboard/disclosure accessibility, non-color labels,
  continuation isolation, and a roughly 100-task payload/render case.
- Run GUI syntax, lint, typecheck, build, focused browser smoke, focused GUI
  server and read-only guard tests, then repository formatting, Clippy, and
  Rust tests because the shared session response contract is additive.

## Preserved boundaries

No mutating GUI endpoint, event rename/schema rewrite, verification shortcut,
runtime namespace change, or history-row expansion is introduced. Existing
phase projection remains unchanged and task state is never used to weaken Gate
3/Gate 4 or acceptance-sheet decisions.
