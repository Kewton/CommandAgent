# Issue #147 design

## Problem

The interactive REPL already treats a plain-text request as a Gate 1 proposal
and dispatches only after `/confirm <hash>`, but its entry surfaces do not teach
that sequence. The startup banner names only discovery commands, `/help` omits
`/confirm`, and the D-3c guard says that confirmation is required without
explaining how to obtain or confirm a card. The README quickstart also chooses
the one-shot `--prompt` path, so a new reader can miss the REPL flow entirely.

## Design

- Add `/confirm <hash>` to the shared slash-command registry as a boundary-only
  command. The REPL continues to own confirmation and dispatch; calling the
  generic slash handler directly must fail closed.
- Put stable first-run and Gate 1 guard copy in the REPL output leaf module.
  Wire the startup banner and the pre-execution guard to those strings so both
  direct users and tests observe one explicit sequence: type a plain-text
  request, review Gate 1, then enter `/confirm <hash>`.
- Change the bilingual README quickstart to start the REPL and show the request
  and confirmation as distinct inputs. Update the bilingual slash-command
  tables, command counts, guide index, and tutorial excerpts to match runtime.
- Pin the exact banner and D-3c copy in focused unit tests and the existing PTY
  smoke path. Extend doc-drift coverage so the README first loop and documented
  registry counts cannot silently regress.

## Non-goals and safety

This change does not alter Gate 1 identity, confirmation persistence, dispatch,
events, acceptance evidence, or `.anvil/` state. It only improves discovery and
guidance while preserving the existing fail-closed execution boundary.

## Verification

Run the focused TUI/library tests, the PTY smoke, and `cargo test --test
doc_drift` first. Because shared Rust CLI/help code changes, also run formatting,
Clippy for all targets, and the full Rust test suite.
