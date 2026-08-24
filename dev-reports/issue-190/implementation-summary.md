# Issues #190, #191, and #192 implementation summary

## Outcome

The GUI shell now uses Next.js client navigation for its brand, runtime-session
badge, and five primary routes. Next.js applies the configured base path to the
route-local hrefs, so the same links work at `/` and `/proxy/commandagent/`
without a document reload. Existing Japanese navigation and running-session
wording remains unchanged, including the Trial session link consumed by the row
#172 automatic reconnect flow.

The active primary navigation item now exposes `aria-current="page"`. Runtime
status refreshes every 750 ms while visible, retaining the existing single-flight
request, hidden-tab pause, abort, and last-success-on-error behavior. The focused
browser smoke observed synthetic running-to-idle transitions in 283 ms at the
root base path and 179 ms at the proxy base path.

## Changed files

- `gui/components/shell.tsx`
  - replaces shell-owned raw anchors with Next.js `Link`;
  - leaves base-path prefixing to the configured Next.js router; and
  - marks only the active primary route with `aria-current="page"`.
- `gui/lib/use-runtime-status.ts`
  - changes the visible-tab refresh interval from 3,000 ms to 750 ms.
- `gui/scripts/smoke.mjs`
  - navigates through all five routes at both base paths;
  - proves the browser document survives every transition; and
  - verifies active-route semantics and rendered hrefs.
- `gui/scripts/session-index-smoke.mjs`
  - measures running-to-idle runtime refresh latency at both base paths and
    requires it to remain within one second.
- `tests/gui_read_only_guard.rs`
  - pins the Link, `aria-current`, refresh interval, and smoke evidence contracts.

## Scope notes

The implementation does not modify Trial monitoring, APIs, event schemas,
acceptance gates, runtime state, or predecessor-owned reconnect behavior. The
row #172 commit was inspected before editing and its production and smoke changes
remain independently mergeable.
