# Phase 19: Large Ownership And Evidence Recovery

Date: 2026-06-23 JST

## Purpose

Phase19 closes the large-eval ownership and evidence blockers left after
Phase18. The goal is not to make every large task pass. The goal is to ensure
that every failed large row is owned, actionable, and explainable from eval
artifacts alone.

Phase19 is complete only when the Phase17 large ledger rows assigned to
Phase19 are either:

- `closed_proven`; or
- `blocked_external` with explicit owner, action, evidence semantics, and a
  recorded provider/model/environment rationale.

`blocked_external` is not allowed for missing owner/action/target/evidence.

## Inputs

- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`
- `docs/eval/loadmap2-phase18-focused-recovery-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- latest broad sign-off command using:
  - Phase16 smoke root;
  - Phase18 focused root;
  - Phase16 focused-fixture root;
  - Phase16 large root or a new Phase19 large root.

## Scope

Phase19 owns the large rows only:

| Ledger row | Current finding | Required Phase19 outcome |
| --- | --- | --- |
| P17-L001 | Large provider/eval timeouts are visible as `provider_transport:eval_timeout`, but still leave ownership/evidence fields missing. | Timeout rows have provider/eval owner, explicit stop/blocker action, attempt outcome, and field-sensitive not-applicable evidence semantics. |
| P17-L002 | `large-nextjs-app-modify` profile dependency/version conflict falls back to `source_implementation_repair`. | Profile dependency/version conflict selects manifest/setup owner, `package.json` target, and setup/manifest action. |
| P17-L003 | Failed large rows miss `evidence_binding_status` and `completion_evidence_status`. | Failed large rows report meaningful evidence binding/completion evidence status or field-sensitive not-applicable reason. |
| P17-L004 | Target-applicable failed large rows miss target path/role. | Target-applicable rows report selected/admitted target path and role; truly targetless rows explain why target is not applicable. |

## Non-goals

- Do not weaken broad sign-off gates.
- Do not call Phase19 "migration complete"; Phase20 owns that decision.
- Do not add hidden retry loops or unbounded large reruns.
- Do not classify missing evidence as model quality.
- Do not put shared failure ownership policy into provider transports.
- Do not require large eval success when the failure is a provider/model
  throughput limit with complete owner/action/evidence.

## Layer Ownership

Phase19 is deliberately split by responsible layer:

| Layer | Responsibility |
| --- | --- |
| Eval observation/reporting | Preserve timeout/profile failure source-of-truth and project runtime job fields for large rows. |
| Sign-off checker | Treat `not_applicable` as acceptable only when field, terminal state, owner, action, and attempt outcome prove it is not hiding missing evidence. |
| Provider/eval boundary | Represent eval timeout as provider/eval boundary evidence, not source repair. |
| Profile/recovery mapping | Route deterministic profile dependency/version failures to manifest/setup repair. |
| Active job arbitration | Prefer manifest/setup/profile owner over generic source when diagnostic code provides a better owner. |
| Target admission | Select target path/role when a diagnostic or verifier command identifies a repairable artifact. |
| Completion evidence | Emit failed/not-applicable completion evidence status for failed rows instead of blank/unknown fields. |

## Design Alignment

This phase follows the repository design principles:

- deterministic evidence over semantic guessing;
- bounded and observable recovery;
- common contracts before profile-specific fixes;
- provider-independent behavior outside transport;
- no hidden continuation;
- eval scripts and docs treated as product code.

The main architectural rule is field-sensitive projection:

```text
deterministic failure
  -> owner/action/target/evidence projection
  -> sign-off interpretation
  -> proof rerun
  -> closed_proven or explicit blocked_external
```

`not_applicable` is allowed only when the row has enough deterministic context
to prove the field is truly not applicable. For example, target can be
not-applicable for provider timeout, but evidence binding cannot be missing for
a profile contract failure that has a repairable manifest target.

## Implementation Direction

### P17-L001: Timeout Ownership

Add a large-row timeout projection that produces:

- `active_job=provider_transport_blocker` or equivalent provider/eval boundary
  job;
- `recovery_owner=provider_transport` or `eval_boundary`;
- `selected_action` / `repair_action` describing explicit bounded stop;
- `attempt_outcome=blocked_external` or equivalent non-success outcome;
- `evidence_binding_status=not_applicable` only if the sign-off checker can
  verify the provider/eval timeout context;
