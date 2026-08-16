# Issue 72 implementation summary

## Outcome

GUI API failures now carry stable machine-readable codes and every authored
fetch path turns them into a next action. Existing HTTP statuses and `error`
strings remain unchanged.

## Changes

- Added a shared Axum `GuiError` response that emits additive
  `{ "code", "error" }` JSON for both repository-read and Trial APIs.
- Assigned explicit codes to the required 401, 403, 409, 412, 428, and 503
  Trial responses. Other read, validation, size, missing-session, and internal
  failures also receive bounded codes.
- Added `gui/lib/errors.ts` with `responseError`, `describeError`, and active
  session ID extraction. It provides token, Origin allowlist, Gate 1,
  execution-root, CLI path, recovery, repository-read, reload, and reconnect
  instructions while retaining server detail.
- Routed Trial polling/actions, shared resource reads, run detail/evidence, and
  measurement report reads through that common descriptor. Browser-level
  network rejections no longer surface their raw implementation message.
- Added a real reconnect link for running-workspace 409 responses. It preserves
  the UUID in the URL and resumes the existing GET polling path without
  dispatching another CLI process.
- Extended Rust integration tests to verify the exact old `error` string plus
  the new code for 401/403/409/412/428/503, and added a coded read-error check.
- Added a source guard requiring the common descriptor across fetch surfaces
  and rejecting the raw browser message from authored GUI sources.
- Added `npm run smoke:errors`, a deterministic Playwright probe using the real
  server and a bounded fake delegate. It verifies wrong-token guidance, a
  rewritten foreign Origin, and a real live 409 with UUID and reconnect link.
- Documented the response shape, recovery matrix, and focused smoke command in
  `docs/user/gui.md`.

## Compatibility

- No existing status or `error` text changed; the 428 contract remains
  `Gate 1 confirmation_hash is required before dispatch`.
- No event schema, historical evidence, live `.anvil/` namespace, provider
  boundary, or verification gate changed.
- The shared error type remains public within the GUI binary surface so the
  predecessor session-file, session-index, option, runtime-status, and polling
  modules can reuse it when integrated.
