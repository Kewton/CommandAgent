# Issue 70 Implementation Summary

## Outcome

GUI Trial failures now have an authenticated, bounded route from the Gate 2
footer and Terminal view to `events.jsonl`, `summary.md`, and other admitted
text artifacts. Both supported base paths render those files inside the
existing read-only document viewer; no filesystem path becomes a browser
navigation target.

## Server

- Added a `session_files` leaf module with GET-only handlers for artifact
  inventory, one bounded artifact, and a bounded recent-event tail.
- Reused the repository viewer's extension allowlist, depth-four walk, skipped
  directories, ordering, document schema, 256-entry cap, and 1 MiB view cap
  without changing existing `workspace/management/runs` handlers.
- Required the existing Trial token and canonical UUID guard. The `.anvil`,
  `runs`, UUID run directory, and each selected file component reject
  symlinks, while canonical containment is still enforced.
- Implemented backward 64 KiB chunk scanning so a tail remains readable when
  the complete event stream exceeds the status API's 4 MiB limit.

## GUI and browser evidence

- Added Japanese recent-events and artifact actions to the Gate 2 footer.
- Automatically opens the artifact inventory at Terminal and exposes stable
  `events.jsonl` and `summary.md` selectors.
- Reused `DocumentViewer`; all reads use the in-memory Trial token and the
  base-path-aware API helper.
- Integrated the viewer with current monitoring recovery, ETag/304 polling,
  reconnect, lifecycle locking, elapsed/phase feedback, and responsive layout.

## Tests and compatibility

- GUI server tests cover missing tokens, canonical UUIDs, traversal, symlinked
  files/run directories/runtime roots, list/file/tail bounds, and a successful
  tail from a stream larger than 4 MiB. `/api/runs` remains readable without a
  Trial token.
- The read-only guard pins GET-only routing, mandatory Trial guards, bounded
  readers, authorized UI fetches, viewer hooks, and null stdout/stderr.
- No event name/schema, recovery/corpus contract, historical evidence, or live
  `.anvil` namespace changed.

## CLI output decision

Delegated stdout/stderr remain `Stdio::null()`. A future raw log requires a
separate CLI-owned output contract; the GUI server remains read-only.
