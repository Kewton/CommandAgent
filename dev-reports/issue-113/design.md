# Issue #113 Design

## Goal

Replace the legacy read-only asset pack cards with a read-only **Extensions**
catalog. The catalog must show repository and configured extension-root packs
with an explicit supply source, preserve the reviewed/unreviewed distinction,
surface an exact-byte hash/pin mismatch on the affected row, and hand an
eligible pack to Trial as an explicit preselection.

## Predecessor integration

The dispatched worktree starts at `origin/develop`, so the required predecessor
commits are not already present. Integrate their committed changes before the
Issue #113 implementation, preserving their commits and resolving overlap in
favor of the later GUI Trial refactor/pack-selection contract and GUI setup
preflight. In particular:

- Issue #112 supplies the accumulated core catalog, CLI/REPL pack selection,
  hardened Trial delegation, refactored Trial UI, and admitted-pack Trial API.
- Issues #104 and #105 supply the authoritative pack-supply and profile-overlay
  documentation contracts.
- Issue #116 supplies the Next.js material-aware pack implementation and its
  repository pack fixture.
- Issue #121 supplies GUI `--extension-root`, root separation, and setup
  preflight behavior.

## API shape and discovery

Keep `GET /api/packs` read-only and evolve each row to contain:

- `id`, `version`, `profile`, `intent`, and repository/extension-relative
  display path;
- `source` and the contract-defined Japanese `source_label`;
- the pin as `expected_hash`, the recomputed exact-byte hash as
  `observed_hash`, and `hash_matches_pin`;
- assist/eval presence plus `retired`, `shadowing_repository`, and a bounded
  warning string when the row cannot be selected safely.

Enumerate only the bounded `<root>/packs/<id>/<version>` layout. Load pack
identity and compute the observed hash through the shared strict pack loader.
Classify a repository row as `admitted` only when its complete compiled
admission tuple matches; otherwise classify it as `repository`. Classify every
extension-root row as `local`. A local row wins display resolution for the same
`id@version`, but carries the required shadowing warning. A missing or mismatched
pin remains visible and non-selectable rather than turning the whole endpoint
into a false success or hiding the discrepancy.

## GUI behavior

Promote the existing `/assets/` route and navigation label to **Extensions**
without changing the exported route, so existing base-path links remain
compatible. Render one accessible row per supplied pack with source, identity,
profile/intent, expected/observed hash, and an inline warning state. Keep the
contracts and measurement-suite tabs as secondary read-only catalog views.

Show `Trial で使う` only for rows whose pin matches, which are not retired, and
which correspond to a pack option Trial can validate. The link targets
`/try/?pack=<id>@<version>` through the shared base-path helper. Trial reads that
query once after pack options load and applies the option's registered profile
and selector together; an unknown value is ignored. Normal profile changes
continue to clear the pack.

## Tests and verification

- Extend GUI-server process tests with a temporary repository/extension root
  fixture that proves admitted repository classification, local source labels,
  local shadowing, and row-local hash mismatch reporting.
- Extend the read-only guard and browser smoke contract for the Extensions
  navigation, warning row, and base-path-safe Trial handoff/preselection.
- Run the focused GUI server/guard tests first, then formatting, clippy (default
  and GUI feature), the full Rust suite, GUI typecheck, lint, build, and smoke.

No mutation endpoint, approval boolean, runtime-state migration, event-schema
change, or historical evidence rewrite is introduced.
