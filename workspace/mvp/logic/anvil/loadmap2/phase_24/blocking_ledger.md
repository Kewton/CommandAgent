# Phase24 Blocking Ledger

Date: 2026-06-23 JST

| blocker id | coverage id | owner layer | incomplete contract | suspected module family | downstream task | proof command / case | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P24-C07-001 | C07 | artifact ledger producer | Ledger source coverage is not proven for graph/tool/read/write/edit/verifier/workspace/setup/scaffold sources. | `artifact_ledger`, `artifact_graph`, `workspace_scope` | Complete source-family producer tests and merge semantics. | `cargo test artifact_ledger` | Every ledger source family renders deterministic source, role, lifecycle, ownership, and observed/change flags. | closed_proven |
| P24-C07-002 | C07 | completion authority input | Pass-side authority may still rely on terminal-state defaults instead of ledger entries. | `artifact_ledger`, `evidence_authority`, `artifact_completion` | Feed ledger entries into completion authority for required deliverables. | `cargo test evidence_authority`; `cargo test artifact_completion` | Required deliverables are accepted only from eligible ledger entries and rejected for generated/cache/raw/out-of-scope facts. | closed_proven |
| P24-C07-003 | C07 | eval/report | Ledger source fields may not be visible enough in focused reports. | `eval_report`, `eval_agent_slice`, `eval_failure_observation` | Add or verify eval fields for ledger source counts and path classes. | `python3 tests/test_eval_report.py`; focused ledger fixture | Eval report identifies ledger source signals and artifact ledger status for Phase24 cases. | closed_proven |
| P24-C08-001 | C08 | completion evidence producer | Verifier/command/file-layout evidence is partially present but not proven for all pass/fail/missing states. | `completion_evidence`, `evidence_producer`, `evidence_authority` | Complete producer tests and authority mapping. | `cargo test completion_evidence`; `cargo test evidence_producer`; `cargo test evidence_authority` | Completion evidence status is distinct for passed, failed, missing, unbound, and stale states. | closed_proven |
| P24-C08-002 | C08 | docs/data/report evidence | Docs, data, report, and profile-wide evidence producers are not proven end to end. | `completion_evidence`, `evidence_producer`, profiles, eval scripts | Add producer functions or fixture projections from observed facts. | focused completion fixture; `python3 tests/test_eval_report.py` | Docs/data/report/profile-wide facts become completion evidence without hidden tool execution. | closed_proven |
| P24-C08-003 | C08 | eval/report | Completion evidence kind/source may be defaulted instead of producer-derived. | `eval_report`, `eval_agent_slice` | Add expected fields and report aggregation for producer-derived completion evidence. | `python3 tests/test_eval_report.py`; focused completion fixture | Eval report identifies completion evidence kind/status/source for C08 cases. | closed_proven |
| P24-C09-001 | C09 | evidence binding producer | Binding families are helper functions but not proven as producer-complete. | `evidence_binding`, `evidence_producer` | Complete deterministic producer coverage for each binding family. | `cargo test evidence_binding`; `cargo test evidence_producer` | Manifest/import/executable/test/docs/schema/citation/file-layout bindings produce target-specific status. | closed_proven |
| P24-C09-002 | C09 | recovery contract evidence | Missing or failed binding must become structured contract evidence without dispatching hidden jobs. | `evidence_binding`, `recovery_contract`, `recovery_orchestration` | Ensure contract evidence carries target, expected binding, required literals, failed step, and repair target. | `cargo test evidence_binding`; targeted recovery/evidence test | Failed binding produces bounded contract evidence; bound binding does not create repair. | closed_proven |
| P24-C09-003 | C09 | completion authority / eval | Binding failures need distinct authority and eval status. | `evidence_authority`, `eval_report` | Feed binding status into completion authority and report sections. | `cargo test evidence_authority`; focused binding fixture | Eval and authority distinguish evidence binding failure from missing deliverable and failed verifier evidence. | closed_proven |
| P24-C10-001 | C10 | deliverable obligation projection | Required deliverables and profile obligations may not project full kind/path/evidence/freshness facts. | `deliverable_obligation`, `task_contract`, profiles | Complete projection and rendering tests for source/setup/test/docs/data/report. | `cargo test deliverable_obligation`; `cargo test task_contract` | Task/profile facts expose deliverable kind, path, required evidence, and freshness rules. | closed_proven |
| P24-C10-002 | C10 | freshness authority | Read-only observations may still satisfy fresh edit/current-plan requirements. | `evidence_authority`, `artifact_completion`, `deliverable_obligation` | Add stale/read-only freshness checks. | `cargo test evidence_authority`; `cargo test artifact_completion` | Stale/read-only evidence reports `freshness_status=stale` and cannot satisfy fresh deliverable completion. | closed_proven |
| P24-C10-003 | C10 | eval/report | Obligation/freshness fields may not be visible or asserted in focused reports. | `eval_report`, `eval_agent_slice` | Add expected fields and focused fixture assertions. | `python3 tests/test_eval_report.py`; focused freshness fixture | Eval report identifies deliverable obligations and freshness status for C10 cases. | closed_proven |

## Review Result

Review findings applied:

- Split each coverage row into producer, authority/consumer, and eval/report
  blockers where needed.
- Kept producer behavior deterministic and bounded; no hidden evidence runner
  or retry loop is allowed.
- No blocker uses provider throughput, model quality, or broad sign-off as a
  row-level closure condition.

## Implementation Result

All Phase24 blockers are `closed_proven`.

- Unit proof covered artifact ledger, completion evidence, evidence producer,
  evidence authority, evidence binding, deliverable obligation, task contract,
  plan lint, and artifact completion filters.
- Focused fixture proof root:
  `eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617`
- Focused assertions: `passed: 6`
- Recheck assertions: `passed_recheck: 6`
- Broad sign-off: `status: pass`
