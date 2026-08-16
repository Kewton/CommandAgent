# Issue 70 Implementation Summary

## Outcome

GUI Trial failures now have an authenticated, bounded route from the Gate 2
footer and Terminal view to the underlying `events.jsonl`, `summary.md`, and
other text artifacts. Both supported base paths render those files inside the
existing read-only document viewer; no external file URL or filesystem path is
used as a browser navigation target.

## Server

- Added a `session_files` leaf module with GET-only handlers for:
  - `/api/sessions/{id}/artifacts`, returning up to 256 text-document summaries.
  - `/api/sessions/{id}/artifacts?path=...`, returning one text document up to
    1 MiB.
  - `/api/sessions/{id}/events?tail=N`, returning the last `1..=2000` lines with
    a 1 MiB response cap.
- Reused the repository evidence viewer's extension allowlist, depth-four walk,
  skipped-directory set, ordering, document schema, and bounds without changing
  any existing `workspace/management/runs` handler.
- Required the existing Trial Bearer guard and canonical UUID guard before
  every read. The `.anvil`, `runs`, UUID run directory, and every selected file
  component reject symlinks, and canonical containment remains enforced.
- Implemented bounded backward chunk scanning for event tails. A stream larger
  than the status API's 4 MiB whole-file limit can therefore still return its
  diagnostic ending without loading the whole stream.

## GUI and browser evidence

- Added **Recent events** and **Browse artifacts** actions to the Gate 2 footer.
- Automatically opens the artifact inventory at Terminal and exposes stable
  `events.jsonl` and `summary.md` selectors plus all other admitted text
  artifacts.
- Reused `DocumentViewer` so selected content stays in the GUI. All reads use
  the memory-only Trial token and base-path-aware API helper.
- Extended the real two-base-path Playwright smoke. Both `/` and
  `/proxy/commandagent/` reached an honest Gate 4 failure, opened recent events,
  opened `summary.md`, and reported no unexpected browser console errors.

## Tests and compatibility

- `tests/gui_server.rs` covers missing tokens, canonical UUIDs, traversal,
  symlinked files/run directories/runtime roots, the 256-entry list cap, the
  1 MiB file cap, the 2000-line tail cap, and a successful tail from a stream
  larger than 4 MiB. It also confirms `/api/runs` remains readable without a
  Trial token.
- `tests/gui_read_only_guard.rs` pins GET-only routing, mandatory Trial guards,
  bounded readers, authorized UI fetches, in-page viewer hooks, and unchanged
  null stdout/stderr delegation.
- No event name or schema, recovery contract, corpus contract, historical run,
  or live `.anvil` namespace changed, so no corpus fixture or state migration
  was needed.

## CLI output decision

Delegated stdout/stderr remain `Stdio::null()`. The CLI-owned structured events
and `summary.md` are sufficient for this issue's failure investigation path and
are now directly inspectable. The GUI server remains read-only; a future raw log
would need a separate CLI-owned output contract rather than GUI-side capture.
