# Issue #108 Implementation Summary

## Implemented

- Fast-forwarded the verified Issue #107 predecessor so pack profile keys use
  the shared profile descriptor constants.
- Added `planner::pack::catalog` with the reviewed admitted registry,
  compatibility enumeration, exact admission checks, the closed `PackSource`
  enum, and runtime-rooted `PackLocator` path/hash resolution.
- Reduced `tui::boundary_shell::pack_catalog` to an adapter over the planner
  catalog and moved the repository-byte hash test to the planner module.
- Added `source` to pinned `PackSelection` records. Older source-less records
  deserialize as `admitted` and retain their schema-v1 card hash, while new
  repository/local sources remain part of the frozen hash.
- Added Japanese supply-source rows to pinned Gate 1 cards and acceptance
  sheets. The `PackSelection::None` card and sheet lines remain unchanged.
- Replaced the production `CARGO_MANIFEST_DIR` reference in `tui/repl.rs` with
  a `PackLocator` rooted at the configured workspace; GUI Gate 1 rendering uses
  its existing repository root.
- Added the one supply-source line to the D-3c display example. The pack
  institution contract and historical evidence were not modified.

## Tests

- Added catalog coverage for enumeration, exact admission, source wire/display
  values, admitted-only resolution, and exact repository hashes.
- Added a persisted legacy confirmation-record fixture without `source` and
  verified loading, defaulting, hash compatibility, and validation.
- Added pinned-source and no-pack presentation assertions for Gate 1 and the
  acceptance sheet.
