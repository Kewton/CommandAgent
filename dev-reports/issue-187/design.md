# Issues #187, #195, and #198 design

## Scope

Implement only the approved W6 H pages/wizard row. Production changes are
limited to `gui/components/pack-wizard.tsx`, `gui/app/assets/page.tsx`,
`gui/app/runs/page.tsx`, and `gui/app/measurements/page.tsx`. The branch is
fast-forwarded to the required Issue #176 predecessor so this row can consume
its shared Japanese repository-run formatter without duplicating or changing
that contract.

Honest contract failures produced five narrow ownership amendments. They
authorize only: the `Trial で使う` to `トライアルで使う` expectation in
`tests/doc_drift.rs`; the directly corresponding help-map row, smoke help entry,
and smoke trial-link expectation; the `pack 作成ウィザードを開く` to
`パック作成ウィザードを開く` entries in the same test, help map, and smoke
help registry; five exact expectations in `tests/gui_read_only_guard.rs` for
the localized Trial, version, retired-state, repository-source, and GUI trial
execution-root copy; and the one direct wizard-smoke selector for
`保存済みの内容を再検証`. No guard logic, smoke control flow, or other
expectation is authorized. Trial components, shared styling, runtime state, and
historical evidence remain outside this row.

## Design

- Replace literal English wizard headings, field labels, actions, lifecycle
  labels, and verification-result labels with Japanese display copy. Preserve
  wire values, technical identifiers, file names, hashes, URLs, API payloads,
  and lifecycle behavior unchanged.
- Keep profile and intent option values unchanged, but render Japanese labels.
  Override only the admitted `community-mini-app` display label at the owned UI
  boundary because its shared descriptor is outside this row. Apply the same
  display mapping to extension cards so the raw identifier is not presented as
  the profile label.
- Use the predecessor's `repositoryRunStatusLabel` in the run picker so raw
  repository status values remain searchable but are not rendered to users.
  Localize the remaining repository/trial source copy in the owned page.
- Add `aria-current="true"` to the selected acceptance/evidence button and the
  selected measurement-report button. Keep the existing `active` class so
  visual behavior and selection logic do not change.
- Centralize wizard step changes through a focus-aware transition helper. After
  a step mounts, move focus to its `tabIndex={-1}` heading; after a validation
  issue link returns to the editor, focus the requested field; after closing,
  return focus to the launcher. Existing step buttons and form controls retain
  their keyboard behavior.

## Verification

Run focused source assertions for localized labels, selected-state attributes,
and focus targets; then run GUI lint, TypeScript checking, and the production
build. Run the focused document-drift and complete GUI guard contracts before
the full required suite. Run the official wizard browser smoke when its
loopback server is available outside the sandbox. Report stale direct-copy
dependencies honestly without broadening any amendment or weakening an
assertion. Finish with `git diff --check` and a scope review.
