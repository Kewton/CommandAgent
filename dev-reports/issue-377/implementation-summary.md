# Issue #377 implementation summary

- Inspected all four committed predecessor branches before production edits.
  Fast-forwarded the Issue #374 branch (including Issue #370), merged the exact
  Issue #375 branch, and merged Issue #376 while retaining both workspace-path
  and typed-task smoke coverage.
- Added the shared public `eval_events::failure_explanation` leaf. Its typed,
  serializable model isolates the final continuation interval, validates exact
  Issue #375 schema-v1 started/terminal identities, classifies seven failure
  categories, and independently bounds every string, command, output, list,
  observation, changed path, and verification failure.
- Projected structured location, cause, command/verification/release/probe
  evidence, completed phase/task progress, repair attempts, Issue #374
  workspace and partial-artifact state, machine codes, and the exact correlated
  `recovery_prompt_saved` handoff. Successful continuations suppress earlier
  failures, while legacy or incomplete records use an explicit unknown
  fallback.
- Added the projection additively to authenticated session status without
  changing event names or schemas. All projected text passes through the
  existing execution-root redaction.
- Added an authenticated GET-only recovery-document route. It permits only the
  current projection's exact non-truncated repair prompt or Recovery Plan path,
  requires an available per-session workspace, rejects traversal and symlinks,
  and reuses the bounded text-document reader.
- Added the ordered Gate 4 result-detail card with location, cause, evidence,
  progress/workspace state, recovery actions, and collapsed technical details.
  Native keyboard actions open the two saved documents, copy complete suggested
  commands, or prefill and focus the existing continuation textarea. None of
  these actions saves, confirms, dispatches, or executes recovery.
- Added failure, continuation-success, and legacy corpus fixtures; shared-model
  unit tests; an authenticated GUI server integration test; read-only/source
  guards; and dual-base-path Playwright coverage for desktop, mobile,
  accessibility, GET-only opens, keyboard copy/apply, and legacy fallback.
- Updated the GUI Trial guide, shell design, help map, mechanism ledger, and
  changelog. Gate 1, directive confirmation, verification, acceptance, release
  gating, evidence limits, historical events, and the live `.anvil/` namespace
  remain unchanged.
