# Loadmap2 Phase 1 Task Contract Projection

Date: 2026-06-22

## Scope

This change implements the first loadmap2 Phase 1 slice for Task Contract and
Behavior Obligation Authority.

Implemented boundaries:

- `TaskContract` projects task kind, required artifacts, behavior obligations,
  and artifact role facts from deterministic inputs.
- Profile obligations and deliverable-required artifacts are adapted into
  behavior obligations instead of remaining only prompt prose.
- Plan generation receives Task Contract facts for normal `/plan-run`.
- Ultra phase planning receives Task Contract facts through the phase workspace
  contract, while phase-local lint does not force every final artifact into
  each phase step plan.
- Plan lint can reject a dropped task required artifact and a missing
  setup/manifest owner for setup/manifest behavior obligations.
- Eval summaries can record `task_contract_kind`, `task_contract_status`,
  `behavior_obligation_codes`, `behavior_obligation_status`, and
  `artifact_role_projection_status`.

## Boundary Notes

Task Contract projection is data-only. It does not execute setup, grant retry
authority, choose hidden future work, or turn profiles into workflow engines.
Existing profile verification, setup bootstrap, recovery policy, and bounded
repair remain the owners of their own decisions.

## Verification

Local checks:

- `cargo fmt --check`
- `cargo test`

Broader release and GitHub Actions checks are tracked by the commit and CI run
for this change.
