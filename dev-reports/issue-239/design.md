# Issue #239 design

## Context

The Python CLI profile manifest fixes three create phases: `cli-scaffold`,
`cli-implementation`, and `cli-validation`. The measured Issue baseline still
called the phase step planner for all three phases, so each response repeated
setup and verification work that the profile already owns. The Issue records
10.8k planner tokens across those three calls.

## Design

- Add a leaf planner module for Python CLI phase-plan synthesis. It classifies
  the fixed manifest phases and builds the scaffold setup step and validation
  verify step from the profile's artifact and verification contracts.
- Wire `PythonCliProfile::deterministic_step_plan` and
  `PythonCliProfile::preset_ultra_plan` to those existing typed profile
  boundaries. The generic runner already skips the model when a profile
  returns a deterministic phase plan; runner wiring is limited to invoking
  the leaf canonicalizer for the one model-owned phase.
- Keep `cli-implementation` model-owned. Add an explicit implementation-only
  instruction to profile guidance and canonicalize its returned StepPlan:
  retain implement steps, remove model-authored setup/verify commands and
  setup-path ownership, and ensure the profile-required entrypoint and usage
  document remain implementation outputs. An empty result continues to fail
  deterministic lint and retry rather than being accepted.
- Preserve the existing final profile invariant, compile oracle, behavior
  probe, and manifest-bound acceptance checks. The generated validation step
  uses the profile's preferred `python3 -m compileall -q src` command; no gate
  is weakened.

## Verification strategy

- Unit-test exact deterministic setup and verify projections and strict
  implementation-only canonicalization.
- Add a corpus fixture that records the measured three-call/10.8k-token
  baseline and the one model-owned phase projection. Assert at least 30%
  reduction and exact contract-matching phase/step ownership.
- Run focused planner/profile and corpus regression tests, then formatting,
  Clippy, and the full Rust test suite because shared planner dispatch is
  affected.
