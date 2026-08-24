# Issue #110 Design: direct pack CLI actions

## Scope

Add three offline, pre-run actions to the `commandagent` binary on top of the
Issue #109 pack-selection surface: `--packs`, `--pack-verify <DIR>`, and
`--pack-pin <DIR>`. These actions must not construct a provider, start a run,
or mutate the live `.anvil/` namespace. Pack schemas, conformance rules, and
event schemas remain unchanged.

The implementation will first integrate the committed Issue #109 dependency
chain (#107 and #108), which supplies exact pack selection and the admitted
catalog. Other completed predecessors were inspected but are unrelated GUI,
setup, documentation, or Next.js runtime work and will not be copied into this
CLI-scoped branch.

## Command model and conflicts

- `--packs` is a flag. It resolves the requested `--profile` and `--intent`,
  lists every compatible admitted catalog entry in deterministic catalog order,
  then discovers compatible pack directories under `--extension-root` and
  labels those entries as local.
- `--pack-verify <DIR>` runs the same public `conform_directory` function used
  by the `pack_conformance` binary and prints the same pretty JSON report.
- `--pack-pin <DIR>` first runs conformance, then creates `pack.sha256` when it
  is absent. An existing identical pin succeeds without rewriting the file; an
  invalid or stale pin fails honestly and leaves it untouched.
- The three direct actions conflict with one another and with run/selection or
  artifact-generation actions that would make their meaning ambiguous.
  Context flags needed by listing (`--extension-root`, `--profile`, and
  `--intent`) remain allowed with `--packs`.

## Placement and output

Put behavior in a new leaf module, `src/pack_actions.rs`, with only early
dispatch wiring in `src/lib.rs` and argument definitions in `src/cli.rs`.
Listing output is a stable table-like text format containing selector, hash,
and source (`admitted` or `local`) for each entry. Local discovery accepts the
same `<root>/<id>/<version>` and `<root>/packs/<id>/<version>` layouts as pack
selection, validates candidates through strict conformance, filters by the
resolved profile/intent, and deduplicates identical directories.

Pin creation uses create-new file semantics so it cannot overwrite a pin
created concurrently. Verification and pin failures use the normal status 1;
Clap usage/conflict errors remain status 2.

## Tests and documentation

Add focused integration tests that compare `--pack-verify` with the
`pack_conformance` binary, cover pin creation/idempotence/tamper failure, and
exercise admitted-plus-local listing. Extend the existing `src/cli.rs` help and
conflict tests. Update the English and Japanese CLI references and first-loop
guide together, then run `tests/doc_drift.rs` plus formatting, Clippy, and the
full Rust suite because the shared CLI surface is touched.
