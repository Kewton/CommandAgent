# Issue 243 Implementation Summary

## Changes

- Extended the existing time-profile aggregation with additive provider usage
  grouped by `planner`, `executor`, `repair`, and `acceptance-repair`.
- Added per-role provider wall time, provider-reported prompt tokens,
  generation tokens, thinking/reasoning tokens, and prefill ratio.
- Added a `Provider usage by role` table to generated `summary.md` files.
- Added `provider_usage_by_role` to `--summary-json` and to the nested
  time-profile event projection without renaming or removing existing keys.
- Kept unavailable provider telemetry honest as JSON `null` and Markdown
  `n/a`; runs without provider turns expose an empty JSON object.
- Documented the additive headless JSON field and updated the focused
  headless-summary corpus fixture.

## Tests

- Covered planner/executor aggregation, repair and acceptance-repair role
  separation, prompt/generation/thinking token projection, prefill ratios, and
  missing telemetry behavior.
- Covered generated `summary.md`, headless fixture projection, final-line CLI
  JSON behavior, omitted-flag stdout compatibility, and corpus expectations.
