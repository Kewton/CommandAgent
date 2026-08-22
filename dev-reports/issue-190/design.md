# Issues #190, #191, and #192 design

## Scope

This combined GUI change keeps the browser document alive through Next.js
client-side route transitions, shortens runtime-status staleness after a Trial reaches a
terminal state, and exposes the active primary route to assistive technology.
Production edits remain within the approved ownership:

- `gui/components/shell.tsx` owns shell navigation and active-route semantics.
- `gui/lib/use-runtime-status.ts` owns the runtime-status refresh cadence.

No API, event schema, Trial mutation, acceptance, or `.anvil/` runtime contract
changes. The row #172 predecessor commit `f1339310` was inspected: its automatic
GET-only Trial reconnect and existing `?session=<id>` link wording are preserved.
This branch does not duplicate or rewrite those predecessor-owned changes.

## Design

1. Render the brand, runtime-session badge, and five primary navigation items
   with Next.js `Link`. Pass route-local paths to `Link` so the configured Next.js
   `basePath` supplies either the root or `/proxy/commandagent` prefix without a
   second prefix. Keep the existing visible Japanese labels unchanged.
2. Add `aria-current="page"` only to the primary navigation item matching the
   shell's `active` route.
3. Refresh `/api/runtime-status` every 750 ms while the document is visible. The
   existing single-flight, cancellation, hidden-tab pause, and last-success
   retention behavior remains unchanged. The 250 ms margin keeps the observable
   terminal-state update within the one-second acceptance bound under the
   deterministic smoke response delay.

## Verification strategy

- Extend the GUI browser smoke with a shell-navigation probe that visits all five
  primary routes through their rendered links, checks the active item on every
  page, validates base-path-prefixed hrefs, and uses a window marker to prove the
  document was not reloaded. Run this probe for both configured base paths.
- Extend the focused session-index smoke to change a synthetic runtime session
  from running to idle and require the header to refresh within one second for
  both base paths, while retaining the existing single-flight and visibility
  assertions.
- Pin the new source and smoke contracts in the focused Rust GUI guard test.
- Run GUI typecheck, lint, builds/smokes, the focused Rust guard, then repository
  formatting, Clippy, and the full Rust test suite because shared GUI behavior is
  touched.
