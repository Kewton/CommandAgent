# Issue 103 design: tracking-only extension roll-up

## Scope and authority

Issue 103 is a tracking-only roll-up. The approved decision forbids duplicating
child production implementation, so this branch will add only closure evidence
and the required worker reports. No Rust, GUI, event schema, pack/profile
schema, `.anvil/` namespace, historical run evidence, or user documentation
will change unless verification exposes a parent-level documentation gap.

## Inputs

- The current branch starts at `81c22d18`, where the already-merged child work
  includes CLI and GUI pack selection, the extension catalog and supply
  lifecycle, the Next.js convention pack, draft profile support, GUI setup
  preflight, and the reorganized profile foundation.
- Required predecessor #122 (`74794b25`) contains the reader-oriented
  documentation reorganization and passed evidence.
- Required predecessor #115 (`efc2f2d2`) is based on #122, contains the GUI
  stage/verify/pin wizard and passed evidence.
- Required predecessor #119 (`60ee8588`) is based on #115, contains the final
  GUI runtime/read-only visibility work and passed evidence.
- Required predecessor #123 (`5a149765`) is based on #119, contains the measured
  external draft-profile cell and passed evidence. It is therefore the single
  aggregate tip for testing all four required predecessor commits together.

## Verification design

1. Build an acceptance matrix that maps every parent criterion to committed
   child behavior, tests, and reports rather than restating the implementation.
2. Verify focused parent contracts at the aggregate #123 worktree: pack
   runtime/catalog behavior, GUI pack lifecycle and read-only contracts, draft
   profile behavior, Next.js conformance, setup preflight, documentation drift,
   and the #123 one-cell measurement.
3. Run the repository guardrails and full suites at that aggregate tip because
   the parent crosses Rust, CLI, GUI, documentation, and smoke contracts. Run
   the provider-free GUI smokes needed to cover root and reverse-proxy paths.
4. Run document/diff checks on this evidence-only #103 branch. Record exact
   commands and outcomes in `verification.md`; use `blocked` if any required
   command fails or cannot run.

The final commit will contain only `dev-reports/issue-103/` and will not merge,
copy, or reimplement predecessor production changes.
