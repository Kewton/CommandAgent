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
