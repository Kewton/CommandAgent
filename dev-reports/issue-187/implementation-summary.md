# Issues #187, #195, and #198 implementation summary

## Implemented

- Localized the remaining literal English headings, labels, actions, lifecycle
  states, and verification result labels in the extension pack wizard and the
  owned assets/run pages. Technical identifiers, file names, hashes, wire
  values, request payloads, and lifecycle rules are unchanged.
- Presented `community-mini-app` as `コミュニティ・ミニアプリ` in the
  wizard profile selector and assets catalog without changing the shared
  profile descriptor owned by another row. Intent values remain the existing
  `create` / `fix` / `investigate` wire values while their labels are rendered
  as `作成` / `修正` / `調査`.
- Reused the Issue #176 predecessor's `repositoryRunStatusLabel` in the run
  picker, replacing raw visible repository status values with stable Japanese
  labels while retaining raw values in the search corpus.
- Added `aria-current="true"` to the selected run acceptance/evidence entry
  and measurement report entry, keeping the existing visual `active` class and
  selection behavior.
- Routed wizard step changes through a focus-aware helper. Each newly mounted
  step focuses its `tabIndex={-1}` heading, validation issue links focus the
  corresponding editor field, lifecycle transitions retain a visible focus
  target, and closing restores focus to the launcher. The step rail also
  announces its current Japanese step through an atomic polite live region.

## Scope control

The authored production diff contains only the four approved files. The branch
was first fast-forwarded to the committed Issue #176 predecessor so this row
could consume its shared formatter contract unchanged. The successive
orchestrator amendments were applied only to their named literals in
`tests/doc_drift.rs`, `docs/user/gui-help-map.md`, `gui/scripts/smoke.mjs`, and
`tests/gui_read_only_guard.rs`. The five GUI guard edits change only direct
expected strings; guard logic is untouched. The final smoke edit changes only
one button name, leaving control flow and every other selector unchanged. No
Trial component, shared style, runtime state, or historical evidence file was
edited.

The local Next probe's generated `gui/AGENTS.md` and `gui/CLAUDE.md` files were
removed, and `gui/next-env.d.ts` plus `gui/tsconfig.json` were restored exactly
to their predecessor state before reporting or staging.

## Test coverage

The temporary Playwright probe verified the Japanese community profile label,
every wizard focus transfer, focus restoration on close, localized run status,
and both `aria-current` selection contracts; it was removed after the probe
passed. The focused document-drift contract exposed the Trial and pack-wizard
help-copy dependencies in sequence; after their exact amendments it passes.
The next full suite reached `tests/gui_read_only_guard.rs` and initially passed
22 of 25 tests, exposing five stale direct expectations across three tests.
After the exact guard amendment, the focused 25-test guard and the complete
Rust suite pass.

The official wizard smoke passed its help-map ownership checks after the
authorized smoke entries were aligned. Its sandbox run could not bind loopback
and returned `EPERM`; the first outside-sandbox run then failed on the stale
selector `保存済み bytes を再検証`. After the final one-selector amendment, the
official smoke passed both root and proxied base-path cases, including its
keyboard, focus, lifecycle, and accessibility checks. No compatibility-only
English marker, fallback label, guard weakening, or unauthorized smoke edit
was introduced.
