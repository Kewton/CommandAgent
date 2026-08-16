# Issue 70 Design

## Problem and boundaries

The Trial status response projects a terminal result but exposes its
`events_path` only as text. The delegated CLI already writes `events.jsonl`,
`summary.md`, and supporting text artifacts below
`<execution-root>/.anvil/runs/<session-id>/`, while the existing evidence APIs
remain rooted at the repository's `workspace/management/runs`. Issue 70 makes
the former inspectable without broadening or changing the latter.

Issue 71 owns session discovery, so these routes accept only a known canonical
UUID. Cancellation, intervention, state migration, and event/schema changes are
out of scope.

## Predecessor review

The integrated `develop` base contains Issues 63, 64, 66, 67, 68, 69, 76, 77,
and 80. Their monitoring recovery, lease recovery, lifecycle lock, dynamic
Trial options, phase projection, Japanese UI, accessibility, Gate 2 feedback,
and conditional polling remain intact. Issue 70 adds an authenticated
read-only file surface and viewer without changing those contracts.

## Read-only API design

- Add `GET /api/sessions/{id}/artifacts`. With no `path`, return at most 256
  text-document summaries below the session run root, reusing the management
  API extension allowlist, depth-four traversal, skipped directories, and
  deterministic ordering. With `?path=...`, return one document with the
  existing 1 MiB view limit.
- Add `GET /api/sessions/{id}/events?tail=N`. Require `N` in `1..=2000`, cap
  the selected UTF-8 response at 1 MiB, and scan backward in bounded chunks so
  the tail remains available after the whole stream exceeds 4 MiB.
- Require the existing Trial workspace and Bearer token, reject non-canonical
  session UUIDs, canonicalize below `.anvil/runs/{id}`, and reject symlink
  runtime roots, run directories, and selected path components.
- Put handlers and the tail reader in a new leaf module. Expose only generic
  document helpers from the existing API module; leave all management routes
  and response contracts unchanged.

## GUI design

The Gate 2 footer gains authenticated recent-events and artifact-inventory
actions. Terminal automatically opens the inventory, placing `summary.md` and
acceptance-related text files one click away. Selected content renders through
the existing in-page `DocumentViewer`; the Trial token remains memory-only and
is sent only in the existing authorization header. The viewer composes with
monitor health, adaptive polling, elapsed/phase feedback, reconnect, and the
Japanese responsive layout.

## CLI output decision

Choose option (b): do not capture delegated stdout/stderr and leave
`Stdio::null()` unchanged. Structured events and CLI-owned `summary.md` are the
authoritative diagnostic surfaces. A later raw log must be a separate CLI-owned
output contract; the GUI server must not write it.

## Tests and verification

- Cover Bearer enforcement, UUID/path traversal, symlink rejection, 256-entry
  and 1 MiB bounds, the 2000-line limit, and tailing a stream larger than 4 MiB
  in `tests/gui_server.rs`.
- Pin GET-only routes, mandatory guards, authorized UI fetches, viewer links,
  and unchanged null stdio behavior in `tests/gui_read_only_guard.rs`.
- Run the full two-base-path Playwright smoke and open recent events plus
  `summary.md` inside each honest terminal session.
- Run focused GUI tests, syntax/lint/type/build checks, formatting, default and
  GUI-feature Clippy, and the full Rust suite.
