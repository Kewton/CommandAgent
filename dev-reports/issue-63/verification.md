# Issue 63 verification

- Status: `passed`

## Checks

- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `node --check gui/scripts/smoke.mjs`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --output ../dev-reports/issue-63/smoke-evidence --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Browser smoke result

Both `/` and `/proxy/commandagent/` returned 200 for desktop and mobile probes,
recovered from their injected first polling failure to Gate 4, displayed a
successful-update timestamp, and passed reload reconnect using only 401/200 GET
calls. The proxy case displayed the Access re-authentication guidance. Both
cases displayed 409 reconnect guidance without dispatching the intercepted
second launch, found no token in the URL or `localStorage`, and reported no
unexpected console errors.

The first sandboxed full-suite attempt encountered `Operation not permitted`
for tests that bind local ports. The required `cargo test` check above is the
subsequent unrestricted rerun, which completed with all non-ignored tests green.
Generated screenshots, JSONL streams, and API logs were summarized here and
removed rather than committed as raw runtime evidence.
