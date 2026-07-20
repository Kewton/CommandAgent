# Issue 21 Verification

- Status: `passed`

## Checks

- `for mapping in docs/dev-guardrails.md:docs/dev/dev-guardrails.md docs/mechanism-ledger.md:docs/dev/mechanism-ledger.md docs/generality.md:docs/dev/generality.md docs/perf-notes.md:docs/dev/perf-notes.md docs/integration-notes.md:docs/dev/integration-notes.md docs/uat-corpus.md:docs/dev/uat-corpus.md docs/profile-manifest.md:docs/dev/profile-manifest.md docs/data-profile-contract.md:docs/dev/data-profile-contract.md docs/uat/scenarios.md:docs/dev/uat/scenarios.md docs/model-probe.md:docs/guide/model-probe.md; do old=${mapping%%:*}; new=${mapping#*:}; git show "b1e82b7:$old" | cmp -s - "$new" || exit 1; done`: `passed`
- `git diff --exit-code b1e82b7 -- docs/migration`: `passed`
- `for file in $(find docs -type f); do rel=${file#docs/}; rg -F -q "$rel" docs/README.md || exit 1; done`: `passed`
- `test -f docs/README.md && test -f docs/dev/data-profile-contract.en.md && test -f benchmarks/README.md && test -f docs/migration/anvil-commit-map.txt && test -f docs/migration/migration-report.md && test ! -e docs/data-profile-contract.md && test ! -e docs/model-probe.md`: `passed`
- `! rg -n 'docs/(dev-guardrails\.md|mechanism-ledger\.md|generality\.md|perf-notes\.md|integration-notes\.md|uat-corpus\.md|uat/|profile-manifest\.md|data-profile-contract\.md|model-probe\.md)' AGENTS.md README.md README.ja.md SECURITY.md src tests .github docs/*.md docs/guide`: `passed`
- `rg -n 'docs/' src tests .github *.md docs`: `passed`
- `git diff --check`: `passed`
- `cargo test manifest_drives_plan_guidance_requirements_and_repair_targets`: `passed`
- `python3 -m pytest tests/test_codex_orchestrate.py`: `passed`
- `ruff check tests/test_codex_orchestrate.py`: `passed`
- `cargo test --test data_profile_conformance`: `passed`
- `cargo test --test corpus_regression`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Notes

The required `rg` inventory was reviewed. Pre-move path text remains only where
the issue requires moved source documents to stay byte-identical, where the
unchanged migration evidence records its original path, or where tests use
unrelated synthetic `docs/` paths. The active source, guide, README, and corpus
references for relocated documents point to their new paths.

The historical-evidence exclusion was enforced: both
`docs/migration/anvil-commit-map.txt` and
`docs/migration/migration-report.md` remain unchanged at their original paths.

The inline documentation audit also confirmed that every file under `docs/` is
listed in `docs/README.md` and that local links in the changed entry-point files
resolve. The full Rust run completed with 1,521 unit tests passed and 15
intentionally gated tests ignored, followed by all integration and doc tests
passing.
