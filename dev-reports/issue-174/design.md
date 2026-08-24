# Issues 174, 175, 176, 179, 180, 196, 197, 198, and 202 design

## Scope

Implement only the W6 G Trial row in the twelve production paths assigned by
the dispatch. The branch is fast-forwarded to the verified Issue 176
predecessor so this row can consume its shared Japanese gate, session, phase
status, and phase stage formatters without changing that contract. Pages,
wizard behavior, shared error/format modules, corpus fixtures, and the final
browser integration harness remain outside this row.

## Design

- Keep the request form continuous: place access-token setup before the goal,
  then render goal, profile, pack, provider, and both model inputs without
  lease or reconnect cards between them. Move the read-only lease inspection
  and manual reconnect controls below the request action.
- When the observed lease is not idle, show a concise notice above the request
  fields with a native reconnect button, disable contract proposal, and carry
  the session ID into reconnect state. Recheck the lease during proposal and
  immediately before launch so a known running session never reaches the
  create-session POST. Retain the server-side 409 path as the honest
  concurrency backstop.
- Project plan-generation lifecycle events as an additive phase-zero planning
  row. This gives early Gate 2 activity a truthful visible state; if no phase
  projection exists yet, explicitly say how many events are present and that
  phases have not started.
- Use the predecessor's total display formatters for the Gate 2 session,
  phase-stage, phase-status, and directive-gate values. Keep wire values and
  CSS state classes unchanged. Localize remaining Trial surface terminology
  and use one visible action name, `契約と見積りを確認`, including normalized
  guidance from the shared error helper.
- Announce monitor connection-state changes through a dedicated atomic live
  region, using assertive priority only when monitoring is lost. Add a real
  label for the additional-request textarea.
- Give the stage rail `ol`/`li` semantics and expose its current step in a
  polite live region. Replace reconnect anchors that cancel navigation with
  native buttons so Enter and Space retain browser-native activation.
- On every actual stage transition, move focus to the newly mounted,
  programmatically focusable stage container on all viewport sizes. Preserve
  mobile scrolling after focus and add an explicit visible focus treatment for
  the stage containers.

## Tests and verification

- Add focused `sessions.rs` unit tests for running, completed, failed, and
  absent plan-generation projections while preserving existing phase
  terminality tests.
- Run the focused GUI-server session tests first, followed by GUI lint,
  TypeScript checking, and a production build.
- Because Rust projection and shared GUI behavior are touched, also run Rust
  formatting, Clippy with warnings denied, and the complete Rust test suite.
  Do not update successor-owned smoke or guard files in this row.
