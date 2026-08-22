# Issue #228 design: editable plan YAML workflow

## Scope and current behavior

`--plan-steps` and `--ultra-plan` currently save bare YAML and print only its
path. Users must infer which fields are safe to edit, which validation the
runner will apply, and which command runs the edited file. Invalid hand-edited
files are discovered only when execution starts, and the custom plan parsers do
not attach source locations to their errors. Recovery UltraPlans carry machine
metadata but do not summarize the bounded difference between retained work and
the remaining repair.

Issue #228 adds an offline edit/validate/run workflow without changing the
StepPlan, UltraPlan, recovery metadata, event, or `.anvil` runtime schemas.

## Design

- Add leaf modules under `src/planner/plan/` for editable rendering, validation,
  command guidance, and recovery diff comments. `src/cli.rs` exposes
  `--validate-plan <PATH>` as an exclusive action; top-level dispatch performs
  the read-only validation before provider/config initialization.
- Keep the canonical `render_step_plan` and `render_ultra_plan` functions
  unchanged for prompts, fixtures, and parser round trips. Only saved user-
  editable plan files receive comments. The existing first stdout line remains
  the saved path, followed by explicit validate and run commands.
- Parse YAML syntax with `serde_yaml` to obtain line and column data, then parse
  the detected StepPlan or UltraPlan shape and run the existing execution lint
  report. All reported failures use `path:line:column: reason`; no verification
  or lint rule is weakened. Validation is offline and read-only.
- Detect StepPlan versus UltraPlan from the mutually exclusive top-level
  `steps` and `phases` keys. A recovery UltraPlan remains an UltraPlan for
  execution and is identified only for success output by its existing recovery
  metadata.
- Prefix recovery YAML with comment-only diff lines derived from the existing
  `RecoveryHandoff`: retained changed paths, missing paths/capabilities, repair
  targets, and checks to rerun. Comments preserve the established recovery
  schema and remain ignored by old parsers.
- Add bilingual `plan-yaml.md` guides and keep the public CLI references/counts
  synchronized. Focused integration tests cover valid and invalid CLI behavior,
  comments, next commands, recovery summaries, and doc drift. A small corpus
  fixture pins the recovery YAML comment contract.

## Compatibility and verification

Existing `--run-plan` and `--run-ultra-plan` behavior remains unchanged.
Generated files stay parseable by the existing parsers, and recovery metadata
and event fields do not change. Verification will run the focused Issue #228
CLI tests and doc-drift/corpus checks first, followed by formatting, Clippy, and
the full Rust suite because the public CLI surface and shared saved-plan
behavior change.
