# Issue 395 verification

- Status: `passed`

## Checks

- `cargo test verification_spec::create_shadow::tests --lib`: `passed`
- `cargo test --test create_shadow_oracle`: `passed`
- `cargo test --test verification_spec_v0`: `passed`
- `cargo test planner::declarative_command_checks::tests --lib`: `passed`
- `cargo test planner::profile_admission::tests --lib`: `passed`
- `cargo test planner::runner::tests::assurance_tests --lib`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `python3 -m pytest tests/eval/test_verification_spec_v0_schema.py tests/eval/test_goal_verify_baseline.py -q`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Compatibility audit

- Phase 0 goal-to-verify baseline and Phase 1 VerificationSpec schema/goldens:
  unchanged and passed.
- Existing declarative verify policy: unchanged and passed, including install,
  dev-server, shell interpreter, mutation, and workspace-escape negatives.
- Existing profile admission cap: unchanged and passed.
- Existing browser release-gate assurance suite: unchanged; 22 passed and one
  pre-existing ignored test. The successful run used loopback permission
  required by its local-port fixtures.
- Full Rust suite: 2,173 library tests passed, 16 ignored, followed by all
  integration and doc tests passing.
- Existing events, CompletionContract, adjudication, terminal projection, and
  `.anvil/` state: no schema or production-path wiring changes.
