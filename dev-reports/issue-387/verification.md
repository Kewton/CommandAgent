# Issue 387 Verification

- Status: `passed`

## Checks

- `cargo test --lib planner::auto_recovery::tests`: `passed`
- `cargo test --lib both_top_level_ultra_plan_actions_route_through_shared_auto_recovery`: `passed`
- `cargo test --lib ultra_plan_execution_slashes_share_the_configured_recovery_limit`: `passed`
- `cargo test --lib confirmed_gate_one_identity_preserves_recovery_limit_for_dispatch`: `passed`
- `cargo test --lib manual_recovery_after_auto_success_remains_visible`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo test --features gui --test gui_server recovery_auto_run_limit_is_hash_bound_validated_and_delegated`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo build --release --features gui`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke:session-index -- --output ../target/issue387-session-preflight-final`: `passed`
- `cd gui && npm run smoke -- --overview-only --output ../target/issue387-browser-preflight-final --commandagent-bin ../target/release/commandagent`: `passed`

## Post-merge CI follow-up checks

- `cargo fmt --all -- --check`: `passed`
- `cargo test --features gui --test gui_server recovery_auto_run_limit_is_hash_bound_validated_and_delegated`: `passed` (11 consecutive runs)
- `cargo test --features gui --test gui_server`: `passed`
- `cargo clippy --features gui --bin gui_server -- -D warnings`: `passed`

Regression note: develop CI run `32818809861` observed
`delegated-args.txt` after shell redirection created the path but before the
single `printf` made the complete argument list observable. The test now uses
the existing bounded deadline to wait for the exact adjacent confirmed
flag/value pair before retaining its original exact assertions. The correction
is test-only; no production or acceptance contract was changed.

The browser report recorded overall `ok: true` for `/` and
`/proxy/commandagent/`. In both cases the Trial compose regression recorded the
explicit Recovery Plan value `3`, an edited proposal value `4`, confirmation
reset after editing, total execution bounds `4` and `5`, matching duration/cost
multipliers, and overall `ok: true`. The session smoke also recorded overall
`ok: true` for both base paths and authenticated GET-only Recovery document
access.

The release binary reported:

```text
commandagent 0.1.0 3ce69666+dirty 2026-08-25T15:00:34+09:00
```
