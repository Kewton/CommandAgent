# Pre-fix reproduction

Production HEAD was `3c814d9167975cc8b054a8b84a626f6cf84f82e4`.
Before production edits, the #474 diagnostic handoff test was changed to require
the desired actual error and attribute, and the fixture was extended to state.

Command: `cargo test --lib issue474_diagnostic_handoff -- --nocapture`

Observed exit: 101. The actual Node process failed and its raw stderr contained
`Error: missing primary`; those assertions passed. The next assertion failed:

```text
assertion failed: report.command_failures.iter().any(|failure|
        failure.reason.contains(actual_error))
test result: FAILED. 0 passed; 1 failed
```

This is the expected failing pre-fix control, not passing verification. The test
executes the saved original command and the product final-acceptance path; it
does not reproduce the summary logic in test code. The unchanged production
also retained the lexical state-first attribution documented by #474. The new
desired test initially stopped at missing reason evidence, before attribution.

The later report-only and CI-only predecessor commits do not change this
production baseline. Their incorporation is recorded in the design note.
