# Issue #425 Reopen Verification

- Status: `blocked`

Fresh verification is pending for the 2026-09-05 reopen scope (comment
5550459250). The results below are historical and do not verify this run.

## Historical verification from PR #427 / 6ccc3913

- Historical status: `passed`

### Historical checks

- `cargo fmt --all -- --check`: `passed`
- `cargo test planner::recovery_contract_authority::tests --lib`: `passed`
- `cargo test planner::auto_recovery::tests --lib`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test --test generality_guardrails nextjs_boundary_erosion_tripwire_keeps_dispatch_sites_audited`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --quiet`: `passed`
- `git diff --check`: `passed`

### Historical environment note

The final full test suite ran outside the filesystem/process sandbox because
existing provider and browser tests require loopback sockets and child-process
inspection. It completed successfully without changing the test selection.
