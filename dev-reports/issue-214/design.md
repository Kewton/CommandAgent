# Issue 214 design

## Scope

- Change only `src/tui/boundary_shell/family_catalog.rs` and its focused tests,
  as required by the Wave 4 ownership decision.
- Do not edit Gate presentation, ambiguity routing, demo assets, or historical
  band evidence.

## Design

- Replace the typed CLI `Stats` family variant with `Generic`. Its canonical
  identity string is `generic`, so a non-filter Python CLI proposal no longer
  renders as `filter` or claims filtering work.
- Keep `TaskFamilyId::Stats` as a source-compatible associated constant and
  accept both `stats` and `generic` while parsing. Existing Rust callers,
  classifier output, and the historical `stats` band row therefore continue to
  resolve to the same typed family and band value.
- Keep `Filter` unchanged. Requests with filter evidence can still select the
  canonical `filter` family and retain the existing Japanese Gate 1 label.
- Update the catalog-local Rust/Python vocabulary guard to compare resolved
  typed identities. The Python band vocabulary remains immutable and may keep
  its historical `stats` spelling.

## Tests

- Verify `generic` and legacy `stats` parse to one typed identity and that
  `TaskFamilyId::Stats` remains source-compatible.
- Build focused Python CLI Gate 1 cards from the generic and filter catalog
  entries. Assert that generic cards show `generic` and never show
  `絞り込み`, while filter cards retain `絞り込み (filter)`.
- Verify the legacy `cli` profile alias and canonical `python-cli` profile see
  the same catalog entries.
