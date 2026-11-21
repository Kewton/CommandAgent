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
provider-turn `Drop` print a warning (each time), while
`append_event_failsafe`, `ExecutionTelemetry::finish`, and the workflow `emit`
return the error. Bytes already written stay in the file; the writer never
truncates, repairs, or rewrites it. Short writes and `Interrupted` are retried.

A failure that leaves a line open in a regular file (the last byte written is
not its LF) marks that file: its handle is held, keyed by the device and inode
read from the handle opened for the write (never from the path), and every
later append to it in this process fails without writing, under any spelling
of its path. A new file created at the same path is a different inode and is
written. Both opens of a provider turn's flush go through this one check. The
mark lives only as long as the process; another process, or the next run,
does not see it. Without an inode identity (not Unix) every events file
shares one key, so a mark refuses every events file in the process. A handle
whose identity cannot be read gets nothing written. A panic while the lock is
held makes every later locked append fail.

A provider turn's tail is copied by its recorded length. A read error or an
early end of the spool (`UnexpectedEof`) while it is copied fails the flush;
the bytes copied until then stay in the file, and when they end inside a line
the file is marked. This differs from a spool write that fails: its bytes are
never committed, so no flush copies them. A nested turn whose merge into its
parent fails is lost whole while the parent keeps its lines and its event.

Line readers read per line. `eval_events/event_reader.rs` splits a file into
physical lines by its LF bytes and only then checks each line for UTF-8 and
JSON, so a line cut inside a multi-byte character no longer hides the whole
lines around it:

- `commandagent runs <id> --events` is strict: it fails on the first line that
  is not one JSON value, and the error names the file and that line's number.
- The terminal report, the completion snapshot, and the lenient run inventory
  skip only the broken line and keep the valid lines before and after it, in
  file order. A valid line before the cut keeps its values, including a
  recorded terminal `run_stop`; a value that appears only inside the broken
  line is never read as one.

Every entrance keeps its display masking, so a skipped line cannot surface a
protected value and a kept line is still scrubbed. Before Issue #1059 the whole
file was decoded as one UTF-8 string first, so a single cut hid every line,
including the whole lines before it (`runs` listed the stop reason as
`unknown`); the strict path still fails on a broken line, and the lenient ones
still skip it — malformed input is not loosened.

Outside this guarantee: another process appending to the same file (the GUI
server hands its events path to the child run through
`COMMANDAGENT_EVAL_EVENTS`, and both write it), network filesystems, and
durability across a crash. No cross-process file lock is taken. An events
path whose opened handle is not a regular file (a FIFO, a device) is written
outside the lock with the calls of `fc903248`, a provider turn's tail through
a second append open included (classified again: a regular file by then is
written as above), so a FIFO waiting for its reader never holds up other
writers; its lines get no guarantee and a failure there marks nothing. A
handle from either open whose metadata cannot be read is refused.

The values the summary's projection treats as closed — the assurance level,
release-gate status, stop reason, and next action vocabularies — are fixed
identifiers (`sensitive_data/fixed_identifiers.rs`). A credential or plain
value equal to one, or a fragment of one, is refused at registration, so a
registered secret can never collide with a value the acceptance-unverified
decision compares. A closed value is therefore displayed as-is, and the
decision and the display read the same value; only the free text the
projection does not classify (a stop reason, a verification command, a body)
is masked.

The acceptance-unverified decision reads the same body the summary shows: the
previous body (masked, not capped) joined with the new text and then passed
through the write limit. There is no second path that reads the raw argument
or a previously rendered line, so the result cannot disagree with the
displayed status and next action.

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
