# Issue 71 Verification

- Status: `passed`

## Checks

- `git diff --check`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-71-post70-full-3 --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed` (root and proxy)
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-71-running-lease --feedback-only`: `passed` (root and proxy)
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `commandagent 0.1.0 c312eb75+dirty 2026-08-16T18:07:31+09:00`

## Coverage notes

The focused server suite proves that missing Trial credentials receive 401,
directory additions and removal are reflected without a restart, unrelated
directories and symlinked event files are not read, the response is capped at
100 rows, and starting, running, completed, failed, and unreadable projections
remain distinct. It verifies start/update epochs and Gate 2/3/4 projection,
including all of the existing acceptance-sheet full conditions before Gate 3.
It also observes a live `running` lease, puts its session first, and preserves a
failed `tui_command_stop` over a later completed `run_stop`.

The read-only guard pins the combined GET/POST route, mandatory Bearer/workspace
guard, bounded scan, read-only lease snapshot, query-only reconnect links,
non-idle launch block, and absence of deletion or lease-reset controls.

The final browser evidence is
`/private/tmp/commandagent-issue-71-post70-full-3/browser-smoke.json`. Both `/`
and `/proxy/commandagent/` record an authenticated `GET api/sessions`, visible
start/update/Gate/status values, a `?session=<id>` link that issues no POST,
GET-only reconnect, no token persistence, no unexpected console errors, and
`ok: true`. The successful run removed its disposable scratch runtime.

The focused running-lease browser evidence is
`/private/tmp/commandagent-issue-71-running-lease/browser-smoke.json`. Both base
paths show the exact `running` session ID in the lease card and session row,
render `GATE_2 / RUNNING`, disable the confirmed launch button with the owning
session reason, record `dispatch_count: 0`, and report `ok: true`.

The first full smoke attempt failed before dispatch because the worktree release
binary did not yet exist. After the release build, a second run exposed two
smoke-assertion defects: it read an already-visible row before refresh completed
and compared CSS-uppercase `GATE_4` to lowercase `gate_4`. Product API responses
were successful in both cases. The smoke now waits for the exact GET response
and normalizes the displayed comparison; the recorded third run passed both
base paths. All loopback/browser commands that needed build, bind, or local
model access ran with explicit sandbox escalation. No user-managed server or
CommandMate process was stopped or restarted.
