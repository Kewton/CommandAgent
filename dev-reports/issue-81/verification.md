# Issue 81 verification

- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev --offline`: `passed`
- `cd gui && node --check scripts/storage-smoke.mjs`: `passed`
- `cd gui && node --check scripts/error-smoke.mjs`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && npm run smoke:storage -- --output /private/tmp/commandagent-issue81-storage.Y4Vjit`: `passed`
- `cd gui && npm run smoke:errors`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence

- The focused browser report is
  `/private/tmp/commandagent-issue81-storage.Y4Vjit/trial-storage-smoke.json`.
  Both root and proxy cases reported `ok: true`; their keys were respectively
  `commandagent.gui.trial-token:/` and
  `commandagent.gui.trial-token:/proxy/commandagent`.
- The authentication/Origin smoke reported `ok: true` after exercising invalid
  token guidance, foreign-Origin rejection, Gate 1 confirmation, the live
  workspace lease conflict, GET-only reconnect, and fake CLI completion.
- The GUI source guard passed 19 tests. The GUI server integration target passed
  16 tests after running outside the filesystem/process sandbox so its
  disposable servers could bind loopback.
- The default library target reported 1,868 passed and 15 intentionally ignored
  tests; all default integration and documentation targets also passed.
- No corpus fixture, event schema, historical run evidence, `.anvil/` runtime
  namespace, or Rust server implementation changed.
