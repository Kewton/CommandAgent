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
Rust source was not changed, so Rust formatting, clippy, and test suites were
not required beyond building the release delegate used by the live smoke.
