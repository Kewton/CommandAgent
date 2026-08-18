# Issue 118 Verification

- Status: `passed`

## Checks

- `cd gui && npm run typecheck && npm run lint && npm run build`: `passed`
- `cargo test --features gui --test gui_read_only_guard`: `passed`
- `cargo test --features gui`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo build --release --features gui`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue118-current-feedback --feedback-only`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue118-current-polling --polling-only`: `passed`
- `origin/develop feedback/polling smoke baseline contract comparison`: `passed`
- `origin/develop versus worktree data-testid occurrence comparison`: `passed`
- `origin/develop versus worktree Japanese literal occurrence comparison`: `passed`
- `origin/develop versus worktree Trial apiPath call-shape comparison`: `passed`

## Baseline Comparison

- Feedback-only: both root and `/proxy/commandagent/` cases passed before and
  after the refactor. All stable output fields matched; only generated and
  freshness timestamps differed.
- Polling-only: both cases passed before and after the refactor. The baseline
  observed 62 requests per case; the refactor observed 61 and 62. All results
  remained inside the pinned 50–65 range, every request after the first carried
  the expected ETag, and reduction remained above 92%.
- Trial UI test IDs: 53 occurrences, exact match.
- Trial Japanese literals: 72 occurrences, exact match.
- Trial API paths: 11 calls, exact match after local identifier normalization.
