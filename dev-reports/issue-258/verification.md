# Issue #258 verification

- Status: `passed`

## Checks

- `cargo build --release --bin commandagent`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `wc -l gui/hooks/use-trial-{compose,monitor,terminal,run}.ts gui/components/trial-{compose,gate-one,gate-two,terminal,run}.tsx | awk '$2 == "total" { next } { print; if ($1 > 300) failed = 1 } END { exit failed }'`: `passed`
- `diff <(git show HEAD:gui/components/trial-run.tsx | rg -o 'data-testid="[^"]+' | sort) <(rg -I -o 'data-testid="[^"]+' gui/components/trial-{compose,gate-one,gate-two,terminal,run}.tsx | sort)`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-258-smoke`: `passed`
- `cd gui && npm run smoke:errors`: `passed`
- `cd gui && npm run smoke:session-index -- --output /tmp/commandagent-issue-258-session-index-smoke`: `passed`
- `cd gui && npm run smoke:storage -- --output /tmp/commandagent-issue-258-storage-smoke`: `passed`
- `git diff --check`: `passed`

The smoke suites used temporary evidence directories outside the repository;
no raw runtime logs or generated smoke artifacts are included in the change.
The full smoke exercised real Trial runs on `/` and `/proxy/commandagent`.

## PR #293 CI follow-up

CI run `32512103334` failed because `tests/doc_drift.rs` still checked the two
pre-split Trial owners. The requested ownership updates were applied without
changing the expected Gate 1 copy or canonical sample goal. A full local test
run additionally exposed related stale ownership assumptions in
`tests/gui_read_only_guard.rs`; those assertions now follow the extracted
files while preserving their behavioral checks.

- `cargo test --test doc_drift`: `passed` (20 passed)
- `cargo test --test gui_read_only_guard`: `passed` (24 passed)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

No Rust production source changed. The full Rust checks above were run for the
PR remediation and all completed successfully.
