# Issue #408 implementation summary

## Outcome

GUI Trial can now stop the active initial run or confirmed directive
continuation through an authenticated, same-Origin operation bound to the exact
session and process generation. The delegated CLI runs in its own process
group, receives `SIGINT`, and is escalated to `SIGKILL` after a bounded grace
period when necessary.

## Implementation

- Added `gui_server/trial_process.rs` as the only GUI signal-capable module. It
  owns the in-memory session/generation/PID/process-group binding, idempotent
  stop state, bounded escalation, group-disappearance verification, and
  server-owned stop audit events.
- Added `POST /api/sessions/{id}/stop`. The handler requires Trial
  authentication, an allowed Origin, a canonical session ID, a current active
  event interval, the matching running workspace lease, and the current
  process generation before signaling.
- Isolated both initial and continuation CLI launches into fresh process
  groups. A fresh UUID generation is returned at launch and continuation time
  and is projected only while that process is active.
- Kept honest terminal semantics: CLI-handled interruption remains exit 130
  with `tui_command_stop.status=interrupted`; forced or unverifiable stops add
  `gui_trial_stop_completed` with `ok=false`. The lease returns to `Idle` only
  after terminal evidence and confirmed process-group disappearance, otherwise
  it remains `RecoveryRequired`.
- Added an active-Gate-2-only stop control with explicit confirmation,
  keyboard-native actions, stopping feedback, actionable failure feedback,
  and polling-driven terminal transition.
- Documented the lifecycle and recovery boundary, added the Issue #408 event
  corpus, and narrowly extended the GUI read-only guard to allow signals only
  in the dedicated stop module while pinning authentication and ownership
  checks.

## Tests

- Added GUI server integration tests for authentication, Origin validation,
  malformed/foreign/terminal/recovery-required sessions, stale generations,
  duplicate requests, graceful exit 130, forced process-group cleanup, and
  confirmed continuation stopping.
- Added lease-level coverage for an unverifiable process tree remaining
  `RecoveryRequired`.
- Extended the dual-base-path Playwright smoke to exercise the confirmation,
  failure/retry, pending, and exact generation-bound request flow by keyboard.
- Updated the existing status projection and guard contracts for the additive
  `process_generation` field and dedicated signal boundary.
