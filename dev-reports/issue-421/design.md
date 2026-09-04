# Issue #421 design

## Problem

Runtime evidence accepts two valid interaction shapes: a state change after a
start transition, and an input-driven state change on a visible surface where
the probe explicitly found no start control. The planner release gate only
accepts the first shape, so successful form-style probes are downgraded to
`interaction_detail_missing` and final acceptance becomes partial.

## Change

- Reuse the existing interaction-qualification leaf module to identify the
  second, startless interaction shape from backward-compatible evidence fields.
- Let the planner release-evidence classifier accept an input state change when
  either a start transition was observed or the visible surface explicitly has
  no start control.
- Preserve explicit failures and the existing requirement for an input state
  change. Game evidence with `start_transition` remains on its current path.
- Add a corpus fixture modeled on the affected Next.js form evidence and a
  focused release-gate regression test that also checks the final-acceptance
  projection. Keep existing event names and schemas unchanged.

## Verification

Run the focused Issue #421 regression and interaction-qualification unit tests,
then formatting, clippy, the full Rust suite, corpus regression, conformance,
and growth guardrails because shared acceptance behavior is affected. Run the
complete repository CI script as a supplemental baseline check.
