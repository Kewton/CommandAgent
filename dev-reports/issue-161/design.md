# Issue 161 design: hide the internal builtin tree and finalize warnings once

## Observed behavior

The GUI catalog assumes every directory below `packs/` is a public pack ID and
every directory one level below that is its version. The internal reviewed pack
uses the distinct layout `packs/builtin/ingest-create/1.0.0`, so discovery
misreads it as the invalid public selector `builtin@ingest-create` and emits a
broken row.

`inspect` also finalizes its warning text before repository classification or
local-shadow classification. Those paths finalize the row again. Because the
first call has already joined multiple warning reasons into one string, the
second call cannot deduplicate the individual reason it appends, producing
repeated text.

## Change

- Exclude the repository-only `packs/builtin` namespace from public GUI catalog
  discovery. Continue treating a local extension pack whose actual ID is
  `builtin` like any other supplied local pack.
- Build warning text only after repository admission and local shadowing have
  been resolved, so each row is finalized exactly once.
- Add focused unit coverage using a temporary repository. Assert that the
  builtin namespace yields no row and that a malformed public pack receives
  each warning sentence exactly once.

## Compatibility and verification

The JSON schema, warning vocabulary, pack loader, builtin floor, and live
`.anvil/` namespace remain unchanged. Issue 160's passed predecessor commit is
carried forward because it updates the same GUI integration-test target. Run
the focused catalog unit tests first, then the GUI-server target, formatting,
Clippy with GUI targets, and the full Rust suite because shared server behavior
is touched.
