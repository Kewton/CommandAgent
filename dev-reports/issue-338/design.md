# Issue #338 design

## Problem

At `develop` SHA `ee8408390533a37cf33e03716b5e7d48b56841ac`, both
attempts of the GitHub Actions `CI / GUI Dashboard` job failed in
`confirmed_session_delegates_with_cli_event_bytes_unchanged`. The test writes
and makes a temporary shell fixture executable, then immediately launches it.
On the Linux runner that launch returned OS error 26,
`ExecutableFileBusy` (`Text file busy`), while the other 33 GUI server tests
passed.

## Change

- Keep the retry entirely in `tests/gui_server.rs`; production process
  launching and GUI server behavior remain unchanged.
- Route only the immediate execution of the freshly written CLI fixture
  through a small helper.
- Allow at most four total launch attempts, separated by 25 milliseconds.
- Retry only an OS error whose `raw_os_error()` is exactly
  `Some(libc::ETXTBSY)`. Return every other error immediately, including a
  synthetic `ErrorKind::ExecutableFileBusy` without raw errno, and return the
  final ETXTBSY error after the bound is exhausted.
- Add a focused test for the retry predicate so the raw-errno filter, the
  synthetic-error rejection, and the attempt bound are explicit.

## Verification

Run the retry-contract test and the exact previously failing test first, then
the full GUI server integration target, the required GUI server Clippy command,
formatting, and the repository-wide Rust checks because this is a CI-sensitive
test harness change. No production behavior, event schema, acceptance rule, or
verification gate is changed.
