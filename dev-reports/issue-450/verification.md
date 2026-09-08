# Issue #450 verification

- Status: `passed`

Final broad verification used Python **3.12.3** with `login=false`. The original
failed run is retained below and in `regression-findings.json`; it was cleared by
restored local prerequisites, the explicitly authorized stronger test-only repair,
and a successful rerun of the unchanged broad command, not by a waiver.

## Checks

- `python3 -m pytest scripts/tests/test_browser_preflight.py -q`: `passed` — 64 tests after the two-viewport follow-up, including missing/mismatched second image, viewport changes and v1 rejection.
- `node --test scripts/tests/test_browser_plugin_probe.mjs`: `passed` — 5 tests after the follow-up.
- `node --check scripts/browser_preflight/plugin_probe_v2.mjs`: `passed`.
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/browser-preflight.py scripts/browser-preflight-smoke.py scripts/browser_preflight scripts/tests`: `passed`.
- `ruff format --check scripts/browser-preflight.py scripts/browser-preflight-smoke.py scripts/browser_preflight scripts/tests/test_browser_preflight.py scripts/tests/check_browser_preflight_smoke.py`: `passed`.
- `python3 -m pytest tests/eval/test_browser_interaction_oracle.py tests/eval/test_parity_gate_report.py tests/eval/test_eval_cli_contract.py tests/eval/test_eval_run_dry.py tests/eval/test_postcheck_dev_server.py -q`: `passed` — 43 passed, 2 existing optional cases skipped.
- `python3 scripts/browser-preflight-smoke.py run --output dev-reports/issue-450/smoke/isolated-profile --executable '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'`: `passed` — initial v1 smoke at commit `d5d358ad`; historical single-view evidence, not sufficient for the complete Issue scope.
- `python3 scripts/tests/check_browser_preflight_smoke.py --root dev-reports/issue-450/smoke`: `passed` — initial v1 checker at `d5d358ad`; historical result retained.
- `python3 scripts/browser-preflight-smoke.py run --output dev-reports/issue-450/smoke-viewports/isolated-profile --executable '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' --viewports scripts/tests/fixtures/browser-preflight/evaluation-viewports.json`: `passed` — actual desktop 1440x900 and mobile 390x844; both measured viewport and decoded PNG sizes match; freeze, guarded launch and owned-resource cleanup pass.
- `python3 scripts/tests/check_browser_preflight_smoke.py --root dev-reports/issue-450/smoke-viewports --viewports scripts/tests/fixtures/browser-preflight/evaluation-viewports.json`: `passed` — both viewport artifacts for both methods revalidated; isolated-profile desktop/mobile and plugin mobile visually inspected.
- `git diff --cached --check`: `passed`.
- `python3 -m pytest scripts/test_score_retrospective.py -q`: `passed` — 11 tests, 6 subtests, Python 3.12.3 / `login=false`.
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/test_score_retrospective.py`: `passed`.
- `python3 -m pytest tests/eval scripts/test_bon_select.py scripts/test_score_retrospective.py -q`: `passed` — final unchanged-command rerun: 571 passed, 11 skipped, 79 subtests passed in 36.03 seconds, Python 3.12.3 / `login=false`.

## Original broad regression failures and resolution

The initial execution of
`python3 -m pytest tests/eval scripts/test_bon_select.py scripts/test_score_retrospective.py -q`
**failed**: 4 failed, 567 passed, 11 skipped, 73 subtests passed. These original
observations remain intact in `regression-findings.json`. The gate remained blocked
through the viewport commit `9b313c5157db836b7d73d6a71dc7e2dc8017e653`.

