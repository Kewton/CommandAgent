# Issue 70 Design

## Problem and boundaries

The Trial status response projects a terminal result but exposes its
`events_path` only as text. The delegated CLI already writes `events.jsonl`,
`summary.md`, and supporting text artifacts below
`<execution-root>/.anvil/runs/<session-id>/`, while the existing evidence APIs
are intentionally rooted at the repository's `workspace/management/runs`.
Issue 70 must make the former inspectable without broadening or changing the
latter.

Issue 71 owns session discovery, so these routes accept only a known canonical
UUID. Cancellation, intervention, state migration, and event/schema changes are
out of scope.

## Predecessor review

The verified Issue 63, 64, 66, 67, 68, 69, and 80 commits were inspected. They
are sibling commits of this branch's `377761a7` base, not ancestors. They cover
poll recovery, lease recovery, lifecycle locking, server-derived launch
options, phase projection, Gate 2 feedback, and conditional polling. None
defines the Trial artifact response contract. This change stays additive so its
authenticated GETs and viewer can compose with those behaviors when integrated;
it does not merge or copy their unrelated changes.

## Read-only API design

- Add `GET /api/sessions/{id}/artifacts`. With no `path`, it returns at most 256
  text-document summaries below the session run root, using the existing
  management API's extension allowlist, depth-four traversal, skipped-directory
  set, and deterministic ordering. With `?path=...`, it returns the existing
  document shape and enforces the existing 1 MiB view limit.
- Add `GET /api/sessions/{id}/events?tail=N`. `N` must be in `1..=2000`; the
  selected UTF-8 response remains capped at 1 MiB. The reader scans backward in
  bounded chunks, so the last lines remain available when the complete event
  stream exceeds the status poller's 4 MiB parsing limit.
- Both routes require the configured Trial workspace and Bearer token, reject
  non-canonical session UUIDs, canonicalize below `.anvil/runs/{id}`, and reject
  every symlink component for individual reads. GET retains the existing Trial
  policy of not requiring an Origin header.
- Put the handlers and tail reader in a new leaf module. Expose only the existing
  generic document helpers needed by that module; do not change any existing
  `workspace/management/runs` route or response.

## GUI design

The Gate 2 footer gains an authenticated **Recent events** action and an
artifact inventory action. At terminal the inventory is loaded automatically,
placing `summary.md` and acceptance-related text files one click away. Selected
events and artifacts render through the existing in-page read-only document
viewer; the token remains memory-only and is sent only in the existing Trial
authorization header.

## CLI output decision

Choose option (b): do not capture delegated stdout/stderr in this issue and
leave `Stdio::null()` unchanged. The non-interactive CLI's structured event
stream and its own `summary.md` are the authoritative diagnostic surfaces this
issue exposes. Adding a product `--log-file` contract would expand CLI and event
ownership without an identified diagnostic gap, while writing a log in the GUI
server would violate its read-only capability guard. A later issue can add a
CLI-owned log only if events and summary prove insufficient.

## Tests and verification

- Extend `tests/gui_server.rs` for Bearer enforcement, UUID/path traversal,
  symlink rejection, 256-entry/1 MiB/2000-line bounds, and tailing a stream
  larger than 4 MiB.
- Extend `tests/gui_read_only_guard.rs` to pin the GET-only routes, guard calls,
  UI authorization, viewer links, and unchanged null stdio behavior.
- Extend the two-base-path Playwright smoke to open both recent events and
  `summary.md` inside the Terminal page.
- Run focused GUI server and guard tests, GUI syntax/lint/type/build checks and
  Playwright smoke, then formatting, Clippy, and the full Rust suite. No corpus
  fixture changes are required because event, recovery, and corpus contracts do
  not change.
