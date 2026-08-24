# Issues 163 and 170 design: GUI Trial server error contracts

## Problem

- A `409 trial_workspace_recovery_required` response contains the recoverable
  session UUID only inside its human-readable `error` string. The GUI cannot
  consume that identifier as stable response data.
- Malformed session event JSONL currently reaches the shared internal-error
  mapper, so polling receives the generic `trial_internal_error` code instead
  of a code that identifies an invalid event stream.

## Constraints

- This row owns only `src/bin/gui_server/error_response.rs` and
  `src/bin/gui_server/sessions.rs`; client behavior belongs to row #159.
- Preserve existing HTTP statuses, `code`/`error` fields, error messages, and
  event schemas. New response data must be additive.
- Do not change workspace recovery policy or weaken malformed-event failure.

## Design

1. Extend `GuiError` with an optional session identifier and a builder that
   emits a top-level `session_id` only when present. Existing callers continue
   to serialize the same response fields.
2. When `workspace_conflict` recognizes the recovery-required lease message,
   extract its canonical UUID and attach it to the existing 409 response. Keep
   running and generic workspace conflicts unchanged.
3. Map each malformed non-empty JSONL line to HTTP 500 with code
   `trial_session_events_invalid` and the same serde diagnostic text previously
   returned as `trial_internal_error`.
4. Add focused server tests for the additive recovery response and malformed
   event-stream code. Existing server contracts continue to assert that
   ordinary error response shapes remain unchanged.

## Verification plan

- Run focused `gui_server` tests for recovery-required and invalid-events
  responses first.
- Run the complete GUI server integration test target, formatting, Clippy, and
  the full Rust test suite because the shared server error response is touched.
