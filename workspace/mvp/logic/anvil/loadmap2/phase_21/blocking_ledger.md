# Phase 21 Blocking Ledger

Date: 2026-06-23 JST

## Purpose

This ledger expands Phase20 blocker `P20-COV-001` into row-level blockers.
Rows are grouped only when they share owner, missing contract, proof command,
and closure condition. C01-C12 do not share all of those fields, so each row is
tracked separately.

## Downstream Phase Grouping

| downstream phase | rows | theme |
| --- | --- | --- |
| Phase22 | C01-C03 | task contract, request admission, behavior obligation projection |
| Phase23 | C04-C06 | artifact role, workspace scope, ownership single source of truth |
| Phase24 | C07-C10 | artifact ledger, completion evidence, evidence binding, deliverable audit |
| Phase25 | C11-C12 | active-job arbitration and recovery dispatch lifecycle proof |

## Ledger

| blocker id | coverage id | owner layer | incomplete contract | suspected module family | downstream implementation task | proof command | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P21-C01 | C01 | step runner task contract | Expected completion evidence, lifecycle state, richer constraints, and cross-command persistence are not row-proven. | `task_contract`, `plan_prompt`, eval report projection | Add typed evidence/lifecycle fields only where deterministic producers exist, then report them. | `cargo test task_contract`; `python3 tests/test_eval_report.py`; broad sign-off | Coverage C01 can move only when task contract core fields are typed, rendered, and reported with proof. | `split_forward` |
| P21-C02 | C02 | plan input / profile intent | Task-kind and request-signal admission is still coarse when profile ambiguity changes workflow. | `plan_input`, `profiles`, `plan_lint` | Define deterministic request signals and admission evidence for ambiguous task/profile cases. | `cargo test plan_lint`; focused task-admission fixture; broad sign-off | C02 closes when request signals select or stop deterministically under focused proof. | `split_forward` |
| P21-C03 | C03 | task contract / behavior obligations | Behavior deltas and completion checks are still partial beyond path/profile facts. | `task_contract`, `plan_lint`, profile verification | Project behavior deltas into obligations and completion checks with focused evidence. | `cargo test task_contract`; `cargo test plan_lint`; focused behavior fixture | C03 closes when obligations affect lint/evidence and prove expected behavior, not only prompt text. | `split_forward` |
| P21-C04 | C04 | artifact graph / profile artifacts | Role taxonomy is not fully unified across profile verification, verifier repair, and recovery admission. | `artifact_graph`, `profile_artifact`, verifier/profile adapters | Make the common role classifier the single consumed source for target admission and evidence. | `cargo test profile_artifact`; `cargo test artifact_graph` | C04 closes when shared roles cover representative profile paths and every consumer reads the same role facts. | `split_forward` |
| P21-C05 | C05 | workspace snapshot / scope admission | Persistent task-scope admission and richer selected-root handling are not proven. | `workspace_scope`, `workspace_snapshot`, safety path confinement | Feed scope-aware workspace walk facts into target admission and reports. | `cargo test workspace_scope`; `cargo test workspace_snapshot` | C05 closes when greenfield/single-project/explicit/ambiguous roots and excluded paths are proven through admission. | `split_forward` |
| P21-C06 | C06 | artifact graph / target admission | Ownership lacks enough completion-evidence producers and repeated-target exclusion proof. | `artifact_ownership`, `target_admission`, `artifact_graph` | Bind ownership decisions into completion evidence and repeated-target/no-progress exclusion. | `cargo test artifact_ownership`; `cargo test target_admission` | C06 closes when owned/candidate/out-of-scope decisions drive admission and completion evidence under proof. | `split_forward` |
| P21-C07 | C07 | minimal loop records / repair evidence | Ledger signals are partial across verifier observations, setup/scaffold deltas, and pass-side authority. | `artifact_ledger`, minimal-loop tool records, eval report | Reconcile all deterministic artifact observations into bounded ledger facts and reports. | `cargo test artifact_ledger`; `python3 tests/test_eval_report.py`; focused ledger fixture | C07 closes when all ledger signal families have producer proof and report projection. | `split_forward` |
| P21-C08 | C08 | verifier / completion authority | Docs/data/report/profile-wide evidence producers remain missing or partial. | `completion_evidence`, `evidence_authority`, verifier producer boundary | Add shared producers for deterministic completion facts and keep verifier authority explicit. | `cargo test completion_evidence`; `cargo test evidence_authority` | C08 closes when completion authority covers required pass/fail producer families with focused proof. | `split_forward` |
| P21-C09 | C09 | verifier / profile / setup binding | Manifest identity, docs section, data schema, source citation, route/import binding producers are not complete. | `evidence_binding`, profile verification, setup validation | Add concrete binding producers and route them to evidence-binding repair or explicit stop. | `cargo test evidence_binding`; focused evidence-binding fixture | C09 closes when binding failures identify target, owner, runner, expected/observed pair, and recovery action. | `split_forward` |
| P21-C10 | C10 | plan lint / profile / eval | Deliverable freshness and obligation projection remain partial across plan/profile/eval. | `deliverable_obligation`, `plan_lint`, eval report | Project deliverable obligations into freshness and evidence requirements without hidden repair. | `cargo test deliverable_obligation`; `cargo test plan_lint`; `python3 tests/test_eval_report.py` | C10 closes when stale/missing/fresh deliverable states are observable and proof-backed. | `split_forward` |
| P21-C11 | C11 | recovery orchestration | Active-job lifecycle and attempt-progress transitions are not broad enough for parity. | `active_job`, `recovery_orchestration` | Extend candidate lifecycle/progress fields and prove conflict/tie/no-owner transitions. | `cargo test active_job`; `cargo test recovery_orchestration`; focused dispatch fixture | C11 closes when dispatch owns one active job or emits a typed stop for all focused owner conflicts. | `split_forward` |
| P21-C12 | C12 | recovery orchestration | Profile-specific candidate producers and broader focused E2E dispatch proof remain incomplete. | `recovery_orchestration`, `recovery_task`, profile candidate producers | Connect remaining profile failure candidates to the common dispatch gate. | `cargo test recovery_orchestration`; `cargo test recovery_task`; broad sign-off | C12 closes when recovery owner/action is selected before prompt rendering across focused profile cases. | `split_forward` |

## Exit Review

Every selected row has an owner, incomplete contract, suspected module family,
proof command, downstream phase, and closure condition. No row remains vague
`Partial` inside Phase21's own ledger.
