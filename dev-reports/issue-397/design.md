# Issue 397 design: investigate VerificationSpec shadow projection

## Scope

Integrate investigate-intent `VerificationSpec v0` proposals as a post-hoc
shadow over the existing I1/I2 evidence contract. Preserve
`adjudication::investigate::evaluate_investigation_evidence` as the only verdict
authority. Do not change `output/diagnosis.md`, investigation evidence schemas,
runtime execution, repair/scaffold behavior, final acceptance, events, or the
live `.anvil/` namespace.

## Design

- Add a leaf `verification_spec::investigate_shadow` module. It accepts the
  already-recorded investigation run and binding, their repository-relative
  artifact paths, and a parsed shadow generation. It calls the existing
  evaluator unchanged and copies that result into a versioned diagnostic
  report with an explicit `authoritative_verdict_changed=false` boundary.
- Project I1 from the authoritative reproducer record and one I2 row for every
  authoritative diagnosis binding claim. Each I2 row gets a deterministic,
  positional binding ID (`error_quote:N`, `file_line:N`, or `code_snippet:N`)
  without adding fields to `evidence/investigation-binding.json`.
- Cover an observed row only when exactly one provider claim preserves the
  artifact path, requirement ID, binding ID, diagnosis stage, reproducer
  lineage, epoch, observation kind, oracle strategy, polarity, and oracle
  artifact path. Missing, duplicate, stale, fabricated, or switched claims are
  explicitly unverified. The authoritative I1/I2 result remains unchanged.
- Separate model claims by role. Only `reproducer_observation` may correlate to
  I1 and only `diagnosis_binding` may correlate to I2. Investigate claims using
  behavior/state/negative/regression kinds are retained as causal hypotheses
  with `observed_fact=false` and `authoritative=false`, regardless of an LLM
  critic's lifecycle, result, or asserted strength. Hypotheses never cover an
  I1/I2 row.
- Do not extract or execute command candidates in the investigate projector.
  This prevents create/fix scaffold, repair, and final-acceptance behavior from
  entering the investigation path. Coverage is a shadow diagnostic only.

## Tests and fixtures

- Add focused conformance coverage for complete I1/I2 projection, stable error
  quote/file-line/code-snippet IDs, causal-hypothesis separation, critic
  non-promotion, fabricated and duplicate claims, claims-absent partial,
  reproducer defect, and baseline-not-reproduced behavior.
- Replay the existing `output/diagnosis.md` and evidence fixture shapes, add a
  dedicated investigate shadow proposal, and add an Issue corpus contract.
- Run focused investigate-shadow and investigation-negative tests first, then
  VerificationSpec fixture replay, corpus/guardrails, profile-crossing tests,
  formatting, Clippy, and the full Rust suite.

## Compatibility

This is additive and shadow-only. `VerificationSpec v0`, its JSON Schema and
prompt, I1/I2 semantics, `output/diagnosis.md`, investigation evidence readers
and fixtures, reproducer reconstruction, and terminal assurance projection
remain unchanged.
