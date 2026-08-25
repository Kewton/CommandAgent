# Fix VerificationSpec shadow coverage v0

`commandagent.verification_spec.fix_shadow_coverage.v0` is an additive,
post-hoc view of the fixed F1/F2/F3 contract in
[`docs/fix-intent-contract.md`](../fix-intent-contract.md). It does not revise
that contract and is never an assurance input.

## Authority boundary

The caller supplies the authoritative `FixEvidenceBundle` and the persisted
workspace-relative path for each observation. The shadow evaluator calls the
existing `evaluate_fix_evidence` function unchanged and copies its result into
the report. Provider-declared oracle lifecycle, result, and observed strength
are not execution evidence and cannot change that result.

Model proposals may carry structured reproducer or regression argv. A proposal
is exposed only after `VerificationSpec v0` bounds/path validation and the
registered declarative command policy both accept it. Every exposed candidate
has `execution_authorized=false`; the evaluator runs no command. A caller may
consider it only on an isolated workspace copy or after authoritative F1/F2/F3
evidence is final. This prevents shadow work from removing the defect before
F1, advancing the authoritative epoch, or changing the run-start frozen F3
set.

## Post-hoc correlation

The report projects F1, F2, and one row for every run-start frozen F3 binding.
A row is covered only when exactly one fix claim and its
`existing_fix_evidence` oracle preserve all of these fields:

- evidence artifact path;
- requirement ID and binding ID (the latter comes from authoritative evidence);
- stage and expected polarity;
- lineage and run-local epoch;
- oracle-to-claim and oracle-to-artifact binding.

Missing, duplicate, switched, stale, or substituted claims are unverified.
They do not replace the authoritative evidence row. Missing F1 with after-only
evidence, an initially successful F1, changed lineage, stale F2 epoch, and a
shrunken or modified F3 set continue to fail in `evaluate_fix_evidence` before
shadow coverage is considered.

## Assurance isolation

`all_required_claims_covered` describes only post-hoc claim correlation. It
cannot make `full`, and it cannot promote the generic/profile-unavailable
`partial` or `static` caps. Likewise, incomplete shadow coverage does not
rewrite an already-authoritative F1/F2/F3 verdict.
