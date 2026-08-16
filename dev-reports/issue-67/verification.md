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

## Post-Issue #63/#64/#66/#76/#77 integration verification (2026-08-16)

The current `develop` history was merged while retaining the server-derived
Trial options, empty Goal/model defaults, and local preflight validation. The
integrated UI keeps all six launch identity controls locked through Gate 2,
terminal, and CLOSED, presents the server-provided descriptions and model hints
in Japanese, and explicitly fills the empty fields for every smoke dispatch.

- `git diff --check`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --features gui --test gui_read_only_guard --test gui_server`: `passed` (20 tests)
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-67-post76-smoke --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed` after applying the reported mechanical test-line wrap
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`

The final smoke report is
`/private/tmp/commandagent-issue-67-post76-smoke/browser-smoke.json` and records
overall `ok: true` for `/` and `/proxy/commandagent/`. Each case loaded the
server-derived `python-cli` option, explicitly supplied Goal and both model IDs
for the initial, conflict, and post-CLOSED runs, preserved GET-only reconnect
and runtime monitoring, kept identity controls locked while active, returned to
the Japanese `下書き` state, and reached terminal projections for distinct
second sessions. The isolated runtime was removed after success.
