# Issue 393 verification

- Status: `passed`

## Checks

- `python3 -m pytest tests/eval/test_goal_verify_baseline.py -q`: `passed`
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 scripts/eval_lib/goal_verify_baseline.py scripts/eval-goal-verify-baseline.py tests/eval/test_goal_verify_baseline.py`: `passed`
- `python3 scripts/eval-goal-verify-baseline.py --run-dir dev-reports/issue-393/runs/fixture-replay-e74b7113`: `passed`
- `python3 scripts/eval-goal-verify-baseline.py --run-dir /tmp/issue393-final-replay.PKv18g`: `passed`
- `cmp dev-reports/issue-393/runs/fixture-replay-e74b7113/baseline.json /tmp/issue393-final-replay.PKv18g/baseline.json`: `passed`
- `cmp dev-reports/issue-393/runs/fixture-replay-e74b7113/manifest.json /tmp/issue393-final-replay.PKv18g/manifest.json`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Scope audit

- Existing `workspace/management/runs/` and `docs/migration/` records: unchanged
- Production Rust behavior: unchanged
- Event authority/schema and assurance verdict: unchanged
- `.anvil/` namespace: unchanged
- Live provider execution: not run; outside the authorized Phase 0 scope
