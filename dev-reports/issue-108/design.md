# Issue #108 Design: core pack catalog and supply identity

## Scope

Move the reviewed admitted-pack registry and its resolution rules from the TUI
boundary into `planner::pack::catalog`. Introduce the pack supply identity used
by a frozen `PackSelection`, and project that identity in the Gate 1 card and
acceptance sheet without changing the `no pack` presentation. Keep the pack
schema, conformance floor, runtime environment selection, event schemas,
institution contract, and historical evidence unchanged.

Issue #107 is a required predecessor but its verified commit is not present in
this branch. Fast-forward that commit first so the new catalog consumes the
central profile identity constants instead of reintroducing profile literals.

## Catalog boundary

- Add `src/planner/pack/catalog.rs` as the owner of `PackSource`,
  `AdmittedPack`, the admitted registry, compatibility enumeration, exact-tuple
  admission checks, and `PackLocator` path/hash resolution.
- `PackLocator` receives repository-root state at runtime. It resolves only
  reviewed admitted entries in this issue; repository and local supply remain
  explicit enum identities for later supply work and are not silently admitted.
- Keep `src/tui/boundary_shell/pack_catalog.rs` as a small adapter from
  `PackSelection` to the planner catalog. Move the exact-repository-byte catalog
  test to the planner module.

## Selection compatibility and presentation

- Add `source: PackSource` to `PackSelection::Pinned`, serialized in snake case
  and defaulted to `admitted` when older records omit it.
- Preserve old schema-v1 confirmation hashes by retaining the legacy hash
  projection for the default admitted source while new records still persist
  the explicit source. Repository and local sources remain explicit in their
  card hashes, so changing supply class cannot reuse an admitted confirmation.
- Render the Japanese supply label for pinned packs in Gate 1 and the acceptance
  sheet. Leave all `PackSelection::None` lines byte-for-byte unchanged.
- Replace the production `env!("CARGO_MANIFEST_DIR")` use in `repl.rs` with a
  `PackLocator` rooted at the configured workspace. GUI callers use their
  existing repository root.

## Tests and verification

Add planner catalog tests for enumeration, exact admission, path/hash
resolution, and repository-byte hashes; confirmation tests for a real legacy
record; and presentation/sheet assertions for supply labels and unchanged
no-pack output. Run the catalog, boundary confirmation/presentation/sheet, pack
runtime, and planner pack tests first, then formatting, Clippy, and the full
Rust suite because confirmation serialization and shared planner APIs change.
