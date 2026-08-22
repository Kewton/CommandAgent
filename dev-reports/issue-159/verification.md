# Issues 159, 163, and 170 verification

- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev`: `passed`
- `cd gui && node --check scripts/error-smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run smoke:errors`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --test gui_read_only_guard gui_fetch_failures_use_one_actionable_error_descriptor -- --nocapture`: `passed`
- `cargo test --test gui_read_only_guard trial_monitor_retries_and_reconnects_with_tab_scoped_access -- --nocapture`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

- The first TypeScript attempt ran before this worktree had dependencies and
  reported only missing Next.js, React, and Node modules. The lockfile-pinned
  install completed, and the required TypeScript check then passed.
- The first browser-smoke launch inside the sandbox was denied permission to
  bind `127.0.0.1:0`. The exact check was rerun outside the sandbox, as required
  for the temporary loopback server.
- Two intermediate smoke iterations exposed assumptions in the new test flow:
  the existing fake CLI does not reach a terminal gate, and a recovery lease
  blocks a live proposal. The final smoke isolates those scenarios with mocked
  read-only responses and passed with one manual missing-session GET, four
  failed polling attempts for each terminal class, and GET-only recovery
  traffic.
