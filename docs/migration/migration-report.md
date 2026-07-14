# Repository Migration Report

Date: 2026-07-14

This report records the history-preserving migration from
`Kewton/Anvil@anvilminimal-migration-base` to the root of
`Kewton/CommandAgent`'s `develop` branch.

## Preconditions

- `git filter-repo` was installed with Homebrew and reported version
  `a40bce548d2c`.
- The frozen source tag resolved to
  `ec1519958c2210e3bcadcd19d7c23e51146a82ce`.
- A CommandAgent push dry-run succeeded.
- The temporary migration directory existed but was empty before the clone.
- Anvil was read only throughout M-1 through M-3.

## M-1: destination preparation

- CommandAgent `develop` was synchronized with `origin/develop`.
- The previous port attempt was preserved remotely as
  `archive/pre-migration-port` at
  `41adf7285bb887089d511e346b32629c4166fad6`.
- Commit `a31be80` cleared the tracked destination tree after the archive
  branch was pushed successfully.
- A pre-existing untracked `.env` was not read, modified, staged, or removed.

## M-2: filtered history import

- The source `develop` HEAD and frozen tag both resolved to
  `ec1519958c2210e3bcadcd19d7c23e51146a82ce` before filtering.
- `git filter-repo` selected two source path families: the former crate
  subtree and `workspace/management`. The crate subtree was renamed to the
  destination repository root.
- The first filter invocation stopped without changing history because the
  explicit `develop` checkout added a second HEAD reflog entry. The same
  command was rerun with `--force` only in the newly created disposable
  clone.
- The filtered `develop` HEAD and rewritten frozen tag both resolved to
  `ce51dbb5a52a129152d8d4074e4fabbe4ab219ac`.
- Root layout validation found `Cargo.toml`, `src/`, `tests/`, and
  `docs/`; `workspace/management/` was also retained.
- The filtered source history contains 376 commits.
- The filter commit map contains 2,323 lines. The source HEAD mapping is:

  `ec1519958c2210e3bcadcd19d7c23e51146a82ce -> ce51dbb5a52a129152d8d4074e4fabbe4ab219ac`

- The filtered clone passed `cargo test --quiet`.
- Commit `1e40996` merged the filtered history without squash by using an
  unrelated-histories merge.

## M-3: repository integration

- Commit `dfd0e70` rewrote live paths for the repository-root layout.
- `workspace/management/runs/` remained frozen. Its Git tree object before
  and after the sweep is
  `0ffa9410f0929148404c327191eddc8df65c8abb`.
- Commit `4e80622` added
  `docs/migration/anvil-commit-map.txt` and the migration note in the
  mechanism ledger.
- Anvil's `.github/workflows/ci.yml` contained the product-specific
  `mvp-anvilminimal-guardrails` job. Commit `f4fc006` migrated that job to
  CommandAgent root paths and added `cargo test --all-targets`.

## Final verification

| Check | Result |
|---|---|
| `cargo test` | PASS: library 1,264 passed / 13 ignored; all integration and doc tests passed |
| `cargo test --all-targets` | PASS: all compiled targets and integration tests passed |
| CI Python golden tests | PASS: 14 tests |
| `cargo build --release` | PASS |
| `target/release/anvilminimal --version` | PASS: `anvilminimal 0.1.0 f4fc006+dirty 2026-07-14T03:01:02Z` |
| Root layout | PASS: required crate directories and management archive present |
| Filtered commit count | 376 |
| Commit-map line count | 2,323 |
| Frozen run archive tree | unchanged at `0ffa9410f0929148404c327191eddc8df65c8abb` |

The `+dirty` version suffix is caused solely by the preserved, pre-existing
untracked `.env`; no migration file was uncommitted when the binary was
built.

## Legacy-path scan

The requested extension-filtered grep was run after excluding
`workspace/management/`. It produced no output and exited with status 1,
which is grep's no-match result. A separate all-text scan excluding only the
frozen `workspace/management/runs/` archive also found no matches.

Legacy paths that remain inside the frozen run archive are intentional
historical evidence and were not edited.
