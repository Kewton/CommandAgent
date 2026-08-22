# Issue 214 implementation summary

## Outcome

Python CLI Gate 1 identities now use the `generic` task-family spelling for
non-filter CLI work. The filter family remains `filter`, so its existing
`絞り込み (filter)` presentation is unchanged.

## Changes

- Replaced the typed `Stats` enum variant with `Generic` in the family catalog.
- Retained `TaskFamilyId::Stats` as a source-compatible associated constant.
- Kept `stats` as a parser alias of `generic`, preserving compatibility with
  existing classifier output, Rust callers, and the immutable historical band
  vocabulary.
- Kept the historical CLI band row name `stats`; it resolves to the same typed
  generic family and existing `BandValue` without changing evidence assets.
- Updated the catalog-local cross-language vocabulary guard to compare typed
  identities after alias resolution.
- Added focused tests for canonical and legacy spelling, `cli` profile alias
  compatibility, and rendered generic/filter Gate 1 cards.

## Scope control

Production changes are limited to
`src/tui/boundary_shell/family_catalog.rs`. No presentation, ambiguity,
historical evidence, demo, README, or documentation asset was edited.
