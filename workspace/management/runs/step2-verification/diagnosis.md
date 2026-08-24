# Step 2 acceptance verification

## Push and CI

The four commits were pushed to `origin/develop`:

`8bda05a`, `4c23f1e`, `cb049dd`, `646dced`.

GitHub Actions run `29810042588` (`acceptance`) failed in the `clippy` step.
`fmt` passed. `cargo test`, Python checks, and changed-file scrub were not
run because the job stops at clippy. The clippy diagnostics are pre-existing
Rust warnings (including `manual checked division`, several `while let`
suggestions, and `sort_by_key`); no Rust files were changed here. The workflow's
explicit exclusion declaration is `browser_probe=0`, count `0`; it was not
reached in this run, so no exclusion output was emitted by the runner.

## Full-suite diagnostic

`cargo test` was started in the authorized local environment and did not reach
a terminal result before the diagnostic run was stopped. The observed output
included environment-sensitive failures in:

- `doctor::tests::ollama_tags_check_covers_reachability_and_each_role_model`
- `planner::runner::tests::assurance_tests::moved::fake_interaction_probe_failure_fails_gate_without_repair_target`
- `planner::runner::tests::dev_server_writes_readiness_before_forced_cleanup_failure`
- `planner::runner::tests::dev_server_cleanup_kills_grandchild_process_group_without_pipe_deadlock`
- `planner::runner::tests::compile_error_repair_prompt_anchors_file_and_then_runs_readiness`

Because the run was interrupted before the final harness summary, complete
failure output and the requested four-commit `--exact` matrix are not claimed.
No tests or source were modified. Classification is therefore deferred to
review; no A/B/C verdict is asserted.

## Tracked derived artifacts

Using `git ls-files` and the requested patterns (`node_modules`, `.next`,
`target`), only one tracked derived file remains:

| campaign | pattern | count | bytes |
|---|---|---:|---:|
| outside `runs/` (`eval/fixtures/acceptance/nextjs_missing_root_page`) | `.next` | 1 | 100 |
| **total** |  | **1** | **100** |

No tracked `node_modules/` or `target/` paths were found. A grep over tracked
report-like files found references to these patterns in 26 reports, including
`workspace/management/runs/uat-test0715-data-005/uat-report.md`,
`workspace/management/runs/uat-test0715-ff1-001/uat-report.md`, and the prior
cleanup report. These are textual references, not evidence that the derived
trees are tracked.

## v2: clippy cleanup and acceptance rerun

The first CI run reported seven warnings: `while_let_loop` (five occurrences),
`sort_by_key` (one), and `manual_checked_div` (one). Equivalent rewrites were
applied in the UX-wave implementation paths; the CI toolchain then exposed one
additional `while_let_loop` in `tests/tui_pty.rs`, which was equivalently
rewritten. No crate-wide lint suppression or adjudication code was changed.
Local `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are
green. Local full `cargo test` completed green: 1,576 passed, 15 ignored.

The final CI run after the remaining test-target rewrite is run
`29810891791`; its observed state at report generation was `cargo test` still
in progress after `fmt` and `clippy` passed. The workflow's explicit exclusion
declaration remains `browser_probe=0`, count `0`; the final log must be checked
after completion to confirm downstream Python and scrub steps.

The earlier “1,300 derived” figure counted `.anvil` paths as well as the three
requested patterns. The requested `git ls-files` pattern count is instead:
`node_modules=0`, `.next=1` (100 bytes, outside `runs/`), `target=0`; total 1.
