# Issue 100 verification

- Status: `passed`

## Checks

- `cd gui && npm ci --include=dev --offline`: `passed`
- `cd gui && node --check scripts/session-index-smoke.mjs`: `passed`
- `cd gui && node --check scripts/storage-smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue100-session-index-final`: `passed`
- `cd gui && npm run smoke:storage -- --output /private/tmp/commandagent-issue100-storage`: `passed`
- `cd gui && npm run smoke:errors`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue100-read-only --read-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Evidence

- `/private/tmp/commandagent-issue100-session-index-final/session-index-smoke.json`
  reports `ok: true` for `/` and `/proxy/commandagent/`. In each case the
  runtime-status projection refreshed five times while the initial idle-to-
  running window kept the session index at one request, proving there is no
  independent short-interval index poll. Both cases then passed launch
  insertion (`GATE_2 / STARTING`), terminal update (`GATE_3 / COMPLETED`),
  stale-row retention, focus/visibility/runtime refresh, Terminal row linking,
  and GET-only reconnect.
- The same focused report marks repository-only, Trial-only, both-present, and
  Trial-unauthenticated source/display scenarios `state_ok: true` for both base
  paths. The unauthenticated scenario issued zero session-index requests.
- `/private/tmp/commandagent-issue100-storage/trial-storage-smoke.json` reports
  `ok: true` for tab restoration, independent-tab isolation, base-path storage
  keys, definitive rejection removal, and token non-disclosure.
- `/private/tmp/commandagent-issue100-read-only/browser-smoke.json` reports
  `ok: true` for both base paths, including the renamed navigation/title,
  repository run selection, source links, and internal base-path links.
- The GUI source guard passed 19 tests and pins the absence of an independent
  session-index interval and a second runtime-status hook. The GUI server
  integration target passed 16 tests without API/schema changes.
