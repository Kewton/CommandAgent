# Issue 71 Verification

- Status: `passed`

## Checks

- `git diff --check`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test --quiet`: `passed`

## Coverage notes

The focused server suite proves that missing Trial credentials receive 401,
directory additions and removal are reflected without a restart, unrelated
directories and symlinked event files are not read, the response is capped at
100 rows, and starting, running, completed, failed, and unreadable projections
remain distinct. It also observes a live `running` lease with the delegated
session ID. The failure fixture records a failed `tui_command_stop` followed by
a completed `run_stop` and confirms the list remains failed.

The read-only guard pins the combined GET/POST route, mandatory Bearer/workspace
guard, bounded scan, read-only lease snapshot, query-only reconnect links,
non-idle launch block, and absence of deletion or lease-reset controls.

The initial sandboxed focused server attempt could not bind `127.0.0.1:0`; the
recorded focused and full passing commands used loopback permission. The first
GUI typecheck found that this worktree had no `node_modules` and therefore no
React/Next type packages. `npm ci --include=dev --offline` restored the
lockfile-pinned dependency graph without modifying the lockfile, after which
all recorded GUI checks passed.
