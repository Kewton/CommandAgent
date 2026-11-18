# Maintainability boundaries

This document records the seams introduced by the 2026-08-07 maintainability
refactor. They are intended to keep orchestration code small without changing
the CLI, planner API, event schema, stop reasons, or runtime namespace.

## Planner phase lifecycle

`runner/phase/state.rs` owns the sixteen reviewed states.
`runner/phase/transition.rs` is pure and rejects illegal terminal transitions.
`runner/phase/effects.rs` is the only adapter used by the existing effectful
flow. StepPlan execution and phase-boundary cleanup live in leaf modules.

The transition layer must not perform provider, filesystem, process, browser,
or network I/O. Event ordering remains guarded by the two-phase lifecycle
fixture.

## Minimal loop

The loop's immutable policy/configuration is in `loop_run/context.rs`, mutable
recovery state is in `loop_run/state.rs`, structured errors are in
`loop_run/error.rs`, and normalized Bash execution effects are in
`loop_run/runtime_bash_effects.rs`. These modules may depend on leaf policy
modules but must not invent success or weaken verification.

## Event compatibility

`eval_events::emit(Option<&Path>, serde_json::Value)` remains the public,
backward-compatible boundary. Internal high-volume phase lifecycle producers
serialize the schemas in `eval_events/typed.rs`; equivalence tests freeze their
legacy JSON shape. New internal events should prefer a typed payload while
retaining schema version 1 unless a migration is explicitly authorized.

## Event file append atomicity

Within one process, an events file that is a regular file holds one event per
physical line: one JSON object followed by one LF, with no bytes of another
writer inside it (Issue #987). Every in-process writer of an events file
builds its record before it writes and then appends it through
`eval_events/append_writer.rs` under one process-wide lock:

- `emit` and `append_event_failsafe`, and through them the OS sandbox status
  events, provider worker threads, speculative phase workers, and the GUI
  server's own events;
- the outermost provider-turn flush, which writes the turn's event line and
  its spooled tail under a single acquisition of the lock;
- `workflow::orchestrator::emit`, which shares its file with sandbox events.

The lock is one for the whole process rather than one per path, so other
spellings of the same file (relative, `..`, a symlink) are serialized too.
Nothing under the lock emits, redacts, publishes, or touches the provider-turn
spool registry, so a flush cannot re-enter it. Redaction still happens before
an event is buffered, spooled, or written.

A failed write is reported as it was at develop `fc903248`: `emit` and the
provider-turn `Drop` print a warning, while `append_event_failsafe`,
`ExecutionTelemetry::finish`, and the workflow `emit` return the error. Bytes
already written stay in the file; the writer never truncates or rewrites it.
Line readers are unchanged: `commandagent runs <id> --events` still fails on a
malformed line, and the terminal report and completion snapshot still skip it.

Outside this guarantee: another process appending to the same file (the GUI
server hands its events path to the child run through
`COMMANDAGENT_EVAL_EVENTS`, and both write it), network filesystems, and
durability across a crash. No cross-process file lock is taken. An events
path whose opened handle is not a regular file (a FIFO, a device) is written
outside the lock with the calls of `fc903248`, a provider turn's tail through
a second append open included (classified again: a regular file by then takes
the lock), so a FIFO waiting for its reader never holds up other writers; its
lines get no guarantee. This holds when the handle can be
classified: a handle whose metadata cannot be read is treated as a regular
file and written under the lock, so a stalled write to such a non-regular file
can hold up other events files.

## Browser probe asset

The Playwright program is stored at
`src/minimal_loop/assets/interaction_probe.js` and embedded with `include_str!`.
Its SHA-256 regression test detects accidental byte drift independently from
Rust formatting.

## Verification entry point

Both GitHub Actions workflows and `just ci` execute `scripts/ci.sh`. Python
tool versions are pinned in `requirements/ci.txt`; intentional ignored Rust
tests and their opt-in paths are listed in `docs/dev/ci-ignored-tests.md`.
The Ruff rule set is explicit and isolated from machine-level configuration,
and CI validates only Git-tracked skills so an unrelated local skill draft
cannot make the repository gate nondeterministic.

The P2F settlement tests inject the immutable production-tree pin recorded by
the campaign when exercising settlement calculations. A separate negative
test proves that the production code still rejects a mismatched pin; no
historical evidence file is rewritten to accommodate current source changes.
