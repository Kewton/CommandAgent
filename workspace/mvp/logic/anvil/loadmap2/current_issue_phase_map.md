# Current Issue To Phase Map

Date: 2026-06-23 JST

## Purpose

This file maps every currently known remaining migration issue to a concrete
future phase. It extends the Phase21 result so the plan does not stop at
Phase22-Phase25.

Phase22-Phase25 close the `P20-COV-001` surface. Phase26 and later cover the
remaining Phase20 continuation blockers.

This file is a derived navigation map. It does not override:

1. `docs/eval/legacy-control-stack-coverage-20260621.md` for final coverage
   adoption and row state;
2. `recovery_plan.md` for Phase17+ gates, recovery rules, and disposition
   semantics;
3. phase-local ledgers for execution detail after a phase package is created.

If this map disagrees with those sources, update this file to match them.

The source baseline is the coverage-table baseline:

- Anvil repository: `/Users/maenokota/share/work/github_kewton/Anvil-develop`
- Anvil HEAD: `b3ca3d330546a10bf90d8dd46bd3e102f1710573`
- dirty state: dirty at inventory clarification time; fixed in
  `anvil_source_baseline.md`

## Summary

| range | blocker | purpose |
| --- | --- | --- |
| Phase22-Phase25 | `P20-COV-001` | Close C01-C12 after Phase21 row-level split. |
| Phase26 | `P20-COV-002` | Close C13-C20 recovery task/setup/profile/semantic repair/action-envelope responsibilities. |
| Phase27 | `P20-COV-003` | Closed C21-C32 target/repair/verifier/completion/patch responsibilities; conflict-dependent branches record a Phase28 dependency instead of claiming C33 resolution. |
| Phase28 | `P20-COV-004` | Closed C33 contract conflict job and Phase27 conflict-dependent branches with focused C33 proof. |
| Phase29 | `P20-COV-005` | Close C34-C44 cross-profile/runtime-support responsibilities. |
| Phase30 | `P20-COV-006` | Closed C49-C50 priority decisions as excluded legacy advisory/UI surfaces. |
| Phase31 | `P20-LEDGER-001` | Closed large timeout proof with a fresh no-timeout large root. |
| Phase32 | final closure | Completed final coverage closure, sign-off, and migration decision. |

## Issue Map

