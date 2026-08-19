# Issue 106 design

## Goal

Keep the GUI Trial HTTP contract unchanged while isolating its only child-process
surface and preventing ambient pack selection from reaching delegated
`commandagent` processes.

## Design

- Keep session polling, event projection, shared Trial authorization, and shared
  error mapping in `sessions.rs`.
- Move Gate 1 proposal/validation/price logic to `gate_one.rs`, initial confirmed
  dispatch and all `std::process::Command` use to `delegate.rs`, directive HTTP
  handlers to `directives.rs`, and run-path construction to `session_paths.rs`.
- Have `delegate.rs` build both initial and continuation commands through one
  helper. The helper calls `env_clear()`, restores only an explicit allowlist of
  basic process/locale variables and documented provider credentials, and then
  sets `COMMANDAGENT_EVAL_EVENTS` explicitly. No ambient `COMMANDAGENT_*`
  variable, including `COMMANDAGENT_PACK_*`, is admitted.
- Rewire routes to the owning modules without changing request/response types,
  status codes, error codes, arguments, or event paths.

## Verification

- Extend the Unix GUI integration test so its fake CLI records the delegated
  environment, proving an allowlisted credential is retained and representative
  `COMMANDAGENT_PACK_*` and unrelated variables are absent.
- Update the GUI process-surface guard and protection coverage audit for the new
  delegate location, retaining explicit negative examples for process creation
  outside that module and adding env-sanitization markers.
- Run the focused GUI server and guard tests, then formatting, Clippy, and the
  full Rust test suite because process policy and shared integration behavior are
  touched.
