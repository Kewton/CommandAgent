# Issue #412 Verification

- Status: `passed`

## Checks

- `cargo test --features gui --test gui_server unclassified_nextjs_create_is_unmeasured_confirmed_and_delegated -- --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test --features gui`: `passed`
- `npm run typecheck` (in `gui/`): `passed`
- `npm run lint` (in `gui/`): `passed`
- `npm run build` (in `gui/`): `passed`
- `node --check scripts/smoke.mjs` (in `gui/`): `passed`
- `npm run smoke -- --gate-one-only --commandagent-bin ../target/debug/commandagent --output /tmp/commandagent-issue-412-browser-smoke` (in `gui/`): `passed`

## Notes

The browser smoke report recorded `ok: true` for both `/` and
`/proxy/commandagent/`. In each case the ambiguity probe observed HTTP 422,
the complete Japanese guidance and candidate detail in `role="alert"`, a
keyboard-focusable intent selector, and HTTP 200 with `unknown` / `未計測`
after retrying with `作成`.

The first broad Rust run had one timeout in the unrelated
`gui_lists_and_proposes_an_external_draft_profile_with_a_local_pack` fake
delegate test. That test passed immediately in isolation, and the identical
full `cargo test --features gui` command then passed all targets on rerun. The
first browser-smoke attempt was sandbox-blocked from binding a loopback port;
the same command passed outside the network sandbox.
