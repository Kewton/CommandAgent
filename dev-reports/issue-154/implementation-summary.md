# Issue 154 implementation summary

## Outcome

The English and Japanese root READMEs now present an explicit three-layer CLI
route: getting started, detailed tutorial, then reference. Every layer links to
the next, so the language-matched reference is reachable in at most three
clicks. Introductory CLI surfaces use the runtime GUI/recording sample goal,
`Create a CLI --pattern filter command`; the separate ingest walkthrough is
identified as an intentionally profile-specific advanced example.

## Documentation fixes

- Added the learning route to the root READMEs, CLI entry page, bilingual guide
  index, tutorials, and documentation index.
- Added the CLI, ingest, and pack contracts to the documentation index.
- Restored missing Japanese `--workflow` and `--origin` reference rows and
  corrected the advertised public flag count from 51 to the implemented 54.
- Corrected model-probe, troubleshooting-anchor, contributor, contract,
  migration-map, corpus, and run-evidence paths found by the full scan.
- Preserved the historical 2026-07-10/11 task-track run IDs while replacing
  links to report files that are not present in this checkout with an explicit
  availability note. No run evidence or migration record was modified.

## Drift coverage

`tests/doc_drift.rs` now:

- scans maintained root documents plus all Markdown under `docs/` (excluding
  immutable migration evidence) and `packs/` for local target and fragment
  drift;
- derives GitHub-style anchors from all ATX heading levels, including Unicode,
  punctuation removal, fenced-code exclusion, and duplicate suffixes;
- fixes the README-to-entry-to-tutorial-to-reference click contract and the
  shared runtime sample goal;
- compares every EN/JA guide pair's heading and per-table row counts; and
- binds both language flag and slash-command tables and their advertised
  counts to the Clap/slash registries.

## Integration

The verified predecessor tips for Issues 147, 160/161, and 245/246 were merged
before implementation. This keeps the documentation and tests aligned with the
19 accepted slash-command names, GUI behavior, and extension discovery rules
that Issue 154 depends on.

## CI follow-up

The Issue 160 Rust 1.98 compatibility code commit `714017ca` was cherry-picked
as `c08d24aa`. The follow-up changes only
`src/bin/gui_server/session_files.rs`: it boxes the Axum `Response` error behind
`SessionFileError` and delegates `IntoResponse` back to the original response.
Consequently the response status, headers, JSON bytes, confined-path handling,
and symlink rejection remain unchanged. No lint allowance or Issue 160 report
file was imported.