| issue id | current status | source blocker | coverage rows | problem statement | assigned phase | required proof |
| --- | --- | --- | --- | --- | --- | --- |
| KI-001 | closed_proven | P20-COV-001 | C01-C03 | Task contract core, request admission, and behavior obligation projection are implemented by Phase22. | Phase22 | `cargo test task_contract`, `cargo test plan_lint`, focused fixture root `eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658`, broad sign-off |
| KI-002 | closed_proven | P20-COV-001 | C04-C06 | Artifact role, workspace scope, and ownership are implemented by Phase23. | Phase23 | `cargo test profile_artifact`, `cargo test artifact_graph`, `cargo test workspace_scope`, `cargo test workspace_snapshot`, `cargo test artifact_ownership`, `cargo test target_admission`, `cargo test artifact_completion`, `cargo test evidence_authority`, focused fixture root `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023`, broad sign-off |
| KI-003 | closed_proven | P20-COV-001 | C07-C10 | Ledger, completion evidence, evidence binding, and deliverable audit are implemented by Phase24. | Phase24 | artifact-ledger/completion-evidence/evidence-producer/evidence-authority/evidence-binding/deliverable-obligation tests, focused fixture root `eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617`, broad sign-off |
| KI-004 | closed_proven | P20-COV-001 | C11-C12 | Active-job lifecycle and dispatch gate are implemented by Phase25. | Phase25 | `cargo test active_job`, `cargo test recovery_orchestration`, `cargo test recovery_task`, `python3 tests/test_eval_report.py`, focused fixture root `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`, broad sign-off |
| KI-005 | closed_proven | P20-COV-002 | C13-C20 | Recovery task, setup/profile mapping, semantic repair, repair brief, and action envelope are implemented by Phase26. | Phase26 | `cargo test recovery_task`, `cargo test recovery_orchestration`, `cargo test setup_lifecycle`, `cargo test setup_artifact_validation`, `cargo test semantic_failure`, `cargo test repair_brief`, `cargo test repair_action_plan`, `cargo test profiles`, focused fixture root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off |
| KI-006 | closed_proven | P20-COV-003 | C21-C32 | Target admission, verifier orchestration, repair lifecycle, completion job, focused edit, patch validation, and no-progress behavior are implemented by Phase27. C33 conflict resolution remains Phase28-owned. | Phase27 | target/repair/verifier/patch tests, focused fixture root `eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917`, broad sign-off |
| KI-007 | closed_proven | P20-COV-004 | C33 | Contract conflict job is implemented with authority decision, repair-target-side projection, and ambiguous/insufficient-authority safe stop. | Phase28 | `cargo test contract_conflict`, `cargo test recovery_orchestration`, focused fixture root `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`, broad sign-off |
| KI-008 | closed_proven | P20-COV-005 | C34-C44 | Language adapters, tool policy/failure recovery, command classification, workspace walk, job reporting, scaffold/data/docs support, answer-mode gating, lifecycle, and provider boundary are implemented as bounded runtime-support projections. | Phase29 | targeted Rust/Python tests, focused fixture root `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335`, broad sign-off |
| KI-009 | closed_excluded | P20-COV-006 | C49-C50 | Quality confirmation and slash/plan UI helper rows were reviewed in Phase30 and excluded as legacy advisory/UI surfaces. Existing CommandAgent eval taxonomy and CLI/slash parser remain native responsibilities, not Anvil compatibility ports. | Phase30 | coverage decision update, Phase30 source alignment, `git diff --check`, `python3 tests/test_eval_report.py`, `cargo test slash_command --lib` |
| KI-010 | closed_proven | P20-LEDGER-001 | P17-L001 | Large timeout rows are closed by a fresh no-timeout large proof root. | Phase31 | `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`, large recheck, broad sign-off pass |
| KI-011 | closed_proven | final closure | all adopted rows | Final migration state is closed as `migration_complete_with_explicit_exclusions`: adopted rows are implemented, explicit exclusions are documented, and final sign-off passes. | Phase32 | final broad sign-off pass and `docs/eval/anvil-migration-complete.md` |

## Minimum Internal Split

The tables below define the minimum row-level split expected when each phase
starts. A phase may split further, but it may not merge these rows into a vague
grouped blocker.

### Phase22: C01-C03

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C01 | Task contract core evidence, lifecycle, constraints, and persistence boundary. | `cargo test task_contract`, eval report tests, broad sign-off |
| C02 | Deterministic task-kind/request signal admission for ambiguous task/profile input. | plan-lint tests, focused task-admission fixture |
| C03 | Behavior-delta obligations projected into lint/evidence/completion checks. | task-contract tests, plan-lint tests, focused behavior fixture |

Status: `closed_proven` by Phase22. No C01-C03 row is split forward.

### Phase23: C04-C06

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C04 | Artifact role taxonomy consumed by profile verification, verifier repair, and recovery admission. | profile-artifact tests, artifact-graph tests |
| C05 | Scope-aware workspace admission for greenfield, single-project, explicit root, ambiguous parent, and excluded paths. | workspace-scope/snapshot tests, focused scope fixture |
| C06 | Ownership decisions consumed by target admission, completion evidence, and repeated-target exclusion. | artifact-ownership tests, target-admission tests |

Status: `closed_proven` by Phase23. No C04-C06 row is split forward.

### Phase24: C07-C10

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C07 | Ledger producers for tool records, verifier mentions, setup/scaffold deltas, workspace observations, and pass-side authority. | artifact-ledger tests, eval report tests, focused ledger fixture |
| C08 | Completion evidence producers for verifier, file layout, docs, data, report, and profile-wide facts. | completion-evidence/evidence-authority tests, focused completion fixture |
| C09 | Evidence binding producers for manifest identity, docs section, schema, source citation, route/import, and test script bindings. | evidence-binding tests, focused binding fixture |
| C10 | Deliverable obligations, freshness rules, and read-only freshness checks projected into plan/profile/eval. | deliverable-obligation tests, plan-lint tests, eval report tests |

