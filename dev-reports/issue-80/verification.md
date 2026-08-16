# Issue #80 Verification

- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-80-smoke --polling-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`

## Smoke observations

- Root deployment: 58 observed calls over 600,000 virtual milliseconds,
  92.759% fewer than the 801-call fixed-750 ms baseline.
- `/proxy/commandagent/`: 57 observed calls over 600,000 virtual milliseconds,
  92.884% fewer than baseline.
- In both cases the first response was 200 and every later observed call sent
  `If-None-Match: W/"synthetic-unchanged"` and received 304.

## Setup notes

- `npm ci --offline` initially inherited `NODE_ENV=production` and omitted the
  lockfile's development dependencies. `npm ci --include=dev --offline`
  restored the declared TypeScript and React/Node type packages; the final
  lint, typecheck, build, and smoke commands above all passed.
- The first sandboxed smoke launch could not bind loopback (`Operation not
  permitted`). The same focused command passed with the required loopback and
  headless-browser permission.

## Post-predecessor integration verification (2026-08-16)

The current `develop` history was merged while preserving failure recovery,
GET-only reconnect, workspace lease visibility, lifecycle locking, Japanese
UI, server-derived options, and corrected phase projection. Success and failure
polling policies now share `gui/lib/trial-monitor.ts`; the obsolete standalone
`trial-polling.ts` leaf was removed. Static immutable caching is restricted to
paths whose first two components are `_next/static`, with a negative nested
path assertion.

- `node --check gui/scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed` (23 tests)
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-80-post68-polling --polling-only --commandagent-bin ../target/debug/commandagent`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-80-post68-full --commandagent-bin ../target/debug/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `git diff --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`

The final full report is
`/private/tmp/commandagent-issue-80-post68-full/browser-smoke.json`. Root and
proxy cases both record overall `ok: true`, 60 calls over ten virtual minutes,
conditional ETags on every request after the initial 200, and a 92.51% request
reduction. Both also recover from the injected Issue #63 failure, reconnect
using GET only, clear CLOSED state, and reach a distinct second terminal
session. The scratch runtime was removed after success. No CommandMate process
was stopped or restarted.
