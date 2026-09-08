# Issue #450 verification

- Status: `blocked`

## Checks

- `python3 -m pytest scripts/tests/test_browser_preflight.py -q`: `passed` — 47 tests.
- `node --test scripts/tests/test_browser_plugin_probe.mjs`: `passed` — 4 tests.
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/browser-preflight.py scripts/browser-preflight-smoke.py scripts/browser_preflight scripts/tests`: `passed`.
- `ruff format --check scripts/browser-preflight.py scripts/browser-preflight-smoke.py scripts/browser_preflight scripts/tests/test_browser_preflight.py scripts/tests/check_browser_preflight_smoke.py`: `passed`.
- `python3 -m pytest tests/eval/test_browser_interaction_oracle.py tests/eval/test_parity_gate_report.py tests/eval/test_eval_cli_contract.py tests/eval/test_eval_run_dry.py tests/eval/test_postcheck_dev_server.py -q`: `passed` — 43 passed, 2 existing optional cases skipped.
- `python3 scripts/browser-preflight-smoke.py run --output dev-reports/issue-450/smoke/isolated-profile --executable '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome'`: `passed` — owned headed Chrome/loopback server; harmless action and image, freeze, denied changed-target launch, permitted harmless launch, cleanup.
- `python3 scripts/tests/check_browser_preflight_smoke.py --root dev-reports/issue-450/smoke`: `passed` — saved artifacts revalidated, both images visually inspected.
- `git diff --cached --check`: `passed`.
- `python3 -m pytest tests/eval scripts/test_bon_select.py scripts/test_score_retrospective.py -q`: `failed` — 4 failed, 567 passed, 11 skipped, 73 subtests passed.

## Broad regression failures retained

| Unchanged test | Observed failure |
| --- | --- |
| `test_goal_verify_live_v2.py::GoalVerifyLiveV2Test::test_v2_runner_persists_independent_same_snapshot_arms` | Required `target/debug/verification_spec_validate` is absent in this worktree. |
| `test_goal_verify_v3.py::WorkspaceBaselineBlindAndReadinessTest::test_live_input_verification_rejects_wrong_binary_commit` | Missing commandagent/validator binaries cause an earlier prerequisite rejection than the test's expected commit check. |
| `test_goal_verify_recovery_a15.py::GoalVerifyRecoveryA15InputsTest::test_new_workspace_hashes_match_generated_fixtures` | Frozen fixture inventory references absent, ignored `before/.pytest_cache/.gitignore`. |
| `test_score_retrospective.py::ScoreRetrospectiveTests::test_repository_scan_covers_all_run_level_band_rows` | Existing historical inventory has 293 rows; the test expects 287. |

`regression-findings.json` records the parent SHA and confirms all eight inspected
test/implementation/contract inputs match that parent. The retrospective scanner
reads `workspace/management/runs`, which this Issue did not modify. No frozen
fixture/hash, historical evidence or unrelated expectation was rewritten to pass
the wider run. The implementation can be reviewed and committed, but the requested
verification contract prevents an overall `passed` status with this failed check.

## GUI observations and limits

The isolated-profile smoke report and immutable Browser manifest are under
`smoke/isolated-profile/`; actual browser/automation versions were measured again
rather than copied from historical evidence. No model generation started.

The plugin smoke used the current skill's existing Browser binding, listed zero
tabs, created its own tab and successfully read/clicked/captured the page. The
supported page environment did not provide navigator metadata; the exact error
and zero identical retries are recorded in `smoke/plugin/metadata-limitation.json`.
The plugin operation report is intentionally `blocked` solely for
`conditions_unverified`. `record` returned exit 2 and a freeze attempt was refused,
which are the expected honest gate outcomes. `smoke/plugin/smoke-result.json`
separately records verified operations and refusal of incomplete campaign pins.

All owned fixture servers, temporary standalone browser/profile and task-owned
plugin tabs were closed. Existing browser connections and user tabs were retained.
Runtime ledgers/locks/start markers are unstaged; the retained artifacts are
completed observations, tickets, reports, images and immutable manifest evidence.
Saved smoke reports are historical observations, not permission to launch now;
fresh preflight is required for future evaluation. No Rust production code changed,
so Rust build/release checks are outside this change's verification scope.
