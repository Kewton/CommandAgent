# Issue #252 design: extension-root inventory

## Scope and current behavior

Extension diagnostics are split across `--packs`, `--pack-verify`, and
`--doctor`. The existing profile loaders and pack selection path intentionally
fail fast, which is correct for execution but prevents an operator from seeing
all malformed, staged, or otherwise unusable entries in one pass. The GUI has
its own catalog projection, but the product CLI has no equivalent read-only
action.

This branch starts from the completed Issue #230 commit so its `--allow`
parsing and scoped policy installation remain intact. Issues #251 and #231/#151
were also inspected; neither changes this action's owned CLI or extension
inventory surface. The merged Issue #228 plan-YAML actions and Issue #249/#250
draft-profile, local-pack, and declarative-check contracts remain outside the
new projection.

## Design

1. Add `--extensions` as an exclusive offline CLI action. It accepts an
   explicit `--extension-root`, otherwise resolves the existing top-level
   `extension_root` setting relative to `--cwd`. The existing `--json` flag is
   shared by `--doctor` and `--extensions`, and remains invalid without either
   owning action.
2. Add a leaf `extension_inventory` module. It enumerates only the established
   `profiles/<id>/{manifest,overlay}.toml` and `packs/<id>/<version>`
   namespaces, validates each entry independently, and returns sorted,
   serializable rows. One malformed entry never suppresses another row.
3. Profile rows expose id, kind, relative path, exact-byte hash when valid,
   draft status, overlay base, validation/usability, and an explicit reason.
   Pack rows expose identity, local source, lifecycle/pin state, conformance,
   compatible profile and intent when decodable, exact-byte hashes, usability,
   and an explicit reason. Missing pins and closed-vocabulary decode failures
   remain honest unusable states; no admission or declarative-check boundary is
   weakened.
4. Project the final non-empty `journal.jsonl` record without changing it. Read
   only a bounded tail, reject symlinks/non-files and oversized or malformed
   final records, and report absence distinctly.
5. Render a compact tab-separated text view with one physical line per entry,
   or the same report as pretty JSON. After preserving the independent profile
   rows, attempt the existing process-local catalog registration so conformant
   draft-profile packs can be decoded and any root-wide catalog error is also
   visible. The action performs no persistent mutation, pinning, supply,
   execution, provider startup, event emission, or runtime namespace mutation.

## Tests and compatibility

- Add a focused corpus extension root containing a malformed manifest, a valid
  overlay, an unpinned conformant pack, a pack with an unregistered source, and
  a journal record.
- Add CLI integration coverage for text rows, structured JSON, configured-root
  fallback, and action conflicts/help.
- Preserve all existing action fields and schemas; only `--json`'s owner set is
  widened from doctor alone to doctor or extensions.

## Verification

Run the focused CLI/library tests first, then the corpus and documentation
drift checks. Because this changes shared CLI behavior, finish with formatting,
Clippy across all targets, and the full Rust test suite.
