# Issue 78 verification

- Status: `passed`

## Checks

- `cargo fmt --all -- --check`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue78-integrated-pass.ZkOo3W --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Browser/layout evidence

The managed Playwright smoke completed both `/` and
`/proxy/commandagent/` cases with `ok: true`. In both cases, each 390 × 844
measurement for compose, Gate 1, Gate 2, and Terminal reported:

- Japanese step labels `依頼`, `確認`, `実行`, `結果`;
- `one_state_visible: true`;
- `primary_in_initial_viewport: true`;
- no unexpected browser console errors.

Both cases also reported `next_session_reached_terminal: true`, preserved the
Gate 2 launch-identity lock, kept reconnect calls GET-only, and removed the
scratch runtime after success.

The root Gate 1, Gate 2, and Terminal desktop/mobile screenshots were visually
inspected. Each shows only its active workflow state; Gate 1 keeps the explicit
confirmation and exact hash, Gate 2 begins with progress, and Terminal keeps
verdict-left/D-3d-right placement on desktop and D-3d-before-verdict placement
on mobile.

The repository smoke used the managed interaction-probe Playwright 1.61.1
package and passed both supported base paths. The smoke-owned disposable GUI
servers exited normally; no user-managed server was stopped or restarted.

## Environment note

Initial non-escalated TypeScript and Cargo build attempts were rejected when the
sandbox tried to write `tsconfig.tsbuildinfo` and `target/*` inside the sibling
worktree. Re-running the identical checks with explicit sibling-worktree write
permission passed. These were sandbox permission failures, not compiler or test
failures.
