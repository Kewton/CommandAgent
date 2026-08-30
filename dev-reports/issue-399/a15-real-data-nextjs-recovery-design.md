# Issue 399 A15 real data / Next.js Recovery design

Status: draft inputs implemented; live collection not started

Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`

Required execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`

## Purpose

The frozen A14-A14 eligible population contains real CLI and generic fix tasks.
Its data and Next.js rows are exclusion sentinels: the data task has an
intentionally unavailable offline dependency, while the Next.js-labelled task
explicitly forbids a Next.js implementation. Those sentinels are useful for
checking that Recovery is suppressed, but they cannot support a claim that one
Recovery improves quality in every product profile.

A15 adds executable, profile-conformant data and Next.js tasks without changing
or pooling the historical A14-A14 run.

## Added eligible tasks

### Data: cell-13

- 10 distinct hash-frozen CSV fixtures.
- A real deterministic data-profile pipeline at `pipeline/main.py`.
- The before stage violates `input_rows = used_rows + excluded rows`; the
  reference repair corrects the accounting without changing the fixture.
- The exact task-bound reproducer changes from exit 1 to exit 0.
- Frozen regressions run pytest and `scripts/contract_check.py`.
- Standard data outputs are present and regenerated:
  `output/inspection.json`, `output/results.json`, and `output/report.md`.

### Next.js: cell-14

- 10 distinct hash-frozen JSON fixtures and DOM selectors.
- A real Next.js 16.3.1 App Router project with React 19.2.8.
- Dependencies and Chromium are restored from the existing hash-bound offline
  provisioning bundle; network installation remains forbidden.
- The exact Node reproducer and rendered route share `lib/label.mjs` as the
  defect boundary.
- The final frozen oracle requires the exact reproducer, Node regression,
  production build, and Chromium DOM observation to pass.

## Collection design

The A15 smoke contains 14 pairs:

- three repeats from one real task in each of CLI, generic, data, and Next.js;
- one dependency sentinel and one profile-contract sentinel.

Smoke GO additionally requires at least one Recovery execution and usable
external oracle evidence in each of the four real profiles. A pooled smoke
success is insufficient.

The draft full design contains 140 pairs:

- 120 eligible pairs: 4 profiles × 10 tasks × 3 repeats;
- 20 sentinels: 10 dependency and 10 profile-contract exclusions.

An “all profiles improve” claim requires all of the following:

- the pooled 2,000-sample task-cluster bootstrap 95% CI lower bound is above 0;
- each profile-specific 2,000-sample task-cluster bootstrap 95% CI lower bound
  is above 0;
- at least five Recoveries execute in each profile;
- no existing-artifact harm, regression, instrumentation-unusable record, or
  sentinel Recovery occurs;
- wall-time and token budgets pass both overall and per profile.

The full contract remains draft. It cannot be frozen until the A15 smoke is GO
and profile-specific wall/token budgets are fixed without observing full-run
outcomes.

## Local verification completed

- Task registry validation: 0 errors for all 60 selected eligible/sentinel cases.
- New task binding and oracle-semantic validation: 20/20 valid.
- Data exact reproducer: before exit 1 and reference exit 0 for all 10 fixtures.
- Data frozen regressions: pass in both stages.
- Next.js exact reproducer: before exit 1 and reference exit 0 for all 10
  fixtures; frozen Node regression passes in both stages.
- Offline Next.js production build: pass with Next.js 16.3.1 and webpack.
- Frozen Chromium DOM oracle: pass for `#result-01 = ready-01`.
- A15 generator: deterministic output hash confirmed.
- Focused Python tests and existing goal-verify regression suites: pass.

## Main files

- `eval/goal_verify/v0/phase6-recovery-v4-a15-corpus.json`
- `eval/goal_verify/v0/phase6-task-contracts-v4-a15.json`
- `eval/goal_verify/v0/phase6-command-adapters-v4-a15.json`
- `eval/goal_verify/v0/phase6-real-workspaces-v4-a15.json`
- `eval/goal_verify/v0/phase6-recovery-v4-a15-smoke-contract.json`
- `eval/goal_verify/v0/phase6-recovery-v4-a15-full-contract.json`
- `tests/fixtures/goal_verify_v4/a15/`
- `scripts/eval_lib/generate_goal_verify_recovery_v4_a15.py`
- `scripts/eval_lib/goal_verify_recovery_a15_report.py`
