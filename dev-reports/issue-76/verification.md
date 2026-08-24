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

## Post-Issue #63/#64/#66/#77 integration verification (2026-08-16)

Current `develop` was merged while preserving monitoring/reconnect behavior,
the read-only launch identity and CLOSED lifecycle, responsive accessibility,
and workspace lease recovery. The integrated Japanese Trial copy retains the
registered `--pattern` family token so a reloaded draft still resolves the
same deterministic `python-cli × create × filter` route.

- `node --check gui/scripts/smoke.mjs`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo test --test gui_read_only_guard`: `passed` (10 tests)
- `cargo test --features gui --test gui_server`: `passed` (9 tests)
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue-76-post64-smoke-final --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- obsolete-copy grep and `git diff --check`: `passed`

The final smoke report is
`/private/tmp/commandagent-issue-76-post64-smoke-final/browser-smoke.json` and
records overall `ok: true` for `/` and `/proxy/commandagent/`. Both cases kept
the Trial controls aligned at 1440px and 390px, cleared the sticky header at
Gate 2 and terminal, recovered from the injected monitoring failure, used only
GET requests for reconnect, locked all six launch-identity controls through
Gate 2/terminal/CLOSED, returned to the Japanese `下書き` state, and reached a
terminal projection with a distinct second session.

An integration smoke attempt exposed that the translated initial goal had
dropped the route catalog's literal `--pattern` token. That honest 422
ambiguity (`stats` versus `filter`) was fixed by restoring the registered token
in the Japanese goal and pinning it in `gui_read_only_guard`; the final clean
smoke above passed. The smoke harness now also preserves a failure screenshot
and visible page diagnostics when a future browser step fails.
