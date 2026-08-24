# Issue #239 implementation summary

## Outcome

Python CLI profile-preset planning now keeps machine-owned work out of the
phase planner:

- `cli-scaffold` returns a deterministic setup StepPlan for
  `pyproject.toml` and the goal-derived `src/<package>/main.py` scaffold.
- `cli-validation` returns a deterministic verify StepPlan using
  `python3 -m compileall -q src`.
- `cli-implementation` is the only model-owned phase. Its prompt explicitly
  requests implement steps only, and the leaf canonicalizer removes any
  model-authored non-implement steps, verify commands, or setup-path ownership
  while preserving the entrypoint and README implementation contract.

The existing `DomainProfile` deterministic-plan and profile-preset hooks do
the phase-level dispatch. The guarded runner change is limited to one leaf
canonicalizer call plus its sanitization accounting; no guardrail baseline was
changed.

## Tests and fixtures

- Added focused leaf tests for manifest-routed setup/verify plans, template
  lint, implementation-only canonicalization, profile scoping, event schema,
  and token-reduction arithmetic.
- Added `tests/corpus/apps/issue239-python-cli-plan-synthesis/` with the
  measured 10.8k-token/three-phase baseline, one-model-phase projection,
  exact setup/verify contracts, and the additive canonicalization event.
- The normalized projection is 3,600 planner tokens versus the measured
  10,800-token baseline, a 66% reduction and above the required 30% floor.
