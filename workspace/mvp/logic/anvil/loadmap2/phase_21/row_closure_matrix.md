# Phase 21 Row Closure Matrix

Date: 2026-06-23 JST

## Scope

Phase21 selects `P20-COV-001` from the Phase20 continuation ledger and
accounts for coverage rows C01-C12.

This matrix does not convert these rows to `Implemented`. Phase20 already
proved that broad sign-off can pass while row-level parity proof is still
missing. Phase21 therefore closes the admission/reconciliation work by
splitting each selected row into an owned downstream blocker with a proof gate.

Baseline:

| field | value |
| --- | --- |
| commit | `a3c5eb3` |
| branch | `develop` |
| dirty flag before Phase21 edits | clean |
| selected blocker | `P20-COV-001` |
| selected coverage rows | C01-C12 |

## Disposition Summary

| disposition | count |
| --- | ---: |
| `closed_proven` | 0 |
| `excluded_with_rationale` | 0 |
| `split_forward` | 12 |
| `open` | 0 |

`split_forward` means the Phase21 admission gate is satisfied for that row:
owner, incomplete contract, target module family, proof command, downstream
phase, and closure condition are all explicit.

## Closure Matrix

| coverage id | source mechanism | owner layer | Phase21 parity target | implementation target | proof gate | disposition | downstream blocker |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C01 | Task contract core | step runner task contract | Task purpose, kind, required artifacts, constraints, expected evidence, and lifecycle boundary are typed and reportable. | `src/agent/step_runner/task_contract.rs`, `src/agent/step_runner/plan_prompt.rs`, `scripts/eval_report.py` | `cargo test task_contract`; `python3 tests/test_eval_report.py`; broad sign-off | `split_forward` | P21-C01 |
| C02 | Task contract inference and admission | plan input / profile intent | Request signals deterministically admit task kind/profile authority when ambiguity affects workflow choice. | `src/agent/step_runner/plan_input.rs`, `src/agent/step_runner/profiles.rs`, `src/agent/step_runner/plan_lint` | `cargo test plan_lint`; focused task-admission fixture; broad sign-off | `split_forward` | P21-C02 |
| C03 | Objective and behavior contract projection | task contract / behavior obligations | User-visible behavior requirements become obligations and completion checks, not prompt prose only. | `src/agent/step_runner/task_contract.rs`, `src/agent/step_runner/plan_lint`, profile verification | `cargo test task_contract`; `cargo test plan_lint`; focused behavior-obligation fixture | `split_forward` | P21-C03 |
| C04 | Artifact role taxonomy | artifact graph / profile artifacts | One role taxonomy classifies setup, implementation, test, docs, data, route, and integration artifacts across shared consumers. | `src/agent/step_runner/artifact_graph.rs`, `src/agent/step_runner/profile_artifact.rs` | `cargo test profile_artifact`; `cargo test artifact_graph`; broad sign-off | `split_forward` | P21-C04 |
| C05 | Task workspace scope | workspace snapshot / scope admission | Scope facts distinguish greenfield, single-project, explicit root, ambiguous parent, generated output, and dependency/cache paths. | `src/agent/step_runner/workspace_scope.rs`, `src/agent/step_runner/workspace_snapshot.rs`, safety path confinement | `cargo test workspace_scope`; `cargo test workspace_snapshot`; broad sign-off | `split_forward` | P21-C05 |
| C06 | Artifact ownership | artifact graph / target admission | Ownership decisions are consumed by target admission and completion authority, with candidate-only and repeated-target exclusions. | `src/agent/step_runner/artifact_ownership.rs`, `src/agent/step_runner/target_admission.rs`, `src/agent/step_runner/artifact_graph.rs` | `cargo test artifact_ownership`; `cargo test target_admission`; broad sign-off | `split_forward` | P21-C06 |
| C07 | Artifact ledger | minimal loop records / repair evidence | Tool records, workspace observations, verifier mentions, setup/scaffold deltas, and ownership reasons form bounded artifact evidence. | `src/agent/step_runner/artifact_ledger.rs`, minimal-loop tool records, eval report projection | `cargo test artifact_ledger`; `python3 tests/test_eval_report.py`; focused ledger fixture | `split_forward` | P21-C07 |
| C08 | Completion evidence | verifier / completion authority | Pass/fail authority is derived from verifier, file-layout, docs/data/report, and profile-wide evidence producers. | `src/agent/step_runner/completion_evidence.rs`, `src/agent/step_runner/evidence_authority.rs`, verifier producer boundary | `cargo test completion_evidence`; `cargo test evidence_authority`; focused completion fixture | `split_forward` | P21-C08 |
| C09 | Evidence binding | verifier / profile / setup binding | Required deliverables bind to concrete evidence runners such as manifest identity, route/import, docs section, schema, or source citation. | `src/agent/step_runner/evidence_binding.rs`, profile verification, setup validation | `cargo test evidence_binding`; focused evidence-binding fixture; broad sign-off | `split_forward` | P21-C09 |
| C10 | Deliverable obligation audit | plan lint / profile / eval | Required deliverables, evidence requirements, and freshness rules are projected and audited without hidden repair triggers. | `src/agent/step_runner/deliverable_obligation.rs`, `src/agent/step_runner/plan_lint`, `scripts/eval_report.py` | `cargo test deliverable_obligation`; `cargo test plan_lint`; `python3 tests/test_eval_report.py` | `split_forward` | P21-C10 |
| C11 | Active job arbiter | recovery orchestration | Candidate owner/job/action/target/tool-policy facts select exactly one dispatch candidate or stop with a contract conflict. | `src/agent/step_runner/active_job.rs`, `src/agent/step_runner/recovery_orchestration.rs` | `cargo test active_job`; `cargo test recovery_orchestration`; focused dispatch fixture | `split_forward` | P21-C11 |
| C12 | Recovery owner / dispatch gate | recovery orchestration | Recovery owner/action selection is a gate before repair prompt rendering, with explicit conflict/no-owner stop behavior. | `src/agent/step_runner/recovery_orchestration.rs`, `src/agent/step_runner/recovery_task.rs` | `cargo test recovery_orchestration`; `cargo test recovery_task`; broad sign-off | `split_forward` | P21-C12 |

## Review Result

The initial closure risk was to treat existing partial implementations as
enough proof. The matrix keeps every selected row below `closed_proven` because
the current coverage table still names missing producer, lifecycle, admission,
or E2E proof. Each row is instead split into a named downstream blocker, which
prevents another phase from closing by prose summary alone.
