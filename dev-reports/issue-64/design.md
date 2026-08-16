# Issue 64 design: recover GUI Trial leases without weakening exclusivity

## Cause

`POST /api/sessions` acquires the workspace lease and persists the Gate 1
confirmation before it calls `Command::spawn`. If spawning fails, the current
error path calls `complete_from_events`; because no child could write a terminal
event, the lease becomes `RecoveryRequired`. The fresh confirmation directory
also survives a GUI server restart, so startup discovers the failed session as
unfinished and recreates the same lease state. The spawn error is converted with
`ToString`, which currently exposes neither an explicit binary path nor a
reliably retained OS cause.

The existing lease is private to the server. Trial users can learn about a
recovery-required session only by attempting another mutating launch and
receiving HTTP 409, and the operator guide has no safe recovery procedure.

## Design

- Keep Gate 1 confirmation before delegation. When `BoundaryShell::dispatch`
  returns before a child was created, remove only that new UUID-scoped run
  directory and return the in-memory lease to `Idle`. If rollback itself fails,
  retain the fail-closed `RecoveryRequired` state and report both failures.
- Format spawn failures as one error containing the configured
  `--commandagent-bin` path and the underlying `std::io::Error`. No shell or
  in-process provider/runner path is added.
- Add an authenticated, read-only `GET /api/trial-workspace` route. It snapshots
  the existing lease mutex as `idle`, `running`, or `recovery_required`, with the
  target session ID for the latter two states. This is a projection of the
  current lease, not a new state store or a reset endpoint.
- Add a Trial-page lease card that reads that route, also refreshing it while
  checking Gate 1 and after a launch conflict. It displays `Recovery required`
  and the exact session ID without offering cancel, reset, or dispatch controls.
- Document recovery as an offline operator procedure: stop the GUI server,
  verify no delegated CLI for the execution root remains, archive the named run
  outside `.anvil/runs`, then restart and re-inspect the lease. Never fabricate
  a terminal event or clear a lease while the child may still be alive.

## Compatibility and predecessor review

Issues 63 (`4313d7ef`) and 66 (`d6f0dec5`) both passed verification but are not
ancestors of this branch. Issue 63 adds GET-only session reconnect and Issue 66
locks launch identity through CLOSED/new-run transitions. This patch does not
copy either predecessor wholesale; the new lease card is independent of their
state machines and preserves their contracts when the branches are integrated.

Event names and schemas, Gate 1 hashing, Origin and Bearer enforcement, the
CLI-only delegation path, the `.anvil` directory layout, and one-process lease
semantics remain unchanged. No corpus contract changes, so no fixture update is
needed.

## Tests and verification

- Extend `tests/gui_server.rs` with missing binary -> HTTP 500 -> install a valid
  binary at the same path -> HTTP 202 in the same execution root. Assert the 500
  body contains the binary path and OS cause, and inspect the lease endpoint.
- Cover the read-only lease endpoint's Bearer protection and
  `recovery_required` session ID projection from an existing unfinished run.
- Extend the static GUI guard for the GET-only lease card and lack of recovery
  mutation controls.
- Run the focused GUI server and read-only guard tests first, then GUI
  typecheck/lint/build, formatting, Clippy, and the full Rust suite.