| Unchanged test | Observed failure |
| --- | --- |
| `test_goal_verify_live_v2.py::GoalVerifyLiveV2Test::test_v2_runner_persists_independent_same_snapshot_arms` | Required `target/debug/verification_spec_validate` is absent in this worktree. |
| `test_goal_verify_v3.py::WorkspaceBaselineBlindAndReadinessTest::test_live_input_verification_rejects_wrong_binary_commit` | Missing commandagent/validator binaries cause an earlier prerequisite rejection than the test's expected commit check. |
| `test_goal_verify_recovery_a15.py::GoalVerifyRecoveryA15InputsTest::test_new_workspace_hashes_match_generated_fixtures` | Frozen fixture inventory references absent, ignored `before/.pytest_cache/.gitignore`. |
| `test_score_retrospective.py::ScoreRetrospectiveTests::test_repository_scan_covers_all_run_level_band_rows` | Existing historical inventory has 293 rows; the test expects 287. |

The orchestrator then restored the eight absent ignored A15 files at their existing
frozen hashes and built both debug/release commandagent and validator binaries.
It explicitly authorized a separate test-only maintenance commit for the remaining
retrospective failure, superseding the earlier instruction to leave that test
untouched. No caches or binaries are committed.

Source commit `d39c84a3368d8b59ca0d450b439ad32133e8b78e` added six Luna ingest rows
and updated `band_aggregate.py`, leaving the older retrospective assertion stale.
The maintenance preserves the original 287-row/profile/full=10 assertions on frozen
run IDs and additionally compares all 287 actual dictionaries against
`f1-retrospective-001/final-vectors.jsonl`. It rejects duplicate or unexpected IDs,
requires the exact six new Luna IDs to be ingest/full, and verifies current
total=293, ingest=60 and full=16. The original score=100 and every-atom=pass checks
still apply to **every** current full row. The resulting test is strictly stronger
than a denominator update: old content cannot drift and only the specified six
fully verified additions are admitted.

`gate-repair-integrity.json` records Python 3.12.3, five unchanged protected
production/history/manifest inputs and all eight restored prerequisite hashes.
`regression-findings.json` remains the original failure record. The final full
command passed without exclusions or changes to production scanners, historical
evidence, manifests or guardrail baselines. Existing 11 optional skips are reported,
not converted to passes.

## GUI observations and limits

The final v2 isolated-profile smoke report and immutable Browser manifest are under
`smoke-viewports/isolated-profile/`; actual browser/automation versions were measured
again rather than copied from historical evidence. `evaluation-viewports.json`
explicitly configures desktop 1440x900 and mobile 390x844; the library does not
silently assume those dimensions. Both per-view measurements, readable state and
images are saved. The full requirements are pinned into the manifest. No model
generation started. The earlier v1 `smoke/` records were preserved, not promoted
to two-viewport evidence or accepted by the v2 gate.

The plugin smoke used the current skill's existing Browser binding, listed zero
tabs, created its own tab and successfully read/clicked/captured the page at both
configured sizes using the documented viewport capability, then reset the temporary
override. Its JPEG dimensions and measured viewport sizes match both requirements. The
supported page environment did not provide navigator metadata; the exact error
and zero identical retries are recorded in `smoke/plugin/metadata-limitation.json`.
The plugin operation report is intentionally `blocked` solely for
`conditions_unverified`. `record` returned exit 2 and a freeze attempt was refused,
which are the expected honest gate outcomes. `smoke-viewports/plugin/smoke-result.json`
separately records verified operations and refusal of incomplete campaign pins.

The first v2 attempt failed to import a query-suffixed local helper path before
browser actions. Its exact error is retained in the first attempt report. A second
attempt used the real `plugin_probe_v2.mjs` path, with the reason recorded in its
ticket, and completed both views. No identical failed import was repeated and no
healthy browser connection was reset. Additional parent-conversation diagnostics
were read only and do not establish plugin permanent repair.

All owned fixture servers, temporary standalone browser/profile and task-owned
plugin tabs were closed. Existing browser connections and user tabs were retained.
Runtime ledgers/locks/start markers are unstaged; the retained artifacts are
completed observations, tickets, reports, images and immutable manifest evidence.
Saved smoke reports are historical observations, not permission to launch now;
fresh preflight is required for future evaluation. No Rust production code changed,
so Rust build/release checks are outside this change's verification scope.
