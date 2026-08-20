# Issue 165 implementation summary

## Outcome

Fixed the GUI pack wizard so **保存済み bytes を再検証** cannot leave an
unsaved editor representation paired with a pinnable hash for different
server bytes. After successful re-verification, the wizard now reads the saved
pack detail and reconciles every editor member before exposing the report.

## Changes

- Reused the existing authenticated pack-detail GET client after the existing
  verify POST; no server route, filesystem write boundary, or response schema
  changed.
- Added a local member-map conversion for `assist.yaml`, `eval.yaml`, and
  sorted direct `materials/*.md` editor rows.
- Clarified in the wizard and GUI extension guide that saved-byte
  re-verification restores the editor to the server's persisted exact bytes.
- Extended the focused source guard to retain the read-back and acceptance
  smoke assertions.
- Extended the provider-free wizard smoke with the reported sequence: valid
  stage, unsaved valid edit, saved-byte re-verification, pin, and full member
  comparison between the pre-pin editor and post-pin pack detail.

## Evidence

The browser smoke passed for both `/` and `/proxy/commandagent/`. Both cases
record `displayed_bytes_reconciled: true`, `pinned_bytes_match_display: true`,
the same verified/reverified hash, and no unexpected console errors in
`dev-reports/issue-165/smoke/browser-smoke.json`.

The required Issue 243 predecessor commit was inspected before implementation.
Its headless provider-usage delta does not overlap this GUI behavior, so no
unmerged predecessor files were imported.
