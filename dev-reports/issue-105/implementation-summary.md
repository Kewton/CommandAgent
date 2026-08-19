# Issue #105 Implementation Summary

## Outcome

Fixed the design decision for one additive-only profile overlay slot. An
admitted embedded profile may be used as the immutable base, but selecting an
overlay creates a distinct draft effective profile whose assurance remains
capped at `static`.

## Changes

- Added the complete overlay `manifest.toml` v1 fragment to the authoritative
  profile-manifest documentation.
- Limited additions to artifacts/cardinality, guidance variants, registered
  profile-bound checks, and their evidence-target mappings.
- Fixed identity as `(effective id, base profile, ManifestSource,
  exact-byte hash)`, with `repository` and `local` overlay sources.
- Fixed merge order as `base -> overlay -> pack`, with collision, replacement,
  weakening, chaining, multiple-overlay, and non-admitted-base rejection.
- Fixed judgment and display: draft/static cap with
  `profile_not_admitted`, explicit base/source/hash presentation, and separate
  pack presentation.
- Recorded the E-02 decision in the mechanism ledger and added a focused
  documentation-drift test for the E-18 implementation contract.

## Scope boundary

Issue #105 is design-only. No production Rust, embedded profile manifest,
event schema, or `.anvil/` runtime state changed. Issue #117 (E-18) owns the
decoder, merger, GUI/CLI selection, and runtime behavior.

## Predecessor evidence

Inspected the committed work for Issues #107, #108, #109, and #116 before
editing. In particular, Issue #116 commit `ef0703f6` demonstrates the pack-only
organizational-convention path that this decision keeps as the default.
