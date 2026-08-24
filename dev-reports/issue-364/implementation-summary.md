# Issue #364 implementation summary

## Outcome

GUI Trial terminal projection now treats the latest meaningful event snapshot
in the current directive round as authoritative. A successful Gate 3 keeps
passed probes as neutral `検証結果` and does not render `FAILED の原因`.
Gate 4 continues to expose its stop reason, release-gate reasons, failed probe
details, and evidence paths.

## Implementation

- Reworked `failure_diagnostics` projection to replace, rather than append,
  probe and release-gate snapshots. A later pass clears stale failure reasons,
  while trailing `not_applicable` values cannot overwrite a meaningful result.
- Scoped terminal details, verdicts, diagnostics, and session-index projection
  to events after the latest directive-continuation boundary. Final acceptance
  is selected from the latest non-neutral result, so `run_stop:not_applicable`
  cannot replace an earlier current-round pass.
- Split blocking diagnostics from successful verification results in the GUI.
  The Gate 3 terminal renders passed probes under `検証結果`; the Gate 4
  terminal retains the existing failure drill-down.
- Added a public GUI-server projection that replaces the absolute execution
  root with `<execution-root>` in Gate 1 identities/cards, session status,
  acceptance sheets, diagnostics, event tails, and readable session artifacts.
  Runtime readiness and generic workspace-conflict responses no longer return
  the configured path.
- Updated current GUI copy, smoke data, and user documentation to present
  `.commandagent/runs` as canonical. Existing `.anvil/runs` discovery remains
  the read-compatible fallback.

## Coverage and compatibility

- Added an Issue-specific corpus fixture with Gate 3 success, Gate 4 failure,
  bounded repair, trailing neutral values, and directive continuation.
- Added table-driven backend projection tests, verdict-slicing tests,
  execution-root redaction tests, and GUI-server integration assertions.
- Extended focused and full browser smoke coverage for Gate 3 and Gate 4 at
  both `/` and `/proxy/commandagent/`, including rendered execution-root
  privacy assertions.
- No event name or wire schema was changed. Historical evidence and the live
  `.anvil/` runtime namespace were not rewritten; only the existing legacy
  read path remains in use for compatibility.
