# Issue #466 verification

- Status: `passed`

## Checks

- `cargo test --lib issue466 -- --nocapture`: `passed`
- `cargo test --lib recovery_ -- --nocapture`: `passed`
- `cargo test --test protection_coverage_audit`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release`: `passed`
- `target/release/commandagent --version`: `passed`
- `shasum -a 256 target/release/commandagent`: `passed`
- `git merge-base --is-ancestor e99f1ebe1e777fb792a9343407736ca754e41bdf HEAD`: `passed`
- `git merge-base --is-ancestor 6c49222db4f282b75070d923479e6d41209b132a HEAD`: `passed`
- `git diff --check`: `passed`

## Results

Final focused Issue #466 run: 12 passed, none failed or ignored. The tests
exercise a 20-case closed existence/owner matrix, exact historical E1/E2/E3
producer-array refusals, fresh preclosure formation, actual host augmentation,
owner-specific requirements, compound command registration, non-Implement/Verify
registration negatives, and real Runner generation/execution with replayed
model replies. Wrong ownership receives feedback before a corrected plan writes
the verifier and runs its original checks. Exhausted, unsafe and altered plans
do not execute. Formation rejection followed by schema/lint failures cannot
escape through setup fallback; ordinary setup fallback remains permitted.

Recovery regression run: 198 passed, 2 existing ignored. Final full `cargo test`
completed with exit 0, including 2,529 library tests passed and 19 existing
ignored, all integration test binaries, and both compile-fail doctests. Within
that full run, corpus regression passed 7 tests, generality guardrails passed
10, and protection coverage audit passed 2. Clippy completed without warnings.
No guardrail baseline, audit exemption, existing verification expression, or
retry budget was relaxed. No ignored test was added by this Issue.

The first broad recovery attempt encountered sandbox restrictions on local
listeners; the complete recovery and full suites were subsequently run outside
the sandbox. Intermediate full runs exposed lint-retry compatibility failures,
the older #456 missing-contract expectation, and the new parser's direct
validation bypass of the shared normalizer. The implementation now leaves
invalid command syntax to the existing lint/retry path, explicitly rejects
missing sealed contracts, and identifies scripts only after the shared typed
`NormalizedVerifyCommand` boundary. Focused checks and the final complete suite
passed after these corrections. Earlier failed runs were not accepted as green.

## Base and release artifact

The initial authorized `git fetch --no-tags origin develop` resolved to
`e99f1ebe1e777fb792a9343407736ca754e41bdf`, also the initial working HEAD.
Inspected and incorporated verified #465 commits
`b60ae3e63a0eb23cdf40cf52057b995bde08f8ca` and
`6c49222db4f282b75070d923479e6d41209b132a` by conflict-free fast-forward.
Final verification HEAD before the #466 commit was the latter SHA; both required
ancestor checks passed. The parent's report that GitHub CI was rerunning is not
treated as completed remote acceptance. This report records local verification.

Release build completed after the final full suite, using this worktree's
uncommitted #466 source on that verified predecessor. Embedded version output:

```text
commandagent 0.1.0 6c49222d+dirty 2026-09-12T13:12:37+09:00
```

SHA-256 of `target/release/commandagent`:

```text
e4cf2c4faa4b13182030d7721b1ae62c6c399693779008d80333be3200e45d86
```

The `+dirty` marker accurately describes the required precommit release build;
the version timestamp is the embedded build metadata. No live-model campaign
or historical E campaign success is claimed. Historical missing proposals and
model/host contribution remain unknown, as recorded in the design and corpus.
Raw test logs and binary artifacts remain outside the commit.
