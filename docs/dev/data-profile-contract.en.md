# Data Profile Contract (v0, frozen before B-2 implementation)

> This file is a reference translation of the
> [Japanese canonical contract](data-profile-contract.md). If the two texts
> differ, the Japanese original governs. The original remains frozen and
> unchanged.

Status: frozen (2026-07-13). Any change to this contract is an explicit
contract revision and must be recorded in the mechanism ledger.

## 1. Scope

Task family: one-shot pipelines that read tabular data (CSV/TSV), clean it,
aggregate it, and generate a report. Inputs are files in the workspace. Outputs
are `pipeline/` (a rerunnable script), `output/results.json` (a
machine-readable form of every calculated value), and
`output/report.{html,md}` (a human-readable report).

## 2. Meaning of `full` (primary invariant)

`full` means that the pipeline is mechanically honest. It does not mean that
the analysis's insights are correct for the business. The latter is
permanently outside this profile's scope and is never claimed at any assurance
tier.

## 3. Required evidence (mandatory gates for `full`)

E1 — Reconciliation:
The number of input rows must equal the number of used rows plus the number of
excluded rows. Exclusions must be counted by reason and recorded in
`output/results.json`. No row may be dropped silently.

E2 — Claims binding:
Every numeric claim in the report body (numbers, percentages, and changes) must
mechanically match its corresponding value in `results.json`. The report may
only interpolate calculated results; numeric values must not be written
directly into its body. Any numeric value that cannot be reconciled is a
`claims_binding_violation` and fails the gate.

E3 — Reproducibility:
Rerunning `pipeline/` must produce values identical to `results.json`. The run
must be deterministic: its seed is fixed and its results do not depend on the
time. Any nondeterministic element fails the gate.

E4 — Schema assertions:
Every declarative check bound in the manifest's `[checks]` section, including
type, range, duplicate-key, and date-boundary checks, must pass.

## 4. Assurance tiers

- `full`: E1–E4 all pass through executed probes.
- `partial`: the pipeline runs and E1/E3 pass, but E2 or E4 is not satisfied.
- `static`: the script was generated but execution probes are incomplete; only
  syntax was verified.
- `failed`: execution failed, E1 was violated by silently dropping rows, or
  reproducibility was violated.

## 5. Execution probes

Run in isolation with networking disabled, a workspace boundary, and a bounded
timeout. Capture stdout, stderr, and artifacts; evaluate E1–E4; and record the
results in `evidence/*.json` (`reconciliation.json`, `claims-binding.json`, and
`rerun-consistency.json`). A probe adjudicates only the artifacts and does not
inspect how they were generated.

## 6. Resistance to false conformance (negative-test requirements)

- An artifact that writes a number absent from `results.json` into the report
  must fail E2.
- An artifact that does not account for excluded rows must fail E1.
- An artifact that depends on randomness or time must fail E3.
- An unexecuted probe must not project `full` (earned-assurance inheritance).

## 7. Explicitly out of scope

Correctness of insights or interpretations; appropriateness of statistical
methods; visualization quality; and semantic correctness of multi-file joins.
Version 0 assumes one input file. Joins are extended by adding scenario
families.

## 8. Constraints on generation (contract-derived guidance)

The pipeline must be deterministic: use a fixed seed, do not depend on
execution time, and keep iteration order stable. Manifest plan and guidance
text embeds this requirement as class knowledge derived from the contract.
