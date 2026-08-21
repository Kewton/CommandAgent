# Issue #148 Design

## Scope

- Change only the CLI Gate 4 presentation. Keep the GUI result panel, sample
  goals, and demo assets deferred to Issue #153 as required by the approved
  Lane E decision.
- Preserve the Gate 3/Gate 4 decision, assurance classification, full
  acceptance sheet, typed action set, event names, and event schemas.
- Keep the already incorporated Issue #234 predecessor at the branch base.
  Issue #258 was inspected for conflict awareness but remains GUI-only and is
  not incorporated into this CLI lane.

## Presentation change

Prepend exactly three reader-facing bullet lines to Gate 4:

1. the command and acceptance gates whose recorded values passed;
2. checks that did not run, with `cli_probe_not_run` explained as the CLI
   C1-C4 behavior probe and the reason assurance remains `static`;
3. one most-effective next action. Static assurance prefers the already typed
   and available `elevated_model` action so the user can retry with a model
   more likely to reach the behavior probe, followed by a fresh Gate 1.

The summary reads only the generated acceptance-sheet fields and action
availability already supplied to `render_gate_four`. It does not recompute or
upgrade a verdict. The full acceptance sheet, Section 5, and complete typed
action list remain visible verbatim after the summary.

## Tests and verification

- Add focused presentation tests that pin the static-assurance three-line
  wording, ordering before the full sheet, and absence of false pass claims on
  a failed result.
- Run the focused Gate presentation tests first, then the completion-metadata
  CLI tests to freeze assurance decisions, corpus regression to freeze event
  contracts, formatting, Clippy, and the full Rust suite because shared CLI
  presentation code changes.
