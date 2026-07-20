# Issue 21 Implementation Summary

## Outcome

The documentation now has a discoverable audience-oriented index, end-user
guidance lives under `docs/guide/`, and the selected contributor/internal
documents live under `docs/dev/`. The frozen Japanese data-profile contract has
a separate English reference translation, and the benchmark fixture now has a
README explaining its runner contract.

## Implemented changes

- Fast-forwarded this worktree to the completed Issue 20 branch, which already
  contains the Issue 19 bilingual README and the Issue 20 bilingual user guide.
- Used `git mv` for the non-migration file set requested by the issue. Git
  reports every move at 100% similarity, so none of the moved source documents
  were rewritten.
- Moved `model-probe.md` to `docs/guide/model-probe.md` and updated both guide
  languages to link to it.
- Added `docs/README.md` with a complete file/description/language/audience
  index, a historical-drift warning, and an explicit note that the mixed
  mechanism and integration ledgers remain untranslated.
- Added `docs/dev/data-profile-contract.en.md`. It links to the byte-identical
  Japanese canonical contract, identifies itself as a non-authoritative
  reference translation, and is paired with the canonical file in the docs
  index.
- Added `benchmarks/README.md` describing `minimal-loop-expanded.yaml`, its
  current use by `scripts/bench.sh`, and `--model`, `--runs`,
  `--max-iterations`, `--recheck-root`, and `--bench-no-debug`.
- Added the docs-index link to both root README languages and updated live
  documentation-path references in repository instructions, source comments,
  data-profile guidance, active design docs, orchestration tests, and focused
  corpus fixtures.
- Added a focused Rust assertion that the embedded data-profile guidance names
  `docs/dev/data-profile-contract.md`.

## Historical evidence exclusion

Per the repository guardrail correction,
`docs/migration/anvil-commit-map.txt` and
`docs/migration/migration-report.md` were restored before verification and
remain at their original tracked paths. Neither file was moved, renamed,
deleted, or rewritten. A direct Git diff and byte comparison against the Issue
20 predecessor commit (`b1e82b7`) both confirmed that the migration directory
is unchanged.

## Compatibility

No runtime event names, schemas, verification gates, or `.anvil/` paths changed.
The runtime-visible data guidance changed only its documentation path, and the
matching corpus text was updated without weakening any expected outcome.
