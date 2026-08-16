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
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-78-smoke.umwo5T --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
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

The root Gate 1, Gate 2, and Terminal desktop screenshots were visually
inspected. Each shows only its active workflow state; Gate 1 keeps the explicit
confirmation and exact hash, Gate 2 begins with progress, and Terminal keeps
verdict-left/D-3d-right placement.

The in-app browser runtime exposed no available browser instance, so no separate
interactive browser session was possible. This did not block the required
repository Playwright smoke, which used the managed interaction-probe Playwright
1.61.1 package and passed both supported base paths.
