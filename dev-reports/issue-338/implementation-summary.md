# Issue #338 implementation summary

## Delivered

- Added a test-only command runner in `tests/gui_server.rs` for the freshly
  written fake `commandagent` shell fixture.
- Limited execution to four total attempts with a 25 millisecond delay between
  retryable failures.
- Restricted retries to `std::io::ErrorKind::ExecutableFileBusy`; all other
  launch errors return immediately, and ETXTBSY still fails after the fourth
  attempt.
- Routed only the direct fixture execution in
  `confirmed_session_delegates_with_cli_event_bytes_unchanged` through the
  helper.
- Added a focused regression test that pins the retryable error kind and the
  final-attempt boundary.

## Scope

No production source, Cargo feature, CI workflow, event contract, acceptance
condition, or verification gate changed. The local `gui/node_modules`,
`gui/.next`, `gui/out`, and `target` directories used for verification remain
ignored and are not part of the delivery.
