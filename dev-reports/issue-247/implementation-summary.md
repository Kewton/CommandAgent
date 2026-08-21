# Issue 247 / 248 implementation summary

Implemented the Epic #260 Lane I combined manifest change for Issues #247 and
#248 on top of the exact committed Issue #217 predecessor.

## Diagnostics

- Flattened external profile and overlay TOML failures into one located error
  at the filesystem boundary.
- Diagnostics now render one `path:line:column: reason` occurrence and do not
  expose the same TOML parser cause through nested source chains.
- Semantic manifest and measured-fixture vocabulary failures also receive a
  best matching source location; the doctor regression proves a duplicated
  table cause appears exactly once.

## Compact external manifest v2

- Added a closed external v2 decoder for common `metadata`, `plan`,
  `artifacts`, `guidance`, and `checks` sections.
- Made external status optional with an effective draft default. Existing v1
  manifests retain their parser, hash, runtime, warnings, and admission
  behavior; embedded manifests and overlays remain strict v1.
- Expanded v2 into the existing manifest-driven runtime using neutral shared
  defaults for profile identity, `{goal}`, step-template internals,
  vocabulary, and repair targets. No runner, event, or assurance contract was
  changed.
- Added a 16-line static-site v2 corpus fixture and recorded the resolved
  external-authoring gap in the format-gap ledger.

## Direct backends

- Made `--validate-manifest <path>` validate v1/v2 profiles and v1 overlays
  without registration or execution, including overlay base existence.
- Made `--init-profile <id> --extension-root <dir>` create a neutral 16-line
  v2 manifest with confined directories, private file creation on Unix, and
  create-new overwrite refusal.
- Added only minimal dispatch wiring in `src/lib.rs`; `src/cli.rs` and all
  built-in profile manifests are unchanged.

## Tests and documentation

- Added focused binary coverage for one-cause doctor output, exact file/line/
  column diagnostics, v1 compatibility, compact v2 doctor loading,
  validation, missing overlay bases, initialization, and overwrite refusal.
- Kept the existing Issue #117, manifest v1, doctor, documentation drift,
  corpus, and guardrail suites green.
- Updated the English/Japanese CLI references and the manifest contract to
  describe the operational backends and compact v2 shape.
