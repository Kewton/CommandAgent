# Phase 21 Implementation Tasks

## 1. Rebaseline And Admission

- [ ] Read `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`.
- [ ] Read `workspace/mvp/logic/anvil/loadmap2/phase_20/coverage_closure.md`.
- [ ] Read
  `workspace/mvp/logic/anvil/loadmap2/phase_20/continuation_ledger.md`.
- [ ] Read
  `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`.
- [ ] Confirm Phase21 is scoped only to P20-COV-001 / C01-C12.
- [ ] Capture current commit hash, branch, and dirty flag.
- [ ] Create the row-level closure matrix before implementation starts.
- [ ] Create a Phase21 blocking ledger before implementation starts.
- [ ] Create a Phase21 reconciliation map before implementation starts.

Required output:

- [ ] `workspace/mvp/logic/anvil/loadmap2/phase_21/row_closure_matrix.md`
- [ ] `workspace/mvp/logic/anvil/loadmap2/phase_21/blocking_ledger.md`
- [ ] `workspace/mvp/logic/anvil/loadmap2/phase_21/reconciliation.md`

## 2. Row-Level Closure Matrix

For each row C01-C12, record:

- coverage id;
- source mechanism;
- current status;
- accepted Phase21 parity target;
- owner module/layer;
- missing contract fields;
- implementation target files;
- unit test proof;
- focused eval proof, if needed;
- broad sign-off dependency;
- final row disposition:
  - `closed_proven`;
  - `excluded_with_rationale`;
  - `split_forward`;
  - `open`.

Rules:

- [ ] Do not mark a row `closed_proven` without proof.
- [ ] Do not use `split_forward` unless the split row has owner, proof command,
  downstream phase, and closure condition.
- [ ] Do not update the main coverage table until row proof is complete.

## 3. Blocking Ledger And Reconciliation

Create a Phase21 ledger that splits P20-COV-001 into row-level blockers.

Required fields:

- blocker id;
- coverage id C01-C12;
- owner layer;
- failed or incomplete contract;
- suspected module;
- downstream implementation task;
- proof command;
- closure condition;
- status.

Create a reconciliation map:

```text
P20-COV-001
  -> Phase21 blocker
  -> C01-C12 coverage row
  -> implementation task
  -> proof command
  -> final sign-off rerun
```

Rules:

- [ ] Do not group multiple coverage rows into one blocker unless they share
  the same owner, missing contract, proof command, and closure condition.
- [ ] Do not start implementation for a row until its ledger and
  reconciliation entries exist.
- [ ] If a row is too broad, split it here first rather than during code work.

## 4. C01-C03 Task Contract And Behavior Projection

Rows:

- C01 Task contract core
- C02 Task contract inference and admission
- C03 Objective and behavior contract projection

Tasks:

- [ ] Identify the current `TaskContract` / plan input / profile projection
  types and tests.
- [ ] Define the MVP parity target for:
  - task kind / intent;
  - required artifacts;
  - behavior obligations;
  - expected completion evidence;
  - lifecycle or persistence boundary.
- [ ] Add or tighten typed fields only where the row proof requires them.
- [ ] Add tests proving the fields are rendered into plan prompts, repair
  evidence, or eval reports as appropriate.
- [ ] Add focused eval only if model-facing behavior changes.

Closure requirement:

- [ ] C01-C03 have row-level proof or are split forward with precise missing
  contracts.

## 5. C04-C06 Artifact Role, Scope, And Ownership

Rows:

- C04 Artifact role taxonomy
- C05 Task workspace scope
- C06 Artifact ownership

Tasks:

- [ ] Identify the common artifact role classifier and profile adapters.
- [ ] Identify workspace snapshot/scope ownership behavior.
- [ ] Define a single source of truth for role/scope/ownership facts consumed
  by target admission and completion evidence.
- [ ] Add tests for profile-neutral roles and at least Next.js, Rust, Python,
  docs, and data profile representative paths where the existing coverage row
  claims common parity.
- [ ] Ensure generated/cache/dependency paths remain excluded from ownership.

Closure requirement:

- [ ] C04-C06 have unit proof plus any needed focused fixture proof.

## 6. C07-C10 Ledger, Completion Evidence, Binding, Obligation Audit

Rows:

- C07 Artifact ledger
- C08 Completion evidence
- C09 Evidence binding
- C10 Deliverable obligation audit

Tasks:

- [ ] Verify tool records, verifier observations, scaffold/setup deltas, and
  workspace observations are reconciled into bounded evidence.
- [ ] Add missing completion evidence producers only where proof gaps are
  deterministic and shared.
- [ ] Add missing evidence binding producers for manifest identity, docs
  section, data schema, source citation, or route/import binding only if
  Phase21 proof requires them.
- [ ] Ensure deliverable freshness remains observable and does not become a
  hidden repair trigger.
- [ ] Project relevant fields into eval reports if they are part of row proof.

Closure requirement:

- [ ] C07-C10 have proof that a deliverable can be observed, bound, and used as
  completion authority, or the unproven producer is split forward.

