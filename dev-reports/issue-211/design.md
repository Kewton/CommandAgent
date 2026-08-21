# Issues #211 and #212 design

## Problem

- The REPL classifies `/resume` as an execution command before checking whether
  its recovery target exists, so an empty inventory produces Gate 1 guidance
  instead of explaining that there is nothing to resume.
- A non-TTY invocation with `--resume <session-id>` reaches the generic TTY
  error before checking whether the named saved minimal-loop session exists.
- `/runs` and `--runs` expose epoch seconds, ambiguous `?/?` phase counts, and
  unbounded stop detail. The live REPL run is indistinguishable from an old
  incomplete run.

## Constraints

- Keep D-3c Gate 1 mandatory for every valid REPL recovery execution.
- Preserve event names, event schemas, recovery selection, verification gates,
  and the live `.anvil/` layout.
- Limit production changes to `src/tui/repl.rs` and `src/runs.rs`.
- Keep every rendered inventory row within 100 terminal columns, including
  wide Unicode characters.

## Design

1. Before the REPL's Gate 1 execution-command check, parse only `/resume` and
   call the existing recovery preparation path. Render its error immediately
   when no recovery run/YAML exists; otherwise continue to the unchanged Gate 1
   requirement.
2. Before returning the non-TTY REPL error, ask `SessionStore` to load an
   explicitly named `--resume` session. Add context that identifies it as a
   missing or unloadable resumable saved session. A valid session still reaches
   the existing requirement to provide `--prompt` or another action.
3. Add a session-aware runs renderer used by the interactive REPL. It compares
   the configured event path with inventory event paths and renders `(current)`
   in a dedicated session column. Keep the existing renderer as the non-TTY
   entry point, where no live run is created.
4. Render start time from the event file modification time as local
   `YYYY/MM/DD HH:MM`, render absent phase data as `-`, and reduce colon-coded
   stop detail to its leading category (for example `model_stagnation`).
5. Use fixed terminal-display-width cells and a bounded final STOP cell so
   headers and all rows are at most 100 columns.

## Verification plan

- Focused unit tests in the two owned source files for missing resume guidance,
  valid-recovery Gate 1 continuation, missing saved-session context, local time
  shape, current marker, phase fallback, concise stop category, Unicode width,
  and the 100-column limit.
- Run the focused `runs` and `tui::repl` unit-test selections first.
- Run formatting, Clippy across all targets, and the full Rust test suite
  because CLI-visible shared Rust behavior changes.
