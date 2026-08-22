# Issue #228 implementation summary

## Outcome

CommandAgent now provides an offline edit/validate/run workflow for saved plan
YAML. Generated step plans and UltraPlans contain comment-only editing help,
the CLI prints the exact validation and execution commands after saving, and
`--validate-plan <PATH>` reports located YAML or plan-lint diagnostics without
executing the plan or initializing a provider.

Recovery UltraPlans now begin with a bounded comment-only diff summary covering
retained changed paths, missing paths and capabilities, repair targets, and
checks to rerun. The established YAML metadata and event schemas are unchanged.

## Implementation

- Added `src/planner/plan/` leaf modules for commented templates, shell-safe
  next-command guidance, offline plan validation, and recovery diff rendering.
- Added the exclusive public `--validate-plan` action and dispatched it before
  runtime configuration/provider setup. Step-plan validation reuses the audited
  execution lint contract; UltraPlan validation reuses its existing lint report.
- Kept runner wiring to one saved-template call and kept the guarded step-plan
  finalizer within its production-line budget by exposing the existing validator
  through that chokepoint.
- Preserved the saved path as the first output line and appended explicit
  validate/run guidance for both plan types.
- Added English and Japanese Plan YAML guides, updated public flag references,
  and pinned documentation parity with doc-drift tests.
- Added focused CLI/unit coverage and a recovery YAML corpus fixture. The tests
  cover valid commented plans, located syntax/schema/lint failures, action
  conflicts, next commands, parser round trips, and recovery summaries.

## Compatibility

Existing plan parsers ignore the new comments, and the canonical prompt/fixture
renderers remain unchanged. Existing run flags, recovery metadata fields, event
names and schemas, and the live `.anvil/` namespace are unchanged.
