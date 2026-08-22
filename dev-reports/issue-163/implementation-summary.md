# Issues 163 and 170 implementation summary

## Implemented

- Extended the shared GUI JSON error response with an optional top-level
  `session_id` field. Callers that do not attach an identifier still emit their
  existing response shape.
- Added `session_id` to `409 trial_workspace_recovery_required` responses after
  validating the identifier from the existing lease-conflict message as a
  canonical UUID. The existing status, `code`, and `error` fields are unchanged.
- Changed malformed session event JSONL failures from the generic
  `trial_internal_error` code to `trial_session_events_invalid`. HTTP 500 and
  the existing serde diagnostic in `error` are preserved.
- Added HTTP integration coverage for both contracts. No GUI client files,
  event schemas, workspace lease policy, or `.anvil/` runtime paths changed.

## Files

- `src/bin/gui_server/error_response.rs`: optional additive session identity.
- `src/bin/gui_server/sessions.rs`: recovery identity attachment and dedicated
  malformed-events error mapping.
- `tests/gui_server.rs`: recovery-required and invalid-events server contracts.
- `dev-reports/issue-163/`: design, implementation, and verification records
  for the grouped Issues #163 and #170 row.
