# Issue 285 Design

## Problem

Issue 205 connected the admitted python-cli C1-C4 runtime to
`ultra_final_acceptance`, but the ordinary `--plan-run` boundary still reaches
`plan_final_contract` without dispatching a profile behavior probe. That event
can report full assurance from static/runtime gates while no
`evidence/cli-assurance.json` exists. Direct-command finalization then rereads
the absent evidence and honestly projects `static (cli_probe_not_run)`, so the
event, human summary, and headless summary disagree.

## Change

- Add a typed `ProfileRuntime` policy hook that opts only canonical
  `python-cli` and the backward-compatible `cli` profile into plan-final
  behavior-probe dispatch.
- At `plan_final_contract`, run the existing profile behavior probe after the
  existing runtime/release checks, bind its pass/partial/static/failed result to
  runtime acceptance, release-gate, final-acceptance, and assurance fields, and
  emit the existing `profile_behavior_probe` event.
- Keep `evidence/cli-assurance.json` as the terminal projection source of truth.
  A passing C1-C4 probe therefore yields full assurance consistently in the
  plan-final event, summary, and headless summary.
- Preserve honest failure: a failed probe fails the release gate and cannot
  earn full assurance; an unexecuted or unpersisted probe remains static during
  terminal evidence rederivation.
- Add fields only to `plan_final_contract`; do not rename events or change
  existing field types. Leave runner and minimal-loop chokepoints unchanged.

## Regression coverage

- Exercise the production `run_step_plan` path with canonical `python-cli`, a
  manifest-shaped CLI, and passing C1-C4 checks. Assert assurance evidence,
  probe/final events, completion projection, rendered summary, and headless
  summary all report full assurance.
- Exercise a failed C1 probe and assert the plan run fails with failed/non-full
  evidence and gate fields.
- Retain the existing unexecuted-evidence completion tests that project static
  assurance.
- Add a corpus fixture freezing the compatible plan-final probe/evidence
  binding fields.

## Verification

Run focused python-cli plan-final and completion-metadata tests, the CLI corpus
regression, then `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test` because shared
plan-final event/acceptance code is touched.

## UAT fallback-evidence addendum

The exact PR-head UAT run used the ordinary `src/anvil_app/main.py` layout, so
the profile runtime correctly selected the fallback behavior probe rather than
the manifest-driven `cli/main.py` C1-C4 probe. The fallback persisted a passing
`.anvil/evidence/python-cli-behavior.json`, and `plan_final_contract` earned full
assurance, but terminal projection unconditionally reread the absent
`evidence/cli-assurance.json` and replaced that result with static assurance.

Keep the canonical C1-C4 artifact authoritative when it exists. When it is
absent, terminal projection may preserve an already-earned full result only if
all current plan-final gates pass and the fallback artifact is structurally
compatible and bound to an existing `src/<package>/main.py`: profile and pass
fields agree, reasons are empty, both executions exited zero, both stdout
observations are non-empty and different, and `changed_by_input` is true. The
fallback artifact alone never earns assurance. Failed gates or failed,
missing, malformed, contradictory, stale, or unexecuted fallback evidence
therefore remain non-elevated.