Status: `closed_proven` by Phase24. No C07-C10 row is split forward.

### Phase25: C11-C12

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C11 | Active-job lifecycle, attempt-progress transitions, deterministic tie-break, no-owner, and conflict stop behavior. | active-job/recovery-orchestration tests, focused dispatch fixture |
| C12 | Recovery owner/action gate connected to profile-specific candidate producers before repair prompt rendering. | recovery-orchestration/recovery-task tests, profile dispatch focused fixture |

Status: `closed_proven` by Phase25. No C11-C12 row is split forward.

### Phase26: C13-C20

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C13 | Recovery messages, repair packets, and safe-stop payload coverage for evidence/completion failures. | recovery-task tests, safe-stop focused fixture |
| C14 | Setup bootstrap lifecycle, candidate validation, setup result ledger, and non-Node setup policies. | setup validation/runtime tests, setup focused matrix |
| C15 | Project probe/profile/scaffold profile facts, bounded scaffold materialization, and scaffold completion evidence. | profile output tests, scaffold focused fixture |
| C16 | Profile failure to typed recovery job mapping across route, manifest, setup, source, scaffold, and explicit stop. | profile mapping tests, focused profile-failure matrix |
| C17 | Semantic failure report conflict objects, cluster target ranking, and live eval evidence. | semantic-failure tests, verifier fixture |
| C18 | Semantic repair plan cluster exhaustion and role-strategy transitions from repair outcomes. | recovery-task/repair-state tests, focused semantic repair fixture |
| C19 | Repair brief target, root cause, constraints, allowed/disallowed actions, and E2E evidence. | repair-brief tests, focused repair brief fixture |
| C20 | Repair action space lifecycle transitions and all action-family proof. | repair-action tests, focused action-envelope matrix |

Status: `closed_proven` by Phase26. No C13-C20 row is split forward.

### Phase27: C21-C32

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C21 | Repair target decision/admission across route, source, test, docs, setup, and evidence-binding cases. | target-admission tests, focused target matrix |
| C22 | Target prioritization by failure kind, authority, role, and progress history. | target-priority tests, focused prioritization fixture |
| C23 | Repair job state machine lifecycle and verifier rerun transition reporting. | repair-job tests, focused lifecycle fixture |
| C24 | Repair attempt ledger across profile families and attempt outcomes. | repair-job tests, eval report tests, focused attempt-ledger fixture |
| C25 | No-progress recovery strategy branches across target, role, evidence binding, contract conflict, scaffold, and explicit stop. | no-progress tests, focused no-progress matrix |
| C26 | Verifier diagnostic assessment with language-specific diagnostics and weak target filters. | verifier-diagnostic tests, verifier fixture |
| C27 | Verifier orchestration, failure attempt limits, rerun outcome events, binding scope, and safe-stop report. | verifier orchestration tests, focused verifier fixture |
| C28 | Verifier command policy for generated tests, self-reference, unsupported assertions, and expectation audit. | verifier-selection/integrity tests, focused verifier-policy fixture |
| C29 | Artifact completion job bound to ledger, ownership, freshness, and missing-evidence distinction. | artifact-completion/evidence-authority tests, focused completion-job fixture |
| C30 | Focused edit recovery after target admission with current excerpt and stale-target rejection. | target-admission/ledger tests, focused edit fixture |
| C31 | Forced small edit / deterministic fallback admission and safe mutation proof. | mechanical-repair tests, patch admission fixture |
| C32 | Patch executor/validation, unsafe/noop/duplicate/test-weakening rejection, rollback proof. | patch-validation tests, focused patch fixture |

