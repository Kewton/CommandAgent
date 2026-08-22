# Issues 159, 163, and 170 design: bounded GUI Trial recovery

## Problem

- A manual reconnect to a nonexistent session performs one GET, but its 404 is
  described as though the GUI will retry with backoff.
- A 404 received by the active poller is treated as transient and is retried
  forever, even though the requested session cannot appear at that identity.
- Malformed event JSONL is only recognized through fragile English-text
  matching, so the additive server code from the predecessor is not consumed.
- The recovery-required response now carries a stable `session_id`, but the GUI
  still looks only for session IDs embedded in older human-readable messages.

## Predecessor contract

Commit `4f7d822c` on `feature/issue-163-170` preserves the existing statuses and
messages while adding:

- top-level `session_id` on `409 trial_workspace_recovery_required`; and
- `trial_session_events_invalid` for malformed session event streams.

This row consumes those fields and does not edit server files.

## Design

1. Extend `GuiRequestError` with the optional response `session_id`. Use that
   structured value for recovery-required reconnects while retaining the old
   message extraction for running/conflict compatibility. The existing UI then
   renders its single reconnect link, whose handler uses the established GET-only
   reconnect path.
2. Classify HTTP 404 responses, including `trial_session_not_found` and
   `trial_session_file_not_found`, as terminal monitor failures with guidance to
   verify the ID and either reconnect or begin a new run. A manual reconnect
   still performs exactly one GET and now reports one non-retrying 404 message;
   an active poll uses the existing four-failure terminal bound.
3. Prefer `trial_session_events_invalid` for malformed-event classification,
   retaining the legacy JSONL-message fallback for compatibility. Keep the
   existing four-failure terminal bound and provide dedicated repair guidance.
4. Extend `smoke:errors` with deterministic browser coverage for the structured
   recovery link, one-shot missing-ID reconnect, bounded polling 404, and bounded
   coded invalid-event failures. Assert request methods/counts and visible
   guidance without mutating server behavior.

## Verification plan

- Run GUI TypeScript checking and `npm run smoke:errors` first.
- Run the GUI lint and production build checks.
- Run the focused GUI read-only guard because it pins shared monitoring and
  error-descriptor contracts.
- Run repository formatting, Clippy, and the full Rust test suite because the
  shared GUI client behavior and smoke contract are touched.
