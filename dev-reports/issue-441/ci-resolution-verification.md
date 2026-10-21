# Issue #441 CI resolution verification

- Status: `passed`
- Parent: `8e3f071ae2a8ad42f6eb2865b7837b806deed634`
- Final tested source: `tests/issue441_placeholder.rs`
- Source SHA256: `f31a8b8e469fda24a8130eae19017a5b7e5c705fcf089f173c451ac77dbba4b2`

## Failure evidence and local reproduction

Read the immutable root logs at
`/Users/maenokota/share/work/github_kewton/CommandAgent-develop/workspace/management/runs/20260908-0151-issue441-pr/ci-failure-34145194835.log`
and the adjacent `acceptance-failure-34145194892.log`.
Both runs failed in
`evidence_hashes_original_bytes_even_when_cd_could_be_stripped` at
`tests/issue441_placeholder.rs:147`: `range end index 64 out of range for slice
of length 49`. Each Issue #441 suite had six passing tests and that one failure.

Before editing the test, ran
`TMPDIR=/tmp cargo test --test issue441_placeholder evidence_hashes_original_bytes_even_when_cd_could_be_stripped -- --exact --nocapture`.
It exited 101 with the same line and 49-byte slice panic. This was an expected
reproduction of the reported defect, not a production evidence failure.

## Checks

- `TMPDIR=/tmp cargo test --test issue441_placeholder -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --all-targets`: `passed`
- `cargo test --doc`: `passed`
- `git diff --check`: `passed`
- `git diff --exit-code 8e3f071a -- src Cargo.toml Cargo.lock tests/corpus dev-reports/issue-441/design.md dev-reports/issue-441/implementation-summary.md dev-reports/issue-441/verification.md dev-reports/issue-441/review-resolution-design.md dev-reports/issue-441/review-resolution-summary.md dev-reports/issue-441/review-resolution-verification.md`: `passed`

The short-`TMPDIR` focused run passed all eight tests with no ignores. Its
boundary cases use commands of 13, 64, 66, and 66 bytes whose lengths do not
depend on the host temporary path. Their expected prefix contents and lengths
are exactly `min(64, original byte length)`. The longer commands have identical
64-byte prefixes and different tails; the assertions verify the whole-command
SHA256, recorded original byte count, and distinct hashes. The actual original
workspace-cd command checks and UTF-8 split-byte case also passed unchanged
apart from the corrected bound and added byte-count assertion.

The all-target run passed 2,730 tests across 64 suites, with 38 existing ignored
tests and zero failures. It includes all seven corpus tests and all eight
Issue #441 tests using the normal host temporary directory. Both doctests
passed. Final checks used the source hash above; only report text was added
afterward. There were no post-fix test failures or additional test retries.

## Scope and handoff

Production code, evidence policy, corpus contracts, dependencies, guardrail
baselines, and all prior Issue #441 reports are unchanged. No ignores were
added. The earlier review-resolution report's readiness failure and rerun
history remains intact; this resolution's full suite passed on its first
post-fix attempt.

This worktree remained independent: no merge, rebase, base synchronization,
conflict resolution, push, or PR/Issue lifecycle operation was performed.
Exact-head CI/UAT and publication remain with the orchestrator; this report
records local verification only.