Status: `closed_proven` by Phase27. No C21-C32 row is split forward. The
C25 contract-conflict branch records a Phase28/C33 dependency only; it does
not close C33 conflict resolution.

### Phase28: C33

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C33 | Contract conflict object, source-of-truth decision, spec authority, and ambiguous-authority safe stop. | closed_proven by contract-conflict tests, focused fixture root `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`, and broad sign-off |

### Phase27 / Phase28 Dependency Boundary

Phase27 may prove only the no-progress branch selection and deferral behavior
for contract-conflict outcomes:

- classify that no-progress reached a contract-conflict candidate;
- record owner/action/target/evidence for that branch;
- stop or defer without falling back to generic source repair;
- create a Phase28 blocker that names C33 as the missing contract-conflict
  implementation.

Phase27 must not claim C33 conflict resolution as `closed_proven`. Phase28
now owns and closes the conflict object, source-of-truth decision, spec
authority, ambiguous-authority safe stop proof, and the C25 contract-conflict
branch that Phase27 deferred.

### Phase29: C34-C44

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C34 | Language-specific mechanical repair families and live proof. | language adapter tests, focused language matrix |
| C35 | Owner/action-aware tool policy and effective policy across setup, evidence, and repair jobs. | tool policy tests, focused tool-policy fixture |
| C36 | Tool failure recovery for provider parse, stale edit, prose-only, and schema branches. | tool recovery tests, focused tool-failure fixture |
| C37 | Bash/setup command classification with evidence binding and setup command authority. | bash policy tests, setup command fixture |
| C38 | Workspace candidates/walk with scope-aware ignored-dir single source of truth. | workspace walk tests, candidate discovery fixture |
| C39 | Job report/progress events with active owner, action plan status, and attempt outcomes. | runtime event/eval report tests |
| C40 | Scaffold pipeline as setup/artifact contract, not hidden workflow engine. | scaffold tests, focused scaffold fixture |
| C41 | Data/docs/research/ops evidence via generic completion/binding producers. | docs/data profile tests, focused non-coding matrix |
| C42 | Answer-only and work-mode gating without broadening normal coding repair. | final-answer/step-policy tests |
| C43 | Interruption, lifecycle, turn state only where explicit recovery contracts require it. | CLI/session/runtime tests |
| C44 | Provider/model request plumbing kept transport-only and policy-free. | provider tests, prompt boundary tests |

Phase29 status: `closed_proven`.

Proof:

- Targeted tests: `cargo test command_classification --lib`, `cargo test runtime_support --lib`, `cargo test setup_lifecycle --lib`, `cargo test workspace_snapshot --lib`, `cargo test recovery_orchestration --lib`, `python3 tests/test_eval_report.py`.
- Focused fixture root: `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335`.
- Recheck: `python3 scripts/eval_report.py eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 --cases-dir eval/cases/focused/control-recovery/runtime-support --recheck`, with `passed_recheck: 11`.
- Broad sign-off: `python3 scripts/eval_signoff.py --require-recheck ... --root supplemental=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 ...`, status `pass`.

### Phase30: C49-C50

Phase30 status: `closed_excluded`.

Proof:

- Coverage decision: C49-C50 are `Excluded / Excluded` in
  `docs/eval/legacy-control-stack-coverage-20260621.md`.
- C49 rationale: existing deterministic eval/recovery taxonomy covers
  verifier, profile, setup, tool protocol, and implementation-quality
  attribution; Anvil semantic quality confirmation remains outside the
  minimal-loop architecture.
- C50 rationale: CommandAgent keeps native CLI/REPL slash commands and excludes
  Anvil UI rendering/helper compatibility as migration work.
- Verification: `git diff --check`, `python3 tests/test_eval_report.py`,
  `cargo test slash_command --lib`.

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C49 | Excluded with rationale: no deterministic gap requires Anvil semantic quality confirmation. | coverage decision report and docs/test checks |
| C50 | Excluded with rationale: Anvil slash/plan UI helpers are not recovery-parity work. | coverage decision report and slash parser regression check |

