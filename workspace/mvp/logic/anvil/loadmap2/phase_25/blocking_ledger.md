# Phase25 Blocking Ledger

Date: 2026-06-23 JST

| blocker id | coverage id | owner layer | incomplete contract | suspected module family | downstream task | proof command / case | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P25-C11-001 | C11 | active-job candidate model | Candidate records may not consistently carry owner/job/action/source/target/tool-policy/rerun/lifecycle facts across producers. | `active_job`, `recovery_orchestration`, `recovery_contract`, `evidence` | Complete candidate schema and source-labelled rendering. | `cargo test active_job`; `cargo test recovery_orchestration` | Every candidate family renders deterministic owner/job/action/source-of-truth/source-layer/target/role/tool-policy/rerun/reason fields. | closed_proven |
| P25-C11-002 | C11 | active-job arbitration | Priority, tie-break, same-owner merge, ambiguous-owner stop, no-owner stop, and conflict-stop behavior are not proven row by row. | `recovery_orchestration`, `active_job` | Add deterministic arbitration tests and stop-state evidence. | `cargo test recovery_orchestration`; focused dispatch fixture | Unique candidates select; compatible same-owner candidates merge; no-owner and ambiguous tie stop explicitly; contract conflict records Phase28 handoff evidence. | closed_proven |
| P25-C11-003 | C11 | eval/report | Dispatch lifecycle fields may still be inferred after the fact from reason text instead of produced by arbitration. | `evidence`, `eval_report`, `eval_agent_slice`, `eval_runtime_job_report` | Expose arbitration-produced lifecycle/candidate/tie-break fields. | `python3 tests/test_eval_report.py`; focused dispatch fixture | Eval rows show active job, owner, action, status, candidates, dispatch reason, and tie-break/stop reason from contract evidence. | closed_proven |
| P25-C12-001 | C12 | recovery dispatch gate | Candidate producers are not guaranteed to route through one selected dispatch decision before repair prompt rendering. | `recovery_orchestration`, `recovery_policy`, profiles, `recovery_contract` | Connect profile/setup/verifier/evidence/tool candidates to one gate. | `cargo test recovery_policy`; `cargo test recovery_orchestration` | Every deterministic candidate source reaches dispatch and produces exactly one selected owner/action or explicit stop. | closed_proven |
| P25-C12-002 | C12 | recovery task input | Repair prompts may still recompute owner/action from prose instead of consuming selected dispatch facts. | `recovery_task`, `repair_brief`, `repair_action_plan`, `runtime/repair_loop` | Render selected dispatch into repair task and brief. | `cargo test recovery_task`; `cargo test repair_brief`; focused owner/action fixture | Prompt input includes selected owner, job, action, target, role, tool policy, rerun authority, required action, and disallowed actions. | closed_proven |
| P25-C12-003 | C12 | dispatch conflict prevention | Multiple candidate owners can produce ambiguous work without a structured stop or same-owner merge rule. | `recovery_orchestration`, `recovery_policy`, `target_admission` | Add conflict/tie dispatch tests and explicit stop fields. | `cargo test recovery_orchestration`; focused ambiguous-dispatch fixture | Competing owner/action candidates do not run multiple repairs; they select by deterministic priority or stop with structured evidence. | closed_proven |
| P25-C12-004 | C12 | focused proof | Existing focused cases cover some owner/action fields but do not prove the full dispatch chain before prompt rendering. | focused eval cases, eval scripts | Add or expand focused dispatch fixtures and expected fields. | focused root under `eval/runs/loadmap2-phase25-focused-fixtures` with recheck | Focused assertions prove setup, manifest, route, source, docs, evidence-binding, verifier-contract, tool-protocol, no-owner, and ambiguous-tie dispatch paths. | closed_proven |

## Review Result

Review findings applied:

- Split C11 into candidate schema, arbitration rules, and eval/report
  visibility blockers.
- Split C12 into producer connection, prompt-input consumption, conflict
  prevention, and focused proof blockers.
- Marked contract-conflict handling as a Phase28 handoff unless Phase25 only
  proves structured stop behavior.
- No blocker uses provider throughput, model quality, CI success, or broad
  sign-off as a row-level closure condition.

## Implementation Result

All Phase25 blockers are `closed_proven`.

Proof:

- `cargo test active_job`
- `cargo test recovery_orchestration`
- `cargo test recovery_task`
- `python3 tests/test_eval_report.py`
- focused root `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`
- focused assertions `passed: 10`
- recheck assertions `passed_recheck: 10`
- broad sign-off `status: pass`

No blocker is carried forward from Phase25.
