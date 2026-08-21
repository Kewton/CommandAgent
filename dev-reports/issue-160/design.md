# Issue 160 design: static route canonicalization and 404 page

## Observed behavior

`gui_server` maps paths ending in `/` to an exported `index.html`, but a path
without that slash is treated only as a literal file. As a result, `/try`
does not discover `try/index.html`. Every static read failure also returns
Axum's empty 404 response even when the Next.js export contains `404.html`.

The same static fallback serves both the root deployment and the router nested
under `--base-path`, so the fix belongs in `src/bin/gui_server/static_files.rs`
rather than in either router assembly branch.

## Change

- Keep serving literal files and slash-terminated directory indexes as today.
- When a slashless request has no literal file but its exported
  `<path>/index.html` exists, return a permanent redirect to the slash-terminated
  URL. Build the `Location` from the configured base path so it is correct for
  both `/` and `/proxy/commandagent` deployments, and retain any query string.
- When no requested export exists (or the request path is invalid), read the
  export root's `404.html` and return it with status 404, HTML content type, and
  the existing `no-store` policy. If `404.html` is unavailable, preserve the
  current empty 404 as the safe fallback.

## Compatibility and verification

No API route, API error schema, event, runtime-state, or filesystem-write
contract changes. A focused process-level integration test will create a
minimal static export and prove `/try` -> `/try/`, successful index delivery,
and rendered `/nope/` 404 delivery at both supported base paths. Because shared
Rust server behavior changes, verification will include the focused GUI server
test followed by formatting, Clippy, and the full Rust test suite.

## Rust 1.98 CI follow-up

PR #271's GUI Dashboard CI job runs a newer Clippy that reports
`clippy::result_large_err` for `artifacts`, `events`, and `session_run_root` in
`session_files.rs`. Each function currently stores Axum's full `Response` as
the `Err` variant even though errors are immediately returned through Axum's
`IntoResponse` handler boundary.

Keep the correction local to `session_files.rs`: introduce a one-pointer
response-error wrapper containing `Box<Response>`, implement `From<Response>`
for the existing `?` conversions, and implement `IntoResponse` by moving the
exact response back out of the box. Continue constructing every error response
through the existing functions before boxing it. This changes only the in-memory
`Result` layout; it does not rebuild or reinterpret a status, header, JSON body,
path check, or symlink decision.

The existing focused GUI-server session-file tests already cover successful
artifacts/events, authentication failures, traversal, file and tail limits,
invalid and aliased session IDs, and a symlinked runtime root. Because the
follow-up deliberately changes no observable behavior, those process-level
tests are the focused regression contract; no new response behavior fixture is
needed. Verification will include both default and GUI-feature Clippy, the
pinned Rust 1.97.1 GUI binary Clippy command, focused GUI-server tests, and both
default and GUI-feature full test suites.
