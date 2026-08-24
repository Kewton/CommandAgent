# Issue #220 design

## Scope and predecessor review

- The assigned reproduction is
  `--profile nextjs --intent fix --packs`, for which no compatible admitted or
  local pack exists.
- Required predecessor commit `e5b0bbca` was inspected and does not change
  `src/pack_actions.rs` or the pack-list rendering contract.
- Preserve exit status zero, the existing tabular stdout format whenever rows
  exist, invalid-local-pack warnings, and the same compatibility filtering.

## Design

1. Build the existing rendered list before printing it.
2. When it contains only the heading, suppress stdout and emit
   `no compatible packs for <profile> × <intent>` on stderr. Non-empty tables
   remain byte-compatible.
3. Add a focused process-level test for empty stdout, the actionable stderr
   notice, and successful exit status.

## Verification plan

- Run the focused Issue #218/#220 CLI integration test first.
- Run formatting, Clippy for all targets, and the full Rust test suite because
  shared CLI behavior is touched.
