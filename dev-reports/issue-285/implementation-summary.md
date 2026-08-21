# Issue 285 implementation summary

## Outcome

Python CLI create runs through ordinary `--plan-run` now execute the admitted
behavior probe at the plan-final boundary. The manifest-driven `cli/main.py`
path persists `evidence/cli-assurance.json`; the ordinary
`src/<package>/main.py` path persists compatible fallback behavior evidence.
Both passing paths now keep the probe event, plan-final event, terminal events,
human summary, and headless summary on the same full, non-static assurance.

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

## UAT follow-up

The PR-head UAT exposed a terminal-only projection gap for the ordinary
`src/anvil_app/main.py` layout. Plan-final correctly ran the fallback probe and
earned full assurance, but completion metadata treated the absent canonical
C1-C4 artifact as an unexecuted probe and overwrote later terminal projections
with `static (cli_probe_not_run)`.

- Terminal metadata now retains the latest plan-final event's existing
  behavior-probe status and evidence path internally. No event name or emitted
  schema changed.
- The canonical C1-C4 artifact remains authoritative whenever it exists.
- When canonical evidence is absent, a full terminal result is preserved only
  when the current runtime, final-acceptance, and release gates pass, the
  current completion event binds the passing fallback path, and that artifact
  proves two successful, non-empty, input-sensitive executions of an existing
  `src/<package>/main.py`.
- A fallback artifact cannot independently earn full assurance. Failed,
  missing, malformed, contradictory, path-unbound, stale, or unexecuted
  evidence remains static/non-elevated.
- The focused production-path regression asserts that
  `evidence/cli-assurance.json` is absent, the fallback artifact is pass with
  `changed_by_input: true` and `src/anvil_app/main.py`, `plan_final_contract`
  is full/full-success/pass, and `tui_command_stop`, `run_stop`, `summary.md`,
  and headless summary all remain full.

The focused UAT fix leaves `src/planner/runner.rs`,
`src/minimal_loop/loop_run.rs`, README files, and demo assets unchanged.
