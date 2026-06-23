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
| Phase27 | `P20-COV-003` | Close C21-C32 target/repair/verifier/completion/patch responsibilities; conflict-dependent branches may record a Phase28 dependency. |
| Phase28 | `P20-COV-004` | Implement or explicitly exclude C33 contract conflict job and close Phase27 conflict-dependent branches. |
| Phase29 | `P20-COV-005` | Close C34-C44 cross-profile/runtime-support responsibilities. |
| Phase30 | `P20-COV-006` | Decide C49-C50 priority/adoption status; may be pulled forward if needed. |
| Phase31 | `P20-LEDGER-001` | Resolve large timeout proof as proven or explicit external limitation. |
| Phase32 | final closure | Reconcile coverage, sign-off, and final migration state. |

## Issue Map

| issue id | current status | source blocker | coverage rows | problem statement | assigned phase | required proof |
| --- | --- | --- | --- | --- | --- | --- |
| KI-001 | closed_proven | P20-COV-001 | C01-C03 | Task contract core, request admission, and behavior obligation projection are implemented by Phase22. | Phase22 | `cargo test task_contract`, `cargo test plan_lint`, focused fixture root `eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658`, broad sign-off |
| KI-002 | closed_proven | P20-COV-001 | C04-C06 | Artifact role, workspace scope, and ownership are implemented by Phase23. | Phase23 | `cargo test profile_artifact`, `cargo test artifact_graph`, `cargo test workspace_scope`, `cargo test workspace_snapshot`, `cargo test artifact_ownership`, `cargo test target_admission`, `cargo test artifact_completion`, `cargo test evidence_authority`, focused fixture root `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023`, broad sign-off |
| KI-003 | split forward | P20-COV-001 | C07-C10 | Ledger, completion evidence, evidence binding, and deliverable audit need full producer/report proof. | Phase24 | producer tests, focused completion/binding proof, broad sign-off |
| KI-004 | split forward | P20-COV-001 | C11-C12 | Active-job lifecycle and dispatch gate need broader lifecycle/E2E proof. | Phase25 | dispatch tests, focused owner/action proof, broad sign-off |
| KI-005 | open | P20-COV-002 | C13-C20 | Recovery task, setup/profile mapping, semantic repair, repair brief, and action envelope remain partial. | Phase26 | row-level ledger, focused recovery matrix, broad sign-off |
| KI-006 | open | P20-COV-003 | C21-C32 | Target admission, verifier orchestration, repair lifecycle, completion job, focused edit, patch validation, and no-progress behavior remain partial. | Phase27 | row-level ledger, focused target/verifier/patch matrix, broad sign-off |
| KI-007 | open | P20-COV-004 | C33 | Contract conflict job is missing. | Phase28 | conflict unit tests, focused conflict fixture, broad sign-off |
| KI-008 | open | P20-COV-005 | C34-C44 | Language adapters, tool policy/failure recovery, command classification, workspace walk, job reporting, scaffold/data/docs support, answer-mode gating, lifecycle, and provider boundary remain partial. | Phase29 | row-level ledger, representative focused cases, broad sign-off |
| KI-009 | open | P20-COV-006 | C49-C50 | Quality and slash/plan UI helper rows are unresolved priority decisions. Default toward exclusion unless deterministic recovery or eval evidence shows the row is necessary. | Phase30, pull-forward allowed | coverage decision update with proof or design exclusion |
| KI-010 | open | P20-LEDGER-001 | P17-L001 | Large timeout rows are owned/evidence-bound but not pure completion proof. | Phase31 | non-timeboxed successful proof or explicit external limitation |
| KI-011 | open | final closure | all adopted rows | Final migration state is blocked until adopted rows are implemented or explicitly excluded. | Phase32 | final broad sign-off pass and final migration decision report |

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

### Phase25: C11-C12

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C11 | Active-job lifecycle, attempt-progress transitions, deterministic tie-break, no-owner, and conflict stop behavior. | active-job/recovery-orchestration tests, focused dispatch fixture |
| C12 | Recovery owner/action gate connected to profile-specific candidate producers before repair prompt rendering. | recovery-orchestration/recovery-task tests, profile dispatch focused fixture |

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

### Phase28: C33

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C33 | Contract conflict object, source-of-truth decision, spec authority, and ambiguous-authority safe stop. | conflict unit tests, focused conflict fixture, broad sign-off |

### Phase27 / Phase28 Dependency Boundary

Phase27 may prove only the no-progress branch selection and deferral behavior
for contract-conflict outcomes:

- classify that no-progress reached a contract-conflict candidate;
- record owner/action/target/evidence for that branch;
- stop or defer without falling back to generic source repair;
- create a Phase28 blocker that names C33 as the missing contract-conflict
  implementation.

Phase27 must not claim C33 conflict resolution as `closed_proven`. Phase28
owns the conflict object, source-of-truth decision, spec authority, and
ambiguous-authority safe stop proof. Phase28 must also close any C25
contract-conflict branch that Phase27 deferred.

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

### Phase30: C49-C50

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| C49 | Decide whether deterministic quality classification/confirmation is adopted, partially adopted, or excluded. Default exclusion unless a deterministic recovery/eval gap requires adoption. | coverage decision report, tests only if adopted |
| C50 | Decide whether slash/plan/command UI helpers are recovery-parity work or excluded from migration scope. Default exclusion unless a CommandAgent UX/eval gap proves it is necessary; do not import Anvil slash commands into the REPL by default. | coverage decision report, tests only if adopted |

### Phase31: P20-LEDGER-001

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| P17-L001 | Large timeout proof remains external-limited unless a non-timeboxed run proves completion. | non-timeboxed proof root or explicit limitation report |

### Phase32: Final Closure

| row | minimum blocker | expected proof family |
| --- | --- | --- |
| final | No adopted `Partial` or `Missing`, all ledgers closed/excluded/accepted external, final sign-off zero. | final coverage closure report and broad sign-off |

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
Phase32 completion state.

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
