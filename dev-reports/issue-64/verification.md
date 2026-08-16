# Issue 64 verification

- Status: `passed`

## Checks

- `git diff --check`: `passed`
- `cargo test --features gui --test gui_server spawn_failure_reports_the_binary_and_releases_the_workspace -- --nocapture`: `passed`
- `cargo test --features gui --test gui_server recovery_required_lease_is_exposed_by_an_authenticated_get -- --nocapture`: `passed`
- `cargo test --test gui_read_only_guard trial_workspace_recovery_is_visible_but_read_only -- --nocapture`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --features gui --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The first sandboxed focused GUI-server attempt could not bind `127.0.0.1:0`.
The required focused and complete GUI-server targets were rerun with loopback
permission and passed. The first TypeScript attempt found that `node_modules`
was absent; `npm ci --include=dev` restored the lockfile-pinned dependency graph,
after which typecheck, lint, and both base-path builds passed. No lockfile change
was produced.

The spawn regression asserts that HTTP 500 contains both the missing binary
path and OS cause, the lease reads `idle`, a restarted server on the same
execution root still reads `idle`, and installing the valid fixture binary at
that path makes the next confirmed Trial return HTTP 202. The recovery snapshot
test asserts an unauthenticated GET is rejected and an authenticated GET returns
`recovery_required` with the exact unfinished session ID.
