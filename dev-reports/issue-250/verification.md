# Issue #250 verification

- Status: `passed`
- `cargo test planner::declarative_command_checks::tests --lib`: `passed`
- `cargo test draft_local_pack_command_check_records_result_and_summary --lib`: `passed`
- `cargo test profile_discovery_preserves_formal_then_unbanded_order --lib`: `passed`
- `cargo test registry_contract_snapshot_is_stable --lib`: `passed`
- `cargo test planner::pack:: --lib`: `passed`
- `cargo test --test issue250_declarative_command_checks`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
