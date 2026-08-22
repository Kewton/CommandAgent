# Issue #153 implementation summary

## Outcome

The GUI Gate 3/4 screen now exposes the recorded result, reason, and typed next
action before any evidence browsing. A static Gate 4 can truthfully say that
the command and final acceptance completed while the independent CLI probe did
not run, without changing the Gate or assurance decision.

This row also completes the assigned residual GUI behavior from closed Issues
#148 and #149: Gate 4 uses a `✗` title, a hidden page with existing notification
permission receives one Gate/duration notification on completion, and a reload
of an already terminal session is not notified as a new completion. No Issue
lifecycle state was changed.

## Implementation

- Added nullable `assurance_reason`, `stop_reason`, and `next_action` fields to
  the session status response and TypeScript contract. Values are projected
  from only the current directive round, prefer the authoritative terminal
  event, and use the existing acceptance-event next-action fallback. Existing
  response fields remain unchanged.
- Replaced the prior three generic GUI lines with three reader-first Japanese
  rows: recorded execution/final-acceptance result, Gate reason, and typed next
  action. Known closed identifiers receive Japanese explanations; unknown
  diagnostics remain visible after a Japanese label.
- Kept Gate 3/4 ownership on the server-projected `gate`. A completed command
  and successful final acceptance can therefore remain Gate 4 when assurance
  is static or required evidence is missing.
- Folded the raw acceptance sheet under **受入シートの詳細を表示**, renamed
  **失敗の証跡** to the Gate-neutral **セッションファイル**, and ordered the
  terminal DOM as result, optional additional request, then session files.
- Consolidated the terminal-owned title behavior on `✔` for Gate 3 and `✗` for
  Gate 4. Notification is a no-op unless the page is hidden, the API exists,
  permission is already `granted`, and the screen just moved from Gate 2.
  The GUI never requests permission.
- Registered the new stable result guidance in the GUI help map and its smoke
  ownership checks.

## Tests and compatibility

- Added focused server projection tests for verbatim terminal values,
  acceptance fallbacks, missing values, and isolation across directive rounds.
- Extended the session API integration test to pin all prior fields plus the
  three additive fields and their recorded/null values.
- Extended the read-only source guards and the two-base-path feedback browser
  smoke to cover Japanese result copy, honest Gate 4 title, folded details,
  section ordering, and the hidden-page completion notification.

No event name, event field, Gate/assurance rule, acceptance threshold, corpus
contract, historical evidence, or `.anvil/` path changed, so no corpus fixture
was updated. Compose files, sample goals, duration discovery, CLI/TUI output,
and Issue state remained untouched.
