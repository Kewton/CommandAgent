# Issue 285 implementation summary

## Outcome

Python CLI create runs through ordinary `--plan-run` now execute the admitted
C1-C4 behavior probe at the plan-final boundary. A passing probe persists
`evidence/cli-assurance.json`, and the probe event, plan-final event, human
summary, and headless summary all project full, non-static assurance.

## Implementation

- Added a typed profile-runtime policy hook that enables plan-final probe
  dispatch for canonical `python-cli` and the backward-compatible `cli`
  profile only. Other profiles and non-create intents retain their existing
  behavior.
- Added a leaf plan-final probe adapter that reuses the existing behavior-probe
  runtime and event. It binds failed probes to the failed release gate and
  binds partial or static results to a non-full release gate without weakening
  any acceptance threshold.
- Reused the completion metadata evidence classifier for non-passing probe
  projections, keeping absent, unexecuted, or unpersisted evidence static and
  failed evidence failed.
- Extended `plan_final_contract` additively with behavior-probe status, reasons,
  and evidence-path fields. Existing event names and field types are unchanged.
- Added focused production-path tests for passing and failing Python CLI probes.
  The passing case verifies the evidence file, probe/final events, terminal
  metadata, rendered summary, and headless summary; the failure case verifies
  that the run cannot earn full assurance.
- Added a Python CLI corpus fixture that freezes the full plan-final and
  headless assurance projection.

No README, demo asset, historical run evidence, live `.anvil/` state,
`src/planner/runner.rs`, or `src/minimal_loop/loop_run.rs` changes were needed.
The #259 GUI smoke and provider-backed `--plan-run` evidence comment remain the
post-merge follow-up after this commit reaches `develop`.
