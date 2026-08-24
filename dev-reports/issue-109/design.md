# Issue #109 Design: CLI and config pack selection

## Scope

Add pre-run pack selection for direct CLI actions without changing pack schemas,
runner event schemas, or the live `.anvil/` runtime namespace. Expose
`--pack`, `--pack-hash`, and `--extension-root`; accept `extension_root` at the
top config level and `pack` in presets; project the resolved identity in
headless summaries and doctor output. Selection errors are CLI usage errors and
must exit with status 2 before run-start emission or provider construction.

## Resolution and validation

- Put parsing, location, exact-byte validation, pin validation, profile/intent
  compatibility, and runtime-environment installation in a new leaf module.
- Require the selector form `id@MAJOR.MINOR.PATCH`. Reject an unpinned
  directory, a stale `pack.sha256`, an explicit `--pack-hash` mismatch, and a
  pack whose declared profile or intent differs from the resolved run.
- Resolve `extension_root` as CLI flag over top-level config. Search the
  extension root before the workspace repository for the same identity. Accept
  the canonical `<root>/<id>/<version>` layout, with `<root>/packs/...` as a
  compatibility layout; the repository uses `<workspace>/packs/...`.
- Resolve pack selection as CLI `--pack` over preset `pack`. When both are
  present they must name the same exact selector; otherwise fail as a
  contradictory request. `--pack-hash` requires a selected pack.
- Preserve the existing runtime pack contract by installing the four
  `COMMANDAGENT_PACK_*` values with a scoped, serialized guard around the run.
  This keeps pack behavior out of planner chokepoints and restores the prior
  process environment after the command.

## Output and diagnostics

- Extend `commandagent.headless-summary/v1` additively with an optional `pack`
  object containing `id`, `version`, `hash`, and `source` (`extension_root` or
  `repository`). Omitted selection keeps the field absent.
- Add a stable `pack.selection` doctor check whose details show the resolved
  identity/source, or report that no pack was selected. Invalid selection is
  represented as a failed doctor check while the JSON report is still emitted.
- Add a small public error-to-exit-code classifier so only pack usage failures
  use status 2; existing runtime failures keep status 1.

## Tests and documentation

Add focused unit tests for selector parsing, source precedence, missing/stale
pins, explicit hash mismatch, and profile mismatch. Add CLI integration tests
for exit status 2, preset-only activation, preset/flag contradiction, summary
projection, and doctor JSON. Update EN/JA CLI and configuration references plus
headless documentation together, and keep `tests/doc_drift.rs` aligned with the
new supported config keys.
