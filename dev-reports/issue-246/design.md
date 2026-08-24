# Issue 246 design: tolerant local pack listing

## Observed defect

`--packs` discovers every `<id>/<version>` directory under the legacy extension
root layout and the documented `packs/` layout, then applies strict pack
conformance to each candidate. One malformed pack or nested memo directory
therefore returns an error from the whole listing before any valid local packs
can be shown.

The GUI catalog already treats malformed pack contents as row-level warnings.
Issue 161's committed change keeps that behavior while fixing warning
deduplication and the repository-only builtin namespace. Issue 245's committed
change reserves co-located `profiles/` from legacy pack traversal. This change
will build on Issue 245's discovery rules and will not duplicate or alter Issue
161's separate GUI projection.

## Change

- Keep extension-root traversal and deterministic ordering unchanged.
- During `--packs` rendering, conform each discovered local candidate
  independently. If conformance fails, print a `warning:` to stderr naming the
  skipped directory and continue with the remaining candidates.
- Keep valid compatible packs in the existing tabular stdout format.
- Keep `--pack`, `--pack-verify`, `--pack-pin`, unreadable-root handling, and
  all pack conformance rules strict. Tolerance applies only to catalog listing.

## Regression coverage and verification

Add a CLI integration test with a pinned valid pack alongside both a malformed
pack and a memo-style nested directory. The test will require exit success,
the valid local row on stdout, and warnings identifying both skipped candidates
on stderr.

Run the focused integration test first, then the complete `pack_actions` test
target. Because shared Rust CLI behavior changes, also run formatting, Clippy
for all targets, the full Rust test suite, and `git diff --check`.
