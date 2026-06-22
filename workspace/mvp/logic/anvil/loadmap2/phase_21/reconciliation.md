# Phase 21 Reconciliation

Date: 2026-06-23 JST

## Reconciliation Rule

Phase21 follows the recovery-plan chain:

```text
P20-COV-001
  -> Phase21 blocker
  -> C01-C12 coverage row
  -> implementation task
  -> proof command
  -> final sign-off rerun
```

If a row cannot be mapped through this chain, it is not eligible for
implementation or closure.

## P20-COV-001 Mapping

| Phase20 blocker | Phase21 blocker | coverage row | implementation task | proof command | final sign-off dependency |
| --- | --- | --- | --- | --- | --- |
| P20-COV-001 | P21-C01 | C01 | Complete task contract core evidence/lifecycle projection. | `cargo test task_contract`; `python3 tests/test_eval_report.py` | broad sign-off must pass after C01 proof. |
| P20-COV-001 | P21-C02 | C02 | Add deterministic request/task-kind admission signals. | `cargo test plan_lint`; focused task-admission fixture | broad sign-off must pass after C02 proof. |
| P20-COV-001 | P21-C03 | C03 | Project behavior deltas into lint/evidence and completion checks. | `cargo test task_contract`; `cargo test plan_lint`; focused behavior fixture | broad sign-off must pass after C03 proof. |
| P20-COV-001 | P21-C04 | C04 | Unify role classifier consumption across profile, verifier, and recovery. | `cargo test profile_artifact`; `cargo test artifact_graph` | broad sign-off must pass after C04 proof. |
| P20-COV-001 | P21-C05 | C05 | Feed scope-aware workspace facts into admission and reports. | `cargo test workspace_scope`; `cargo test workspace_snapshot` | broad sign-off must pass after C05 proof. |
| P20-COV-001 | P21-C06 | C06 | Bind ownership to completion evidence and repeated-target exclusion. | `cargo test artifact_ownership`; `cargo test target_admission` | broad sign-off must pass after C06 proof. |
| P20-COV-001 | P21-C07 | C07 | Reconcile deterministic artifact observations into bounded ledger facts. | `cargo test artifact_ledger`; `python3 tests/test_eval_report.py`; focused ledger fixture | broad sign-off must pass after C07 proof. |
| P20-COV-001 | P21-C08 | C08 | Add shared completion evidence producers for deterministic pass/fail facts. | `cargo test completion_evidence`; `cargo test evidence_authority` | broad sign-off must pass after C08 proof. |
| P20-COV-001 | P21-C09 | C09 | Add concrete evidence binding producers and recovery mapping. | `cargo test evidence_binding`; focused evidence-binding fixture | broad sign-off must pass after C09 proof. |
| P20-COV-001 | P21-C10 | C10 | Project deliverable obligations into freshness and reportable evidence states. | `cargo test deliverable_obligation`; `cargo test plan_lint`; `python3 tests/test_eval_report.py` | broad sign-off must pass after C10 proof. |
| P20-COV-001 | P21-C11 | C11 | Prove active-job lifecycle/progress and dispatch conflict behavior. | `cargo test active_job`; `cargo test recovery_orchestration`; focused dispatch fixture | broad sign-off must pass after C11 proof. |
| P20-COV-001 | P21-C12 | C12 | Connect remaining profile candidates to common recovery dispatch. | `cargo test recovery_orchestration`; `cargo test recovery_task` | broad sign-off must pass after C12 proof. |

## Current Final Sign-off Command

The broad sign-off command remains the Phase20 established command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Phase21 reruns this command to prove that the admission/reconciliation docs do
not regress the broad sign-off state. It does not use a green sign-off to
promote C01-C12 to `Implemented`.

## Review Result

The reconciliation keeps all arrows explicit. The main correction from earlier
phases is that future implementation phases cannot cite `P20-COV-001` as a
single broad blocker; they must close the specific P21-Cxx rows.
