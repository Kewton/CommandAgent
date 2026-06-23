# Phase28 Implementation Tasks

Date: 2026-06-23 JST

Status: completed / closed_proven

## Implementation Summary

Phase28 closed C33 without split-forward.

- Added `src/agent/step_runner/contract_conflict.rs`.
- Connected contract conflict decisions to existing recovery orchestration.
- Added C33 eval/report/expected-field support.
- Added six focused deterministic fixtures.
- Focused root:
  `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`
- Broad sign-off: pass.

Detailed proof and closure state are recorded in
`implementation_report.md`, `row_closure_matrix.md`, `blocking_ledger.md`,
and `reconciliation.md`.

## Phase Admission

- [ ] Confirm C33 remains `Missing / Adopt` in
  `docs/eval/legacy-control-stack-coverage-20260621.md`.
- [ ] Confirm KI-007 remains open in
  `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`.
- [ ] Confirm Phase27 closed C25 only as conflict-branch deferral, not C33
  resolution.
- [ ] Record unrelated dirty files before implementation and keep them out of
  Phase28 commits.

## Source Alignment

- [ ] Reconcile C33 against Anvil source files:
  - `contract_conflict_job.rs`
  - `spec_authority.rs`
  - `api_contract_expectation.rs`
- [ ] Identify adopted behavior:
  - conflict object;
  - source-of-truth decision;
  - spec/docs/API/test/source authority handling;
  - ambiguous-authority safe stop.
- [ ] Identify omitted behavior:
  - hidden confirmation;
  - advisory memory;
  - unbounded continuation;
  - provider/model-specific policy.
- [ ] Record the mapping in `source_alignment_matrix.md`.

## C33 Contract Object

- [ ] Add or extend a shared contract-conflict module.
- [ ] Represent conflict sides with:
  - side role;
  - artifact path;
  - source layer;
  - source of truth;
  - observed/expected pair;
  - affected case;
  - freshness/binding status when available.
- [ ] Add render/eval projection fields for conflict object details.
- [ ] Unit test conflict object creation from semantic failure payloads.

## C33 Source-of-Truth Decision

- [ ] Define deterministic authority categories:
  - user/request contract;
  - behavior obligation;
  - profile contract;
  - existing docs/API/schema;
  - pre-existing test;
  - generated test with binding;
  - verifier command;
  - implementation preservation constraint.
- [ ] Define authority outcomes:
  - `implementation_authoritative`;
  - `test_authoritative`;
  - `docs_or_api_authoritative`;
  - `verifier_contract_limited`;
  - `ambiguous_authority`;
  - `insufficient_evidence`.
- [ ] Keep authority side and repair target/action as separate fields.
- [ ] Unit test precedence and equal-authority ambiguity.
- [ ] Ensure generated tests without binding cannot override user/profile/docs
  contracts.

## C33 Recovery Orchestration

- [ ] Add a `contract_conflict` candidate when semantic or no-progress evidence
  identifies an implementation/test/docs/API conflict.
- [ ] Project selected conflict action into existing recovery action/action
  envelope fields.
- [ ] Route repairable conflicts to existing action kinds:
  - source repair when a stronger test/docs/API/user/profile authority proves
    implementation is the non-authoritative side;
  - test alignment repair when the test/verifier artifact is
    non-authoritative;
  - docs/API repair when docs/API artifacts are non-authoritative;
  - verifier contract correction when the verifier command is weak or exceeds
    its authority.
- [ ] Route ambiguous or insufficient conflicts to explicit safe stop.
- [ ] Verify no new hidden retry or continuation path is introduced.

## C33 Recovery Task And Safe Stop

- [ ] Render conflict object, authority candidates, selected authority,
  selected action, missing evidence, and rerun authority into the recovery
  task.
- [ ] Render explicit safe-stop payload for ambiguous or insufficient authority.
- [ ] Add repair packet fields for conflict jobs.
- [ ] Unit test recovery task rendering and safe-stop payload.

## C33 Eval And Report Fields

- [ ] Add eval schema fields for:
  - `contract_conflict_status`;
  - `contract_conflict_sides`;
  - `contract_conflict_authority`;
  - `contract_conflict_selected_action`;
  - `contract_conflict_safe_stop_reason`;
  - `contract_conflict_missing_evidence`;
  - `contract_conflict_source_of_truth`.
- [ ] Update `scripts/eval_case_schema.py`.
- [ ] Update `tests/test_eval_report.py`.
- [ ] Update `eval/README.md` and `docs/evaluation.md`.

## Focused Fixtures

- [ ] Add focused fixtures under
  `eval/cases/focused/control-recovery/contract-conflict/`.
- [ ] Cover implementation vs generated test conflict.
- [ ] Cover implementation vs pre-existing test conflict.
- [ ] Cover docs/API/schema vs implementation conflict.
- [ ] Cover weak verifier vs source/test conflict.
- [ ] Cover Phase27 no-progress handoff.
- [ ] Cover ambiguous-authority safe stop.
- [ ] Run focused fixture root and recheck.

## Coverage And Roadmap Updates

- [ ] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
  proof.
- [ ] Update `current_issue_phase_map.md` for KI-007 closure only after proof.
- [ ] Update `recovery_plan.md` Phase28 exit status only after proof.
- [ ] Update `workspace/mvp/logic/anvil/loadmap2/README.md` Phase28 row only
  after proof.
- [ ] Add `implementation_report.md` at closure time.

## Verification

- [ ] `cargo fmt --check`
- [ ] `cargo test contract_conflict`
- [ ] `cargo test semantic_failure`
- [ ] `cargo test recovery_orchestration`
- [ ] `cargo test recovery_task`
- [ ] `cargo test repair_job`
- [ ] `python3 tests/test_eval_report.py`
- [ ] focused C33 fixture root with recheck
- [ ] broad sign-off with C33 focused root
- [ ] `cargo test`
- [ ] `cargo build --release`

## Review Gate

- [ ] Confirm C33 does not become a profile workflow engine.
- [ ] Confirm ambiguous authority stops rather than choosing by path order.
- [ ] Confirm generated tests require binding before becoming authoritative.
- [ ] Confirm source repair is not the default when authority is unclear.
- [ ] Confirm no provider/model-specific branch is introduced.
- [ ] Confirm docs are updated with any runtime/eval behavior change.

## Plan Review

Review findings applied:

- Split tasks by conflict object, authority decision, orchestration, recovery
  task rendering, eval fields, and focused proof.
- Added explicit separation between authority side and repair target/action.
- Added explicit safe-stop tasks so the phase cannot close only by proving
  repairable conflicts.
- Required Phase27 handoff proof to prevent C25 deferral from remaining open.
- Kept coverage and roadmap status updates after proof, not before.
