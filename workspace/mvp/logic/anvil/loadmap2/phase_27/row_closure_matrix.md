# Phase27 Row Closure Matrix

Date: 2026-06-23 JST

| coverage id | current status | adoption | owner layer | missing contract | target modules | required proof | closure condition | disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C21 | Implemented | Adopt | target admission / recovery orchestration | Target admission lacks broad proof across route/source/test/docs/setup/evidence-binding and rejection cases. | `target_admission.rs`, `artifact_graph.rs`, `artifact_ownership.rs`, `artifact_ledger.rs`, eval scripts | target-admission tests, focused target matrix, broad sign-off | Selected repair targets are admitted or rejected with role, ownership, scope, source-of-truth, freshness, excerpt, and exhaustion evidence. | closed_proven |
| C22 | Implemented | Adopt | target priority / semantic repair context | Target prioritization is not fully proven by failure kind, authority, role, and progress history. | `target_admission.rs`, `semantic_failure.rs`, `repair_job.rs`, eval scripts | target-priority tests, focused prioritization fixture, broad sign-off | Priority is deterministic, authority-aware, and stops on ambiguous same-priority admitted targets. | closed_proven |
| C23 | Implemented | Adopt | repair job lifecycle / verifier rerun | Repair job lifecycle and verifier rerun transitions are incomplete. | `repair_job.rs`, `runtime/repair_loop.rs`, `recovery_task.rs`, eval scripts | repair-job tests, focused lifecycle fixture, broad sign-off | Repair job state records lifecycle stage, verifier rerun outcome, pass/fail/no-progress, and safe-stop report. | closed_proven |
| C24 | Implemented | Adopt | attempt ledger / repair loop | Attempt outcomes are not broadly proven across profile families. | `repair_job.rs`, `runtime/repair_loop.rs`, `artifact_ledger.rs`, eval scripts | attempt-ledger tests, eval report tests, focused attempt matrix, broad sign-off | Each attempt records target, role, cluster, before/after signatures, changed files, and outcome. | closed_proven |
| C25 | Implemented | Adopt | no-progress strategy / repair loop | No-progress strategy branches need focused proof; contract-conflict branch must defer to Phase28. | `repair_job.rs`, `target_admission.rs`, `recovery_orchestration.rs`, eval scripts | no-progress tests, focused no-progress matrix, broad sign-off | No-progress selects bounded switch/stop/defer strategy without increasing retry budgets; C33 branch is Phase28-owned. | closed_proven |
| C26 | Implemented | Adopt | verifier diagnostic / semantic failure | Language-specific diagnostics and weak target filters are incomplete. | `verifier_diagnostic.rs`, `semantic_failure.rs`, `recovery_contract.rs`, eval scripts | verifier-diagnostic tests, focused verifier fixture, broad sign-off | Diagnostics expose failure kind, source excerpt, observed/expected, affected cases, candidate artifacts, weak reason, and unknown counts. | closed_proven |
| C27 | Implemented | Adopt | verifier orchestration / repair loop | Verifier rerun outcome events, attempt limits, binding scope, and safe-stop report are incomplete. | `verify.rs`, `runtime/repair_loop.rs`, `repair_job.rs`, `evidence_binding.rs`, eval scripts | verifier orchestration tests, focused verifier-rerun fixture, broad sign-off | Original verifier rerun is bounded, observable, scoped, and cannot be replaced by a weaker check. | closed_proven |
| C28 | Implemented | Adopt | verifier command policy / integrity | Generated-test, self-reference, unsupported assertion, and expectation audit checks are incomplete. | `verifier_selection.rs`, `integrity_guard.rs`, `plan_lint/verifiers.rs`, eval scripts | verifier-selection/integrity tests, focused verifier-policy fixture, broad sign-off | Weak or self-referential verifiers/tests are rejected before progress or completion evidence is claimed. | closed_proven |
| C29 | Implemented | Adopt | artifact completion / evidence authority | Artifact completion job is not fully bound to ledger, ownership, freshness, and missing-evidence distinction. | `artifact_completion.rs`, `evidence_authority.rs`, `deliverable_obligation.rs`, eval scripts | artifact-completion/evidence-authority tests, focused completion-job fixture, broad sign-off | Missing deliverables and missing/failed/stale evidence remain distinct and owned. | closed_proven |
| C30 | Implemented | Adopt | focused edit / target admission | Focused edit recovery lacks proof after target admission and stale-target rejection. | `target_admission.rs`, `artifact_ledger.rs`, `runtime/repair_loop.rs`, eval scripts | target-admission/ledger tests, focused edit fixture, broad sign-off | Focused edit requires current excerpt/read evidence and rejects stale changed-only targets. | closed_proven |
| C31 | Implemented | Adopt | mechanical fallback / patch admission | Forced small edit and deterministic fallback admission are incomplete. | `mechanical_repair.rs`, `repair_action_plan.rs`, `target_admission.rs`, eval scripts | mechanical-repair tests, patch admission fixture, broad sign-off | Mechanical fallback is bounded, target-admitted, verifier-owned, and cannot mutate without validation. | closed_proven |
| C32 | Implemented | Adopt | patch validation / repair loop | Patch validation, unsafe/noop/duplicate/test-weakening rejection, and rollback proof are incomplete. | `integrity_guard.rs`, `mechanical_repair.rs`, `runtime/repair_loop.rs`, `repair_job.rs`, eval scripts | patch-validation tests, focused patch fixture, broad sign-off | Patch outcomes are validated before progress, rollback admission is explicit, and unsafe mutations are rejected. | closed_proven |

## Closure Rules

- `closed_proven` requires row-specific unit or fixture proof plus focused
  proof where listed.
- `split_forward` is allowed only for a narrower same-surface blocker with
  failed proof evidence, owner, downstream phase, and closure condition.
- C25 contract-conflict resolution must not be closed in Phase27; only branch
  selection/deferral evidence can close Phase27's portion.
- Broad sign-off is regression evidence, not row proof.
- C21-C32 cannot be closed by docs alone, CI alone, field existence alone, or
  post-hoc eval derivation from reason text.

## Review Result

Review findings applied:

- Kept each coverage row independent so patch validation cannot hide target or
  verifier gaps.
- Required both selected repair and rejection/safe-stop proof.
- Made Phase28 contract-conflict dependency explicit.
- Required row-specific focused proof before any coverage status change.
