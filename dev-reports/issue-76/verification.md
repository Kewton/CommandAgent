# Issue 76 verification

- Status: `passed`

## Checks

- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `git grep -n -E 'Launch once\. Trust the gates\.|Claims need coordinates\.|Pinned means visible\.' -- gui tests docs/dev/mechanism-ledger.md; test $? -eq 1`: `passed`
- `cargo test --features gui --test gui_server --test gui_read_only_guard`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /tmp/commandagent-issue-76-smoke.Ji2YX6 --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --quiet`: `passed`
- `git diff --check`: `passed`

## Evidence

- Smoke report: `/tmp/commandagent-issue-76-smoke.Ji2YX6/browser-smoke.json`
- Both `/` and `/proxy/commandagent/` cases reported `ok: true`.
- Every page returned its Japanese heading and distinct `… | CommandAgent`
  title. Both mobile probes hid the intro description, fit the 390px viewport,
  and retained the Issue 77 stage-scroll clearance.
- The header reported Trial available and no active execution before launch,
  the actual short session ID while running, and no active execution again
  after terminal projection. Issue 63 degraded/recovered monitoring, GET-only
  reconnect, token-memory, and 409 recovery checks remained green.
- Dashboard, mobile Gate 1, and desktop Gate 2 screenshots were visually
  inspected; the compact intro, four-item navigation, and runtime badges had no
  observed overlap or overflow.

The first sandboxed smoke attempt could not bind `127.0.0.1:0`. The required
command above is the subsequent approved localhost-capable run and passed. A
focused GUI server attempt run concurrently with Next build also exited during
startup; the same focused command was rerun without that build race and passed
all 16 tests.