### Phase31: P20-LEDGER-001

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| P17-L001 | Large timeout proof required a fresh non-timeboxed run. | closed_proven by `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`, recheck, and broad sign-off |

### Phase32: Final Closure

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| final | No adopted `Partial` or `Missing`, all ledgers closed/excluded/accepted external, final sign-off zero. | closed_proven by final coverage closure report `docs/eval/anvil-migration-complete.md` and broad sign-off pass |

Phase32 status: `closed_proven`.

Final decision:

```text
migration_complete_with_explicit_exclusions
```

Proof:

- final report: `docs/eval/anvil-migration-complete.md`;
- final sign-off: `python3 scripts/eval_signoff.py --require-recheck ...`,
  result `status: pass`;
- Phase32 implementation report:
  `workspace/mvp/logic/anvil/loadmap2/phase_32/implementation_report.md`.

## Already Accounted Rows

The following coverage rows are not assigned to Phase22-Phase32 because they
are already implemented or explicitly excluded in the coverage table.

| coverage rows | current state | reason |
| --- | --- | --- |
| C45 | Implemented | Provider transport parser is already implemented and is not a remaining migration blocker. |
| C46-C48 | Excluded | Working memory/reminders, case records, anti-pattern corpora, and advisory sidecars remain outside the product direction. |
| C51-C54 | Excluded | Legacy engine selector, hidden/unbounded repair loop, provider/model-specific behavioral policy, and model-issued dependency installation remain excluded by design. |

## Phase Admission Rule

Each phase must start by splitting its assigned blocker into row-level ledger
items before runtime changes begin. A phase may close only when each assigned
row is one of:

- `closed_proven`;
- `excluded_with_rationale`;
- `blocked_external` with owner/action/evidence, only where allowed;
- split to a narrower same-surface blocker with failed proof evidence.

No phase may use CI success alone as migration proof.

Broad sign-off is a regression and ownership gate, not a row-closure proof by
itself. A row closes only with its row-specific proof plus any required broad
sign-off dependency. `blocked_external` is valid only for proof limits after
owner/action/evidence exist; by final closure it must be accepted in the final
report or converted to an explicit exclusion. `split_forward` may close an
intermediate phase only when the narrower blocker, owner, downstream phase,
failed proof, and closure condition are recorded. It is not allowed as a final
Phase32 completion state. KI-011 is now closed by Phase32.

## Review Gate

Before starting a phase:

- The phase directory must contain `README.md`, `implementation_tasks.md`,
  `concrete_work_plan.md`, `source_alignment_matrix.md`,
  `row_closure_matrix.md`, `blocking_ledger.md`, and `reconciliation.md`.
- Every selected row must have owner, missing contract, target module family,
  proof command, closure condition, and downstream broad sign-off dependency.
- `source_alignment_matrix.md` must list Anvil source files, adopted behavior,
  intentionally omitted behavior, CommandAgent target modules, and proof method
  for every selected coverage row.
- If a row's proof requires model-facing behavior, changes focused assertions,
  or depends on focused eval for closure, `focused_worklist.md` must name the
  case and expected assertion. Otherwise it is not required.

Before closing a phase:

- `implementation_report.md` must include row disposition counts and exact
  verification results.
- Any row not `closed_proven` must be either explicitly excluded,
  allowed `blocked_external`, or split to a narrower same-surface blocker with
  failed proof evidence.
- The final response for the phase must state whether the phase closed its
  assigned blockers or only created a follow-up split.

For Phase26-Phase31, `split_forward` is allowed only for a newly discovered
narrower same-surface blocker with failed proof evidence, one owner, one proof
gate, and an assigned downstream phase. It is not allowed for work that was
already in that phase's selected rows, and it cannot be used to defer a row
because implementation is simply incomplete.

Coverage `Partial` and adoption `Partial` are not final states. Coverage
`Partial` means implementation proof is incomplete. Adoption `Partial` means
only a scoped subset is adopted; the adopted subset must become `Implemented`
and the omitted subset must become `Excluded` with rationale before final
closure.
