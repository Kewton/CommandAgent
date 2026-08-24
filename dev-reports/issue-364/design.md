# Issue #364 design: project current terminal outcomes without stale failures

## Problem

GUI Trial currently treats every projected probe as a failure diagnostic. A
Gate 3 event stream can therefore contain only successful probes and still
render `FAILED の原因`. The backend projection also appends reasons across
events, so a failed probe followed by bounded repair and a successful probe
keeps the old failure text. Neutral `not_applicable` fields in the trailing
`run_stop` event can obscure the authoritative final-acceptance result, and the
detail endpoint looks up the latest verdict outside the current directive
round.

The GUI-facing API additionally serializes the confirmed identity and generated
acceptance sheet with the absolute execution root. Current writes already use
`.commandagent/runs`, with `.anvil/runs` retained as the read fallback, but
several GUI labels, smoke fixtures, and current user instructions still present
the legacy path as canonical.

## Design

- Keep the existing `failure_diagnostics` wire shape. Fold each probe into its
  latest meaningful snapshot instead of accumulating all historical reasons.
  A later passed result replaces a failed result and clears its reasons; a
  trailing `not_applicable` does not overwrite an already observed pass or
  failure. Preserve evidence paths for the current snapshot.
- Treat release-gate reasons as the current release-gate snapshot. A passed
  release gate clears older reasons, while a trailing neutral status does not
  erase an actionable failure. Treat `tui_command_stop` as the primary terminal
  outcome and let `run_stop` enrich it only with an actionable failure.
- In the GUI, distinguish blocking diagnostics from neutral probe results.
  Gate 3 may show passed probes under `検証結果`, but it must never show
  `FAILED の原因`; Gate 4 keeps stop, release-gate, probe, and evidence detail.
- Scope verdict, assurance reasons, stop reasons, next actions, and diagnostics
  to events after the latest directive-continuation boundary. Do not infer a
  current verdict from an earlier round.
- Add a GUI-server public projection that replaces the absolute execution root
  with `<execution-root>` in serialized identities, Gate 1 cards, and generated
  acceptance sheets. Runtime readiness and workspace-conflict responses expose
  state without returning the configured path.
- Keep canonical writes and legacy reads unchanged. Update visible GUI copy,
  smoke data, and current documentation to call `.commandagent/runs` canonical
  and describe `.anvil/runs` only as the legacy read-compatible location.

## Tests and verification

- Add a corpus fixture containing representative Gate 3 success, Gate 4
  failure, bounded repair, and directive-continuation event rounds.
- Add table-driven backend projection tests for success, failure, repair,
  trailing neutral values, and round slicing, plus integration assertions that
  no absolute execution-root bytes reach proposal/status/runtime API bodies.
- Extend the two-base-path GUI feedback smoke with separate Gate 3 and Gate 4
  terminal projections and assertions for neutral verification versus failure
  diagnostics.
- Run focused Rust and GUI checks first, then every Issue-required Rust, lint,
  typecheck, build, and focused/full two-base-path browser smoke command.
