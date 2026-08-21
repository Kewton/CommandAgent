# Issue 245 implementation summary

## Outcome

Extension roots can now contain both `profiles/` and `packs/` without
`--packs` mistaking a profile directory for a legacy local pack. Pack listing,
external draft-profile loading, and pinned local-pack selection all work from
the same root.

## Changes

- Updated `src/pack_actions.rs` so the legacy top-level pack traversal skips
  both reserved extension namespaces, `packs/` and `profiles/`. The canonical
  `packs/<id>/<version>` traversal remains unchanged, as does strict
  conformance for every discovered pack candidate.
- Added `co_located_profiles_and_packs_remain_independently_usable` in
  `tests/pack_actions.rs`. It builds an extension root containing the existing
  `static-site` draft manifest and a pinned `my-cli-pack@1.0.0`, then verifies
  listing, profile loading, and pack selection through the real CLI binary.
- Added an Unreleased changelog entry for the user-visible discovery fix.

No event, schema, runtime-state, profile-manifest, or pack-conformance contract
was changed.
