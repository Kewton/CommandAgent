# Issues #172 and #167 implementation summary

## Outcome

GUI Trial result routes now restore their server-backed session automatically
after reload. The reconnect path reuses the existing GET-only session fetch,
removes the consumed `sample` query parameter, and retains the user's submitted
goal from the session response instead of reinjecting the sample draft.

Terminal browser titles now use an honest gate marker: Gate 3 renders `✔`, while
Gate 4 renders `✗`. The title uses the rendered result heading and the coordinated
`| CommandAgent` separator, so a failing Gate 4 never receives a success mark.

## Changed files

- `gui/hooks/use-trial-monitor.ts`
  - consumes `sample` before compose initialization when a session route loads;
  - automatically reconnects each session/token pair once access is ready;
  - canonicalizes launched and restored session URLs;
  - clears a stale session query when starting a new run; and
  - projects the terminal Gate 3/Gate 4 marker into the browser title.
- `gui/scripts/smoke.mjs`
  - covers sample-goal replacement, reload restoration, GET-only reconnect, and
    a failing Gate 4 title without a check mark;
  - updates the full lifecycle smoke for automatic reconnect.
- `gui/scripts/session-index-smoke.mjs`
  - covers automatic GET-only reconnect from both the history row and runtime
    header links, including rejected-token recovery.
- `tests/gui_read_only_guard.rs`
  - pins the source and smoke evidence for the new reconnect and title contracts.

## Scope notes

The existing `?session=<id>` links in `gui/components/trial-session-index.tsx`
and `gui/components/shell.tsx` already satisfy the shared route contract, so no
production edit to either component was necessary. Their behavior is exercised
by the session-index smoke. Row #159-owned files, event schemas, APIs, acceptance
logic, and the `.anvil/` runtime namespace remain unchanged.
