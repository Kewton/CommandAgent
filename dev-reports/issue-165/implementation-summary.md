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

## CI follow-up

- Cherry-picked only Issue 160 code commit `714017ca` as local commit
  `1523752d`; its sole changed blob is
  `src/bin/gui_server/session_files.rs`, and that blob is byte-identical to the
  source commit.
- Did not apply Issue 160 report commit `1f28c021` or add any Issue 160 report
  files.
- The imported Rust 1.98 compatibility change boxes the existing Axum error
  response and returns that same response from `IntoResponse`. It does not
  rebuild the response, so status, headers, and JSON body bytes remain intact.
  The path-confinement and symlink-rejection branches are unchanged.
- Added no lint allowance. Rust 1.97.1 clippy passes with `-D warnings`, and the
  focused session-file tests pass for authentication, response contracts,
  bounded reads, path confinement, and symlink rejection.
- Re-ran the Issue 165 wizard source guard, typecheck, smoke syntax check, and
  two-base-path browser smoke after the cherry-pick. Both smoke cases still
  report reconciled displayed bytes and exact pinned-member equality.
