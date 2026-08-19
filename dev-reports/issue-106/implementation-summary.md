# Issue 106 implementation summary

## Implemented

- Split the former monolithic GUI Trial session module into focused owners:
  `gate_one.rs` for proposals and validation, `delegate.rs` for confirmed CLI
  execution, `directives.rs` for D-3d handlers, and `session_paths.rs` for run
  paths. `sessions.rs` now owns polling/event projection and shared access/error
  policy.
- Centralized initial and continuation child construction in `delegate.rs`.
  Delegated commands call `env_clear()`, restore only the explicit basic
  process/locale and provider-credential allowlist, then set
  `COMMANDAGENT_EVAL_EVENTS` explicitly. Ambient GUI secrets, unrelated values,
  and all `COMMANDAGENT_PACK_*` selectors are excluded.
- Rewired the existing routes without changing request/response schemas, HTTP
  statuses, stable error codes, CLI arguments, or session paths.
- Extended the real GUI delegation integration test to record the fake CLI's
  environment and prove allowlisted credential retention plus pack-selector,
  GUI-token, and unrelated-secret exclusion.
- Moved the sole-process-surface and protection-audit allowlists to
  `delegate.rs`, added negative examples for the old process location, missing
  environment clearing, and explicit pack injection, and updated structural
  checks for the new handler ownership.
- Documented the delegated environment allowlist at the GUI's sole process
  surface.

## Compatibility

The full existing `tests/gui_server.rs` suite passes unchanged at the API level.
No event schema, runtime namespace, historical evidence, or guardrail baseline
was changed.
