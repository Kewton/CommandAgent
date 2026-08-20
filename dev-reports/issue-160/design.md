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
