# Issues 159, 163, and 170 implementation summary

## Implemented

- Extended the shared GUI request error with the predecessor's optional
  top-level `session_id`. Recovery-required conflicts now use that structured
  UUID to expose the existing single reconnect link, while running/conflict
  message parsing remains as a compatibility fallback.
- Classified HTTP 404 session reads as terminal monitor failures with guidance
  to check the session ID and execution root, reconnect, or start a new run.
  Manual reconnect remains one GET with one non-retrying 404 message; active
  polling stops through the existing four-failure terminal bound.
- Classified `trial_session_events_invalid` directly instead of relying on the
  server message to contain English JSONL wording. The legacy message match is
  retained for compatibility, and coded failures stop at the same four-attempt
  bound with dedicated event/artifact repair guidance.
- Extended `smoke:errors` with deterministic browser assertions for structured
  recovery identity, exactly one recovery link, GET-only reconnect traffic,
  one-shot missing-session handling, bounded polling 404s, and bounded coded
  invalid-event failures.

## Scope

- `gui/lib/errors.ts`: structured recovery session identity consumption.
- `gui/lib/trial-monitor.ts`: terminal 404 and invalid-event classification.
- `gui/scripts/error-smoke.mjs`: focused end-to-end error/recovery coverage.
- `dev-reports/issue-159/`: design, implementation, and verification records.

The branch was fast-forwarded to predecessor commit `4f7d822c` before this
implementation. This row did not edit server files, event schemas, `.anvil/`
runtime state, or recovery policy.
