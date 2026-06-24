# Loadmap2 Phase25 Plan

Date: 2026-06-23 JST

## Objective

Phase25 closes the Phase21 split-forward rows C11-C12:

| coverage id | responsibility |
| --- | --- |
| C11 | Active-job arbitration lifecycle, attempt-progress transition inputs, deterministic tie-break, no-owner, and conflict stop behavior. |
| C12 | Recovery owner/action dispatch gate connected to candidate producers before repair prompt rendering. |

The goal is to turn the existing active-job and recovery-dispatch fields into a
single deterministic decision boundary:

```text
deterministic failure evidence
  -> candidate active jobs
  -> active-job arbitration
  -> one dispatch decision or explicit stop
  -> Recovery Task Contract input / verifier-owned setup / explicit stop
  -> minimal loop execution
```

Phase25 must prove that CommandAgent does not leave ownership selection to the
model during repair. The model can execute the selected bounded task, but it
must receive a clear owner, action, target hint, tool policy, rerun authority,
and disallowed actions when a deterministic failure has already been detected.

## Inputs

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_22/`
- `workspace/mvp/logic/anvil/loadmap2/phase_23/`
- `workspace/mvp/logic/anvil/loadmap2/phase_24/`
- existing modules under `src/agent/step_runner/`:
  - `active_job.rs`
  - `recovery_orchestration.rs`
  - `recovery_policy.rs`
  - `recovery_task.rs`
  - `recovery_contract.rs`
  - `repair_brief.rs`
  - `repair_action_plan.rs`
  - `target_admission.rs`
  - `profiles.rs`
  - `evidence.rs`
  - `runtime/repair_loop.rs`
- eval/report modules:
  - `scripts/eval_agent_slice.sh`
  - `scripts/eval_report.py`
  - `scripts/eval_runtime_job_report.py`
  - `scripts/eval_signoff.py`
  - `tests/test_eval_report.py`
  - focused cases under `eval/cases/focused/control-recovery/`

## Non-goals

- Do not implement the broader Recovery Task Contract, setup/profile semantic
  repair, repair brief expansion, or action-envelope lifecycle. Those are
  Phase26.
- Do not implement repair target prioritization, verifier orchestration,
  no-progress strategy switching, patch validation, or attempt ledger closure.
  Those are Phase27.
- Do not implement full contract-conflict resolution. C11 may stop on a
  deterministic conflict, but C33 conflict objects and source-of-truth
  resolution are Phase28.
- Do not add hidden repair loops, hidden continuation, retry expansion, or
  provider/model-specific dispatch behavior.
- Do not make profiles workflow engines. Profiles may emit typed candidate
  hints, but active-job arbitration owns final selection.
- Do not run dependency setup implicitly from normal repair. Setup may be a
  visible verifier-owned dispatch decision only when the existing policy allows
  it.

## Design Alignment

Phase25 follows the current CommandAgent contract stack:

```text
task/profile/ledger/evidence facts
  -> typed recovery candidate producers
  -> active-job arbitration
  -> dispatch decision
  -> recovery task rendering / setup runner / explicit stop
  -> eval-visible owner/action/target evidence
