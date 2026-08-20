# Issue 161 implementation summary

## Outcome

The GUI Extensions catalog no longer exposes the internal
`builtin@ingest-create` pseudo-row, and catalog warning sentences are emitted
once rather than being re-appended after source classification.

## Implementation

- Fast-forwarded the branch through verified predecessor Issue 160 before
  implementation, retaining its static-route and 404 behavior.
- Reserved the repository's `packs/builtin` directory from the public
  `<id>/<version>` catalog scan. Local extension-root IDs retain the existing
  discovery path.
- Deferred repository warning finalization until after admission
  classification. Local rows continue to finalize after shadow classification,
  so every resolved row now builds its warning text exactly once.
- Kept the existing `GET /api/packs` JSON shape, warning strings, source labels,
  hash/pin checks, and Trial eligibility semantics unchanged.

## Tests

- Added a focused `pack_catalog` unit test with a temporary repository
  containing the internal three-level builtin layout and a malformed public
  pack.
- The test proves that no `builtin` row is returned and that both malformed-pack
  warning sentences occur exactly once.
- Re-ran the GUI read-only capability guard, catalog API integration test, full
  GUI-server target, formatting, GUI-feature Clippy, and the full Rust suite.

No corpus fixture, screenshot, documentation copy, event schema, or `.anvil/`
runtime state changed because the defect and its observable output are fully
owned by the server-side catalog projection.
