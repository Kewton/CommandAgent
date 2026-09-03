# Issue #412 Design

## Problem

GUI Trial currently reports every non-unique Gate 1 route as
`trial_request_invalid`. For the Issue reproduction goal, an omitted intent
leaves both Next.js `create` and `fix` candidates, but the GUI therefore shows
generic input guidance and exposes the candidate list as the only clue.

## Decision

- Keep the deterministic router, intent vocabulary, family catalog, Gate 1
  confirmation, delegation, and assurance paths unchanged.
- In the GUI server's Gate 1 adapter, classify a rejected automatic-intent
  proposal as `trial_intent_ambiguous` only when the request omitted `intent`
  and the remaining deterministic candidates contain more than one distinct
  intent.
- Preserve HTTP 422 and the existing English candidate-list message as the
  technical detail. Same-intent family ambiguity, unknown routes,
  contradictory explicit bindings, and all other request/access/lease errors
  keep their existing codes.
- Map only `trial_intent_ambiguous` in the browser to a complete Japanese main
  message: automatic intent detection failed; choose `作成` for a new app or
  `修正` for an existing app, then retry. Continue appending the server message
  under `詳細` for troubleshooting.
- Keep the existing `role="alert"` error container and form controls unchanged,
  preserving live-region announcement, keyboard access, and non-color text.

## Tests

- Extend the Rust GUI-server integration coverage with the exact reproduction
  goal. Assert the structured ambiguity code and candidate detail, assert that
  same-intent family ambiguity remains `trial_request_invalid`, and assert that
  explicitly selecting `create` still produces Issue #409's `unknown` /
  `未計測` proposal.
- Extend the existing Playwright compose regression, which runs once for `/`
  and once for `/proxy/commandagent/`, with a structured 422 response. Assert
  the actionable Japanese main message, retained technical detail,
  `role="alert"`, usable intent selector, and successful `作成` retry into an
  unmeasured Gate 1 card.
- Run focused Rust and GUI checks first, followed by formatting, clippy, and the
  full Rust suite because the shared GUI-server error contract changes.
