# Issue 44 implementation summary

## Outcome

The two documented PTY launch paths now pass `--include-ignored` through Cargo
to libtest. Running the `test-pty` recipe executes all three PTY integration
tests in this branch and reports `3 passed` instead of silently succeeding with
zero executed tests.

## Changes

- Updated `justfile` so `just test-pty` combines the existing environment opt-in
  with libtest's ignored-test opt-in.
- Updated the raw command in `CONTRIBUTING.md` to match the recipe and documented
  that `#[ignore]` remains intentional because the suite requires a Unix-like
  pseudo-terminal and should stay out of ordinary portable test runs.
- Added a focused doc-drift test that derives the command from the `test-pty`
  recipe, requires `--include-ignored`, and requires an exact matching command
  in `CONTRIBUTING.md`.
- Added an Unreleased changelog entry for the repaired contributor workflow.

## Scope notes

No production behavior, PTY test body, event schema, corpus fixture, release
workflow, or runtime namespace changed. The guard does not hard-code the
current count of three tests, so Issue 43's committed fourth ignored PTY test
will also be selected when that predecessor is integrated.

The optional platform CI expansion was not added; this change stays focused on
the broken documented local launchers and prevents those two launch paths from
drifting again.
