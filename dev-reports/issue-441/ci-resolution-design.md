# Issue #441 CI resolution design

Parent: `8e3f071ae2a8ad42f6eb2865b7837b806deed634`.
Read the root's immutable CI `34145194835` and acceptance `34145194892`
failure logs under `20260908-0151-issue441-pr`. Both identify the test's
unconditional 64-byte slice of a 49-byte command at
`tests/issue441_placeholder.rs:147`. Production evidence already uses
`min(64, original byte length)` correctly.

Reproduce the original test with `TMPDIR=/tmp`, then correct only its expected
prefix bound. Retain the actual workspace-cd normalization control and the
whole original command's SHA256 assertion. Add short, exactly 64-byte, and
over-64-byte evidence cases with commands independent of temporary path length.
Check recorded original byte length, exact prefix contents and length, and the
full-command hash, including distinct tails after a shared 64-byte prefix.
Retain the existing UTF-8 split-byte check. No production behavior, evidence
policy, ignores, or existing corpus contracts need to change.

Run focused Issue #441 tests with the short temporary directory, followed by
formatting, clippy, all-target Rust tests including corpus, and doctests. Add
separate CI resolution summary and verification reports while preserving the
original and review-resolution reports. Commit explicit task paths only; no
base synchronization, publication, or lifecycle operations.
