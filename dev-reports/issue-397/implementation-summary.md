# Issue 397 implementation summary

## Outcome

Added an investigate-intent VerificationSpec shadow projector while preserving
the existing I1/I2 evaluator as the sole verdict authority.

## Changes

- Added `verification_spec::investigate_shadow`, a leaf module that:
  - copies the unchanged result of `evaluate_investigation_evidence` into a
    versioned shadow coverage report;
  - projects the authoritative I1 reproducer observation and every I2 error
    quote, file/line, and code-snippet claim;
  - derives stable shadow-only binding IDs (`error_quote:N`, `file_line:N`, and
    `code_snippet:N`);
  - requires exact artifact path, requirement, binding, stage, lineage, epoch,
    claim kind, oracle strategy, polarity, and oracle path correlation;
  - retains behavior/state/negative/regression claims as non-authoritative
    causal hypotheses, even if a critic marks an oracle executed and passing;
  - authorizes no candidate execution and has no runtime, scaffold, repair, or
    acceptance wiring.
- Added a complete investigate shadow proposal fixture plus focused conformance
  tests for full projection, hypothesis separation, critic non-promotion,
  fabricated evidence, duplicate claims, claims-absent partial, reproducer
  defects, passing baselines, and observation-kind substitution.
- Added an Issue corpus contract covering authority separation, stable I2 IDs,
  negative outcomes, and create/fix path isolation.
- Documented the additive Phase 4 projection in the frozen VerificationSpec v0
  and investigation intent contracts.

## Compatibility

No fields or meanings changed in `VerificationSpec v0`,
`output/diagnosis.md`, `InvestigationRunEvidence`,
`InvestigationBindingEvidence`, evidence envelopes, events, adjudication,
terminal projection, or `.anvil/`. Existing create and fix shadow suites,
investigation fixtures/readers, adjudication byte compatibility, and the full
Rust suite remain green.