- `completion_evidence_status=not_applicable` only under the same context.

### P17-L002: Profile Failure Mapping

For profile diagnostics such as
`profile_verification:nextjs_dependency_version_conflict`, route to:

- active job: `manifest_repair` or setup/profile equivalent;
- owner: `setup` or `manifest`;
- target: `package.json`;
- target role: `setup_manifest`;
- repair action: dependency/version manifest correction or explicit setup
  blocker when setup execution is not allowed.

This must be implemented in common profile/recovery mapping where possible,
with Next.js-specific diagnostic classification only in the profile adapter or
profile diagnostic table.

### P17-L003: Evidence Field Completion

Failed large rows must not leave evidence fields blank or `unknown`.

Allowed states should be explicit and field-sensitive:

- `passed`;
- `failed`;
- `missing`;
- `not_applicable:<reason>` or a structured equivalent;
- `blocked_external:<reason>` for provider/eval limits.

The sign-off checker must reject `not_applicable` unless the row's terminal
state and owner/action justify it.

### P17-L004: Target Projection

Target-applicable rows must include:

- target path;
- target role;
- target source of truth;
- target admission status.

Provider timeouts may be targetless. Profile, verifier, setup, dependency,
route, and source failures are target-applicable unless a deterministic
not-applicable reason exists.

## Horizontal Rollout

The change must apply across large families:

- Next.js new and modify;
- FastAPI/Python new and modify;
- Rust new and modify.

The rollout is common-contract first:

1. shared eval/report projection;
2. shared sign-off missing-field interpretation;
3. shared active job/target/evidence fields;
4. profile-specific mapping only for diagnostic-to-target facts.

No new profile workflow should be added unless a profile emits a deterministic
diagnostic that cannot be represented by the common contract.

## Documentation Updates

Update docs when implementation changes behavior:

- `docs/evaluation.md`: large sign-off field semantics and
  field-sensitive not-applicable rules.
- `docs/architecture.md`: provider/eval boundary and large-row ownership
  projection if runtime/event semantics change.
- `docs/known-limitations.md`: only if provider/model throughput remains a
  Phase19 accepted external blocker.
- `docs/eval/loadmap2-phase19-large-recovery-<date>.md`: final Phase19 report.
- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`: close or
  explicitly block P17-L001 through P17-L004.

## Proof Strategy

Use fixtures first, then live large eval:

1. Unit tests for timeout/evidence/target/profile projection.
2. Report/sign-off fixture tests for field-sensitive missing interpretation.
3. Targeted large Next.js modify rerun or synthetic profile failure fixture for
   P17-L002/P17-L004.
4. Full large eval with a practical release timeout.
5. Broad sign-off rerun with smoke, focused, focused-fixture, and large roots.

## Exit Gate

Phase19 is complete only when:

- P17-L001 through P17-L004 are `closed_proven` or valid `blocked_external`;
- broad sign-off has no unowned large failure;
- broad sign-off has no generic source fallback when a better owner exists;
- broad sign-off has no missing evidence binding/completion evidence for
  failed large rows;
- broad sign-off has no missing target for target-applicable rows;
- final Phase19 report records proof roots and remaining Phase20 work.

## Completion Result

Phase19 proof is recorded in:

```text
docs/eval/loadmap2-phase19-large-recovery-20260623.md
```

The proof used the existing full large time-boxed root because it already
contained all Phase19 blockers P17-L001 through P17-L004. Rechecking that root
after the projection/sign-off changes produced a broad sign-off `status: pass`
with:

- provider/eval timeout rows owned by `provider_transport_blocker`;
- `large-nextjs-app-modify` owned by `manifest_repair` with target
  `package.json`;
- no `generic_source_fallback`;
- no missing evidence binding/completion evidence;
- no missing target for target-applicable rows.

Phase19 does not declare migration complete. Phase20 remains responsible for
the final migration-complete decision.

## Plan Review Result Reflected

The initial plan risk was to treat all large failures as one eval-reporting
issue. This document instead splits the work by row and layer:

- timeout ownership is separate from evidence field completion;
- profile dependency/version mapping is separate from target projection;
- sign-off interpretation is field-sensitive and does not globally accept
  `not_applicable`;
- Phase19 does not claim migration completion.
