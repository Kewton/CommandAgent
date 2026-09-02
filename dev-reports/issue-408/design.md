# Issue #408 design

## Scope

Add one authenticated, same-Origin GUI Trial stop operation for the currently
running process generation. The operation must cover both the first confirmed
run and a confirmed directive continuation without turning interruption into a
successful result or weakening any existing gate.

## Design

- Add a leaf `gui_server/trial_process.rs` module as the sole owner of GUI
  signal capability. It tracks one in-memory `(session_id, generation, pid,
  process_group)` identity, starts every delegated CLI in a fresh process
  group, sends `SIGINT` once to that exact group, and escalates that same group
  to `SIGKILL` after a bounded grace period.
- Generate a fresh opaque generation for every delegated CLI process. Return
  it from create/continuation responses and the live status projection. Require
  both session ID and generation in `POST api/sessions/{id}/stop`; a duplicate
  request for the already-stopping identity returns the same accepted result.
  A terminal, recovery-required, foreign-session, stale-generation, or
  pre-restart request is rejected before signaling.
- Keep the workspace lease and process registry as independent bindings. The
  stop handler first requires the current lease to be `Running` for the route
  session, then atomically checks the registry generation. Completion returns
  the lease to `Idle` only after terminal evidence exists and the process group
  is confirmed absent; otherwise it remains `RecoveryRequired`.
- Preserve CLI-owned interruption evidence when the CLI handles `SIGINT`
  (`tui_command_stop.status=interrupted`, `ok=false`, exit 130). If the CLI
  fails to write a current terminal (including an ignored `SIGINT` followed by
  forced termination), append a distinct server-owned
  `gui_trial_stop_completed` terminal event with `ok=false`, force/escalation
  facts, and process-tree verification. Also append a non-terminal
  `gui_trial_stop_requested` audit event for every first accepted stop.
- Expose the control only on active Gate 2. The browser shows an explicit
  confirmation step, keyboard-operable cancel/stop actions, a stopping status,
  and a non-color-only failure alert. Polling remains the authority for the
  terminal transition.

## Tests and compatibility

- Add Unix integration coverage for authentication, Origin, IDs, lease and
  generation ownership, idempotence, CLI-handled interruption, forced stop,
  descendant cleanup, unverified process-tree recovery, restart-stale PID
  rejection, and directive continuation.
- Extend the GUI smoke for both `/` and `/proxy/commandagent/`, plus source
  guard assertions that whitelist signals only in the stop module and pin the
  authentication/lease/generation binding.
- Add an Issue #408 corpus fixture for CLI-owned and server-owned interruption
  contracts. Existing event names and fields remain unchanged; all new event
  data is additive.
