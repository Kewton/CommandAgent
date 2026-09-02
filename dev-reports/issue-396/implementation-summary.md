# Issue 396 implementation summary

## Outcome

Added an additive fix-intent `VerificationSpec v0` shadow integration without
changing the F1/F2/F3 evaluator, runtime, profile bindings, event schema, or
terminal assurance projection.

## Changes

- Added `verification_spec::fix_shadow`, which:
  - calls the existing `evaluate_fix_evidence` function for the authoritative
    result and copies that result unchanged into the shadow report;
  - correlates post-hoc `evidence/fix-*.json` observations with exactly one
    VerificationSpec claim by artifact path, requirement ID, stage, expected
    polarity, lineage, epoch, claim/oracle binding, and existing-evidence
    oracle strategy;
  - projects F1, F2, and every run-start frozen F3 binding independently, with
    explicit unverified reasons for missing or duplicate evidence/claims;
  - surfaces bounded structured F1 reproducer and F3 regression proposals only
    as candidates with `execution_authorized=false` and proposed (not
    authoritative) lineage/epoch;
  - performs no provider call, workspace mutation, candidate execution, event
    emission, or verdict mutation.
- Added a complete fix shadow fixture and focused conformance tests covering
  field preservation, candidate/authority separation, before/after switching,
  after-only evidence, stale epoch, frozen regression shrink/change, provider
  execution claims, and partial/static non-promotion.
- Tightened `tests/fix_intent_conformance.rs` with explicit before/after
  requirement substitution and after-only negatives.
- Added the Issue 396 corpus contract and developer documentation for the
  post-hoc and isolated-execution boundary.

## Compatibility

The existing `VerificationSpec v0` schema/prompt and create shadow behavior are
unchanged. `src/planner/fix_runtime.rs` and `src/planner/fix_reproducer.rs` were
not modified, preserving the one-rebuild reproducer-defect limit and the rule
that repair cannot begin until F1 is confirmed. Existing adjudication byte
fixtures remain identical, and unavailable generic/profile regressions remain
subject to the existing partial/static behavior.
