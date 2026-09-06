# Issue #428 verification

- Status: `passed`

## Checks

- `cargo test --lib tools::bash::path_tokens::tests`: `passed`
- `cargo test --lib tools::bash`: `passed`
- `cargo test --test issue428_bash_path_tokens --test bash_workspace_confinement`: `passed`
- `cargo test --lib issue428`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `rustfmt --check --edition 2024 src/planner/auto_recovery/issue428_tests.rs`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Results and execution context

- Final focused runs passed 27 Bash/guard unit tests, 7 confinement integration tests (5 Issue #428 plus 2 existing), and the Issue #428 Recovery test covering 3 independent failed-promotion cases. The 6 corpus-driver tests passed without changing `tests/corpus_regression.rs`.
- The full `cargo test` run on final source completed outside the sandbox with exit 0, including integration tests and doctests. Its initial sandbox attempt showed failures among existing local HTTP/provider tests and was interrupted; the complete outside-sandbox rerun supersedes that attempt. No test was skipped or weakened to obtain the passing result. Live provider/probe opt-in variables were unset.
- An initial new Recovery assertion expected a generic rejection prefix; inspection showed the production gate correctly preserves the failed verification command in its reason. The test now checks the actual command-specific reason. The corrected focused test and complete suite passed.
- Eight fixture reads execute successfully in a temporary treatment workspace: expense detail, approve, reject, quote concatenation, escaped brackets, input redirection, and `/dev/null` before an adjacent command separator. The tool-registry run emits no confinement-rejection events for those reads.
- Negative coverage preserves outside absolute references, embedded interpreter/substitution references, exact `/dev/null` semantics, read/write traversal, existing symlinks, an unquoted bracket glob that can select an outside symlink, and unapproved writes. Reconstructed workspace paths use the existing canonicalization proof before approval.
- The real Recovery finish/promotion gate rejects synthetic build failure, a failing registered expense-approval predicate, and missing business-interaction evidence after successful route reads. Original control source remains byte-identical and no treatment-promoted event appears.
- The source-only fixture models E1 commands and acceptance failures; it is not a live-provider replay, real Next.js build, or browser acceptance run. No live runtime state or historical/external evidence was modified.

## CI follow-up: interaction-probe harness timeout (2026-09-06)

- Scope: this section covers only the shared-test timeout fix ported from
  Issue #430 (PR #434 commit a5372155). The Issue #428 Bash confinement
  implementation, its fixture, and its tests are unchanged.
- This branch's first CI runs on `da4c6a81` split: the `CI` run
  (34006422653) passed, while the `acceptance` run (34006422596) failed one
  pre-existing library test,
  `minimal_loop::interaction_probe::tests::draft_only_storage_does_not_satisfy_committed_persistence`,
  with `failure_kind: "probe_infrastructure_failed:probe_timeout"`,
  `duration_ms: 12045`, and last stage `persistence_reload`. Neither the
  Issue #428 change set nor this branch touched `interaction_probe.rs` or
  `assets/interaction_probe.js`; the same failure appeared on PR #434's CI
  at the same time.
- Root cause (from the Issue #430 analysis): the fake-Playwright harness
  `run_fake_probe_scenario` passed a hard-coded 12s budget, while the
  negative `draft_only_persistence` scenario legitimately exhausts three 3s
  polling windows plus the input-marker and recovery-transition waits,
  measuring ~12s on an idle machine. The harness deadline is polled every
  50ms after child spawn, so CI scheduling jitter under the parallel test
  load crossed it.
- Fix: `run_fake_probe_scenario` in `src/minimal_loop/interaction_probe.rs`
  now uses the documented test constant `FAKE_PROBE_SCENARIO_BUDGET` of 60s,
  matching the smallest production budget (`browser_probe.rs`). This is the
  same hunk as PR #434 commit a5372155. No persistence, token-echo, or
  failure-kind assertion changed, no test was ignored or retried, and no
  probe-script timing constant changed.
- Rechecks on this branch after the fix:
  `cargo test --lib draft_only_storage_does_not_satisfy_committed_persistence`
  passed (finished in 11.93s);
  `cargo test --lib minimal_loop::interaction_probe::tests` passed all 36
  tests; `cargo fmt --all -- --check` passed;
  `cargo clippy --all-targets -- -D warnings` (with `RUSTFLAGS=-D warnings`)
  passed; `git diff --check` reported no whitespace errors. The Issue #428
  focused suites above were not rerun because no Issue #428 file changed.
