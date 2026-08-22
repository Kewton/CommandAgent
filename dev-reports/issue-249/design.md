# Issue #249 design

## Context

External profiles and local packs may share one extension root, but pack
identity currently uses the compiled-only `PackProfile` vocabulary. A pack
whose `pack.profile` is the exact ID of a registered external draft therefore
fails decoding before Gate 1, and the draft Gate 1 constructor separately
forces `PackSelection::None`.

The external-profile runtime already enforces the required promotion boundary:
external manifests are registered as `draft`, their terminal assurance is
capped at `static`, and the cap reason remains `profile_not_admitted`.

## Design

- Extend the typed pack-profile identity with a process-lifetime draft value.
  It is created only by resolving an exact ID from the already registered
  external-profile catalog; arbitrary YAML cannot register a profile or make
  an unknown ID valid.
- Make `profile_descriptor.rs` the single resolver for compiled pack profiles
  and registered external draft pack profiles. Keep compiled descriptor
  admission, band, and promotion data unchanged.
- Make `pack/catalog.rs` own the supply trust rule: a draft pack-profile
  identity is valid only from `PackSource::Local`. Repository and admitted
  supply can never acquire draft compatibility merely by matching a string.
  Existing exact-byte pins, conformance, local shadowing, retirement, and
  intent checks remain necessary.
- Route CLI listing/selection, runtime reload, and Gate 1 validation through
  that central identity and source rule. A draft Gate 1 without a locator
  remains pack-free; the locator-aware Gate 1 path may carry one validated
  local exact-byte selection. This avoids accepting unverified serialized
  pins.
- Keep assist/check vocabulary compiled and closed. Draft create packs have no
  invented compiled contract floor, while every declared injection source,
  point, parameter, check, and material continues through the existing strict
  schema, capability, scrub, and conformance gates.

## Verification strategy

- Add focused pack/profile tests for exact registered-draft decoding, unknown
  identity rejection, local-only source compatibility, and unchanged compiled
  profile behavior.
- Add a corpus case containing a draft external profile and an exact-byte
  pinned local pack, plus frozen Gate 1 and completed terminal evidence proving
  the local source and `static / profile_not_admitted` result.
- Extend the external-profile integration path to select the fixture pack,
  render the Gate 1 card, and generate a non-full terminal sheet.
- Run the focused pack, external-profile, GUI, and corpus checks first, then
  formatting, Clippy, and the full Rust suite because pack identity and Gate 1
  are shared contracts.
