# Design: Issue #226

## Scope and predecessor

- Fast-forward to the completed `feature/issue-255-229-232` predecessor before
  implementation. Its `.commandagent` canonical write namespace and legacy
  `.anvil` read compatibility are the frozen state/config/run contract.
- Add workspace exclusion in a new leaf module, consume the existing typed band
  catalog for duration guidance, refine the existing plan handoff wording, and
  keep `src/lib.rs`/footer changes to lifecycle and presentation wiring.
- Do not edit `src/runs.rs`, duplicate the GUI's Markdown parser, change event
  names or schemas, rewrite historical evidence, or grow planner/minimal-loop
  chokepoints.

## Decisions

1. Execution-capable top-level commands acquire `.commandagent/lock` before
   entering the panic boundary. The lock is an immediate, nonblocking OS file
   lock held for the full top-level run; read-only run inspection, model probe,
   and the UX demo remain available. Workflow children inherit the outer
   exclusion and do not reacquire it.
2. The lock file records a version, PID, and run ID. A crashed process releases
   the kernel lock automatically; the next owner overwrites stale metadata.
   Contention never waits indefinitely and reports the recorded owner without
   claiming that malformed metadata is trustworthy.
3. Duration estimates are frozen typed values beside their band identities,
   using the same formal rows currently selected by GUI Gate 1. The CLI does not
   parse GUI evidence. A missing formal sample remains visibly `unmeasured`;
   no duration is inferred from success rates or timeouts.
4. Gate 1 shows the catalog-backed mean before confirmation. Goal-bearing
   direct commands also print it before preflight when deterministic routing
   resolves to one catalog entry.
5. Footer token usage remains cumulative across provider replies, but renders
   prompt, generation, and total counts separately. Unknown components remain
   `n/a`; existing provider telemetry and summary tables remain authoritative.
6. Generated-plan guidance names optional editing first, validation as the next
   command, and execution only after successful validation. Validation success
   continues to print the executable next command.

## Verification

- Focused unit tests cover lock contention/reacquisition and metadata, catalog
  duration integrity and honest missing data, startup/Gate 1 estimates, token
  breakdown accumulation, and step/UltraPlan guidance.
- Because shared CLI lifecycle and TUI presentation change, run formatting,
  Clippy with warnings denied, and the full Rust test suite after focused tests.
