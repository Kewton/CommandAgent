# Issue #221 design

## Scope and predecessor

Issue #221 owns only the exit/SIGINT consumer wiring in `src/lib.rs` and the
headless user contract in `docs/user/headless.md`. The approved decision
requires the additive summary JSON to be emitted on interruption when run
evidence exists, without changing terminal failure, acceptance, assurance, or
release-gate semantics.

Required predecessor Issue #227 is committed at `3a0b1f6e` with passed
verification but is not yet an ancestor of this branch. Its exact diff and
reports were inspected before this note. It adds the seven v1 terminal fields
and typed `Source` overrides for terminal status and exit code while explicitly
reserving `src/lib.rs` and `docs/user/headless.md` for Issue #221. Fast-forward
this worktree through that commit before implementing the consumer wiring.

## Design

1. Keep the existing normal `--summary-json` path: construct one
   `headless_summary::Source` after configuration/pack resolution, run the
   command, and render the JSON after terminal evidence has closed.
2. Also pass a clone of that source through the CLI panic boundary to the
   existing direct-command completion guard. Workflow child runs receive no
   source, so only the outer command owns the final JSON line.
3. In the SIGINT thread, preserve the established ordering and honest terminal
   projection: reap registered server children, emit the interrupted terminal
   event and summary, finish terminal notification state, then write the JSON.
   Render it with Issue #227's typed `Interrupted` and `130` overrides. Write
   only when the configured event path now names a real file, so an
   interruption before evidence exists does not invent a handoff.
4. Put the evidence check and single-line write in a small writer-taking helper
   in `src/lib.rs`. This makes the signal projection testable without sending a
   process-killing signal to the test harness. Flush after the line because the
   SIGINT path immediately calls `process::exit`.
5. Map an `anyhow` interruption that reaches `main` to process exit `130` as
   well. Retain exit `2` for pre-run pack/CLI rejection and exit `1` for other
   failures. This aligns cooperative and signal-driven interruption without
   weakening any terminal result.
6. Extend the headless guide with Issue #227's additive v1 fields and state
   explicitly when the SIGINT summary exists and how its `status`/`exit_code`
   fields relate to the process exit.

## Tests and verification

- Add focused unit coverage for interruption exit-code mapping, v1 JSON
  projection (`status = interrupted`, `exit_code = 130`), newline/flush
  behavior, and suppression when no evidence file exists.
- Run the focused library tests and headless integration/corpus checks first.
- Run formatting, Clippy with warnings denied, and the full Rust suite because
  shared CLI termination and summary behavior are touched.
