# Issue #67 Verification

- Status: `passed`

## Checks

- `git diff --check`: `passed`
- `cargo test --features gui --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-67-smoke.7uAUke/evidence --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`

## Notes

- The focused GUI-server test was rerun with localhost permission after the
  sandbox correctly rejected ephemeral port binding. The first dashboard-only
  assertion also required `gui/out`; after the passing production GUI build,
  all eight GUI-server integration tests passed.
- This environment omits npm development dependencies by default.
  `npm ci --include=dev` restored the lockfile-pinned TypeScript and React type
  packages without changing the lockfile, after which every recorded GUI check
  passed.
- The live smoke report recorded overall `ok: true` for `/` and
  `/proxy/commandagent/`. Both cases loaded the server-derived `python-cli`
  option, explicitly filled Goal and both model roles, reached Gate 1 and an
  honest terminal Gate 4, and reported no unexpected console errors. The
  isolated runtime was removed automatically after success; output evidence is
  in the temporary path above and no raw runtime evidence is committed.
- An additional in-app Browser inspection was unavailable because this session
  had no connected in-app or extension browser. It was not substituted with a
  different interactive browser surface; the repository's required managed
  Playwright smoke passed independently as recorded above.