```

Layer ownership:

| layer | Phase25 responsibility |
| --- | --- |
| `active_job` | Own recovery owners, job lifecycle terms, deterministic priority/tie-break/no-owner/conflict-stop vocabulary. |
| `recovery_orchestration` | Own candidate collection, arbitration, selected dispatch decision, candidate list, dispatch reason, and rerun authority projection. |
| `recovery_policy` / profiles | Produce candidate hints from profile failures without selecting the final job. |
| `recovery_task` / `repair_brief` | Render the selected owner/action/target/tool policy into bounded repair instructions. |
| `runtime/repair_loop` | Consume dispatch decisions and execute only the selected bounded action or explicit stop. |
| eval/report scripts | Prove owner/action/dispatch/tie-break/candidate fields are visible and not inferred only from prose. |

This phase strengthens dispatch closure. It does not add a second execution
engine, workflow scheduler, sidecar controller, or hidden Anvil loop.

## Architecture Shape

Prefer one common dispatch boundary over profile-specific branching.

Expected implementation shape:

1. Inventory existing active-job fields and candidate producers.
2. Define a stable candidate model for owner/job/action/source/target/tool
   policy/rerun authority/lifecycle state.
3. Add deterministic arbitration rules for selected, no-owner, ambiguous tie,
   explicit stop, and conflict-stop states.
4. Ensure recovery prompt rendering consumes the selected dispatch decision
   rather than recomputing owner/action from free text.
5. Add focused fixtures that assert dispatch fields, not only final success.
6. Update coverage only when C11 and C12 have row-specific unit, focused, and
   sign-off proof.

This keeps the design stable: dispatch is a bounded contract decision with
observable evidence. It does not execute extra work or increase retries.

## Horizontal Rollout

Phase25 must cover common failure families, not only Next.js:

- setup/dependency readiness failures that should select `setup_bootstrap`;
- manifest/config failures that should select `manifest_repair`;
- route/import integration failures that should select
  `route_integration_repair`;
- source diagnostics that should select `source_implementation_repair`;
- docs literal failures that should select `documentation_repair`;
- evidence-binding failures that should select `evidence_binding_repair`;
- verifier contract failures that should select
  `verifier_contract_correction`;
- tool protocol failures that should select `tool_protocol_correction`;
- no-owner and competing-owner cases that should stop explicitly with
  structured evidence.

Horizontal rollout should extend shared candidate and dispatch contracts.
Profile-specific code may only produce typed facts or candidate hints.

## Documentation Updates

Runtime changes in Phase25 must update:

- `docs/architecture.md` if active-job or dispatch boundaries change.
- `docs/adr/0002-contract-recovery.md` if dispatch semantics or explicit-stop
  policy changes.
- `docs/evaluation.md` if eval fields or sign-off interpretation changes.
- `docs/profiles.md` if profile candidate-hint responsibilities change.
- `docs/ultra-plan-run.md` only if planner-facing recovery fields change.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after proof
  exists for C11-C12.
- a new `docs/eval/loadmap2-phase25-active-job-dispatch-*.md` report at
  implementation closure.

## Required Proof

Minimum proof before a row can be `closed_proven`:

| row | minimum proof |
| --- | --- |
| C11 | Unit tests proving active-job lifecycle states, candidate priority, deterministic tie-break, no-owner, explicit stop, and conflict-stop behavior. |
| C12 | Unit tests proving recovery owner/action dispatch selects exactly one owner/action or explicit stop before repair prompt rendering, and that profile/setup/verifier/evidence/tool candidates connect to the dispatch gate. |

Phase-level proof:

- `cargo fmt --check`
- targeted Rust tests for active job, recovery orchestration, recovery policy,
  recovery task, recovery contract, repair brief, target admission, and runtime
  dispatch consumption
- `python3 tests/test_eval_report.py`
- focused eval proof for owner/action/dispatch/tie-break/candidate fields
- broad sign-off rerun after behavior changes

## Exit Gate

Phase25 can close only when:

- C11 and C12 are each `closed_proven`, or a narrower same-surface split is
  created with failed proof evidence, owner, downstream phase, and closure
  condition.
- `source_alignment_matrix.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, and `reconciliation.md` are updated with final
  results.
- focused proof and broad sign-off results are recorded when behavior changes.
- coverage table status changes are made only for rows with proof.
- focused proof root is recorded from fixtures that explicitly prove C11-C12
  dispatch fields.

## Implementation Result

Phase25 is complete.

| item | result |
| --- | --- |
| C11 | `closed_proven`; coverage status promoted to `Implemented` |
| C12 | `closed_proven`; coverage status promoted to `Implemented` |
| focused root | `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110` |
| focused assertions | `passed: 10` |
| recheck assertions | `passed_recheck: 10` |
| broad sign-off | `status: pass` |

Implemented behavior:

- `active_job_lifecycle` is recorded in contract evidence, orchestration
  evidence, recovery task rendering, deterministic fixture output, and eval
  summaries.
- Active-job dispatch records selected, no-owner, ambiguous-tie,
  explicit-stop, and conflict-stop lifecycle states.
- Recovery task rendering consumes dispatch facts such as owner, job,
  dispatch reason, candidate jobs, and tie-break reason before bounded repair
  prompt rendering.
- Focused fixtures cover setup, manifest, route, source, docs,
  evidence-binding, verifier-contract, tool-protocol, no-owner, and
  ambiguous-tie dispatch paths.

No C11-C12 blocker is split forward from Phase25. Phase26 and later still own
deeper recovery task semantics, setup/profile mapping depth, target
prioritization, verifier orchestration, patch validation, and contract-conflict
resolution.

## Plan Review

Review findings applied:

- Kept Phase25 scoped to dispatch selection and prompt-input ownership, not
  Phase26 repair semantics or Phase27 target/verifier lifecycle behavior.
- Required proof that owner/action is selected before repair prompt rendering,
  because evidence fields alone can still leave the model to choose the repair
  job.
- Added no-owner, ambiguous tie, and conflict-stop cases so arbitration does
  not silently fall back to source repair.
- Required cross-family candidate coverage across setup, manifest, route,
  source, docs, evidence binding, verifier contract, and tool protocol.
- Preserved bounded behavior: no retry count increase, hidden continuation,
  provider/model branch, profile workflow engine, or implicit dependency setup
  is part of this plan.
