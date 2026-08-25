# Issue 394 verification

- Status: `passed`

## Checks

- `cargo test --test verification_spec_v0`: `passed`
- `python3 -m pytest tests/eval/test_verification_spec_v0_schema.py tests/eval/test_completion_contract_snapshots.py -q`: `passed`
- `ruff check --isolated --select E4,E7,E9,F,I --ignore E402 tests/eval/test_verification_spec_v0_schema.py`: `passed`
- `cargo test --test adjudication_compat`: `passed`
- `python3 -m pytest tests/eval/test_goal_verify_baseline.py -q`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Compatibility audit

- Existing `ultra_final_acceptance` 81-key byte fixtures: unchanged and passed.
- Existing CompletionContract snapshots: unchanged and passed.
- Existing Phase 0 goal-to-verify baseline replay: unchanged and passed.
- Existing event names and `.anvil/` runtime namespace: no production changes.
- Issue-owned verification artifacts were saved under
  `/Users/maenokota/share/work/localwork/commandagent_trial/issue/394`.
