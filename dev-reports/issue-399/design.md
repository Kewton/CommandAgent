# Issue 399 design: Phase 6 A/B UAT and go/no-go

## Scope

Add a reproducible, evaluation-only Phase 6 decision layer over the Phase 0–5
shadow contracts. This change does not authorize a provider run, promote
`VerificationSpec` into the acceptance path, alter events, or change `.anvil`
state. The checked-in decision must remain `INSUFFICIENT-EVIDENCE` wherever a
candidate sample or an approved live result is absent.

## Design

- Add a versioned matrix manifest under `eval/goal_verify/v0/`. It identifies
  the baseline and candidate, freezes the intent/profile/language/size cells,
  and separates four evidence lanes: blind review, CI, offline/local replay,
  and explicitly approved live runs. Each lane carries its own status and raw
  evidence references; an unapproved live lane cannot be treated as evidence.
- Add a leaf Python aggregator plus a thin CLI. The aggregator validates the
  closed manifest schema, resolves repository-relative evidence references,
  reads the frozen Phase 0 baseline, and emits a deterministic decision report
  and failure-case list into a new/empty run directory.
- Report every registered go/no-go indicator with `baseline`, `candidate`,
  `delta`, paired-bootstrap 95% CI, threshold, and verdict. Missing candidate
  observations, underpowered matrix cells, incomplete lanes, or absent
  resource-budget registration fail closed as `INSUFFICIENT-EVIDENCE` rather
  than being pooled or inferred from conformance tests.
- Keep rollback and flag-off evidence explicit. The report verifies referenced
  evidence paths and records that shadow modules are additive/opt-in and that
  the authoritative acceptance path remains unchanged. Missing rollback or
  flag-off evidence blocks GO.
- Add focused Python tests for deterministic output, lane separation, exact
  indicator shape, path validation, fail-closed live authorization, and
  NO-GO precedence. Add a corpus fixture contract for the checked-in matrix.

## Decision rules

`NO-GO` takes precedence when a measured candidate breaches a frozen safety
threshold or reports authority leakage. Otherwise, `GO` requires all evidence
lanes required by the manifest, every target cell at its registered sample
size, all indicators passing, and rollback/flag-off evidence present. Any
remaining gap yields `INSUFFICIENT-EVIDENCE`.

## Verification plan

Run the focused Python tests, generate the checked-in Phase 6 report twice and
compare it byte-for-byte, run Ruff for changed Python, run the corpus contract,
then run formatting, Clippy, and the full Rust suite because the report binds
shared Phase 0–5 contracts even though production Rust is unchanged.
