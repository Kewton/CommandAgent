# Issue 21 Design

## Goal

Separate end-user guidance from contributor and historical documentation,
provide one discoverable documentation index, add an English reference
translation of the frozen Japanese data-profile contract, and explain the
benchmark fixture and runner.

## Predecessor baseline

Issue 20 is based on Issue 19. Its committed tree contains the bilingual root
README from Issue 19 and the bilingual `docs/guide/` tree from Issue 20, so this
branch will fast-forward to Issue 20 before applying Issue 21 changes. This
keeps the guide and README work intact without recreating it.

## Changes

- Move the non-migration acceptance-criteria file set into `docs/dev/` with
  `git mv`, keeping every moved file byte-for-byte identical. Move
  `model-probe.md` into `docs/guide/` because it is linked from the end-user
  guide. Per the repository guardrail correction,
  `docs/migration/anvil-commit-map.txt` and
  `docs/migration/migration-report.md` remain at their original tracked paths;
  they are not moved, renamed, deleted, or rewritten.
- Add `docs/README.md` as the documentation map. It will list every document,
  its purpose, language, and audience, and warn that historical records can
  differ from the current implementation. The index will also identify
  Japanese/English data-contract counterparts and note that mixed-language
  ledgers remain intentionally untranslated.
- Add `docs/dev/data-profile-contract.en.md` as a faithful reference
  translation. It will link to the unchanged Japanese canonical contract and
  state that the Japanese text governs. Because the canonical source is frozen
  and must remain byte-identical, reverse navigation will be supplied by the
  documentation index rather than by editing that source file.
- Update live references outside the frozen moved documents, including
  repository instructions, Rust comments/guidance, focused corpus fixtures,
  the bilingual README pair, and the guide index. Historical prose inside the
  moved files remains unchanged even when it records an old path.
- Add `benchmarks/README.md` describing `minimal-loop-expanded.yaml` and the
  supported `scripts/bench.sh` options requested by the issue.

## Compatibility and tests

No runtime event name or schema changes. Contract-reference values unrelated
to the moved data-profile document remain unchanged. The only product-facing
string update is the data-profile documentation path embedded in generation
guidance, with matching corpus fixtures updated.

Verification will first check moves, content identity against the pre-move
commit, links, and focused data-profile tests. Because a production guidance
string and corpus fixtures are touched, the final checks will include formatting,
Clippy, and the full Rust test suite.