## 7. C11-C12 Active Job And Dispatch Gate

Rows:

- C11 Active job arbiter
- C12 Recovery owner / dispatch gate

Tasks:

- [ ] Verify active-job candidates carry owner, job, action, target hint,
  artifact role, rerun authority, tool policy, and deterministic reason.
- [ ] Verify dispatch chooses exactly one owner/action or emits an explicit
  contract-conflict stop.
- [ ] Add lifecycle/attempt-progress fields only where needed to close C11/C12.
- [ ] Add tests for compatible candidate merge, competing owner conflict, and
  deterministic tie-break behavior.
- [ ] Add focused eval if owner/action selection behavior changes.

Closure requirement:

- [ ] C11-C12 have proof that recovery dispatch is a gate, not prompt-only
  guidance.

## 8. Focused Eval And Broad Sign-off

- [ ] Add focused eval fixtures for any model-facing behavior introduced in
  Phase21.
- [ ] Run targeted focused fixtures after implementation.
- [ ] Run broad sign-off using the established roots.
- [ ] If broad sign-off fails, map findings to row IDs and update
  `row_closure_matrix.md`.
- [ ] Do not close Phase21 with unowned sign-off findings.

## 9. Documentation And Coverage Updates

- [ ] Create
  `docs/eval/loadmap2-phase21-core-contract-ownership-<date>.md`.
- [ ] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only for
  rows that have proof.
- [ ] Update architecture/profile/evaluation docs if behavior or report schema
  changes.
- [ ] Create `implementation_report.md` with:
  - implemented rows;
  - split-forward rows;
  - excluded rows, if any;
  - proof commands;
  - final broad sign-off result.

Required output:

- [ ] `workspace/mvp/logic/anvil/loadmap2/phase_21/implementation_report.md`

## 10. Verification

Minimum:

- [ ] `cargo fmt --check`
- [ ] `cargo test`
- [ ] `python3 tests/test_eval_report.py`
- [ ] `python3 tests/test_eval_signoff.py`
- [ ] `bash scripts/eval_smoke.sh`
- [ ] final broad sign-off command
- [ ] `scripts/check_branding.sh`
- [ ] `git diff --check`

If report schema or eval scripts change:

- [ ] `python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py`
- [ ] relevant eval report/signoff unit tests

If focused eval fixtures are added:

- [ ] targeted fixture run
- [ ] focused report/recheck

## 11. Failure Handling

- [ ] If a row lacks proof, leave it `open` or `split_forward`.
- [ ] If a row has multiple owners, split it before implementation.
- [ ] If a proof command fails twice with the same finding, write a design
  review note and attach it to the row.
- [ ] If implementation introduces new sign-off findings, map them to
  coverage rows before closing Phase21.
- [ ] Do not treat CI success as row closure.

## 12. Phase21 Closure Review

Before closing Phase21, answer:

- [ ] Are C01-C12 all `closed_proven`, `excluded_with_rationale`, or
  `split_forward` with owner/proof/downstream phase?
- [ ] Does every changed coverage row name a proof source?
- [ ] Does every focused fixture pass or produce an owned blocker?
- [ ] Does final broad sign-off pass?
- [ ] Does the branding check pass after tracked doc updates?
- [ ] Does the implementation report state remaining blockers clearly?
- [ ] Did Phase21 avoid hidden retry, provider-specific behavior policy, and
  profile-as-workflow behavior?

## Execution Status

Phase21 execution completed the admission/reconciliation tasks:

- [x] Read the recovery plan, Phase20 coverage closure, continuation ledger,
  Phase20 final decision, and coverage table.
- [x] Confirmed Phase21 scope is limited to `P20-COV-001` / C01-C12.
- [x] Captured baseline commit, branch, and dirty flag.
- [x] Created `row_closure_matrix.md`.
- [x] Created `blocking_ledger.md`.
- [x] Created `reconciliation.md`.
- [x] Created `implementation_report.md`.
- [x] Created the tracked Phase21 eval report.
- [x] Updated the tracked coverage note without promoting unproven rows.

Closure answers:

| question | answer |
| --- | --- |
| Are C01-C12 all accounted for? | yes |
| Are implemented rows proven by tests/eval? | no row was promoted to implemented |
| Are split-forward rows row-level and owned? | yes |
| Does every changed coverage row name a proof source? | no status row was changed |
| Did Phase21 avoid hidden retry/provider-specific behavior policy/profile workflows? | yes |

Verification command results are recorded in `implementation_report.md`; all
required local checks passed.

## Review Result Reflected

- Narrowed Phase21 to P20-COV-001 instead of trying to close all Phase20
  blockers.
- Required row-level proof for C01-C12 before coverage status changes.
- Added `split_forward` as a controlled outcome with owner/proof/downstream
  phase requirements.
- Added explicit Phase21 ledger and reconciliation tasks before implementation.
- Required broad sign-off after any implementation.
- Required branding check for tracked documentation updates.
- Kept optional focused eval scoped to model-facing behavior only.
