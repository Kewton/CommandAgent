# Phase22 Blocking Ledger

Date: 2026-06-23 JST

| blocker id | coverage id | owner layer | incomplete contract | suspected module family | downstream task | proof command / case | closure condition | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P22-C01-001 | C01 | task contract | Lifecycle state is not a first-class proof boundary. | `task_contract` | Add/render/test lifecycle state without using it as hidden control flow. | `cargo test task_contract` | Lifecycle values are deterministic and visible in rendered/eval evidence. | closed_proven |
| P22-C01-002 | C01 | task contract / eval report | Constraints and expected completion evidence are not fully represented. | `task_contract`, `eval_report` | Project deterministic constraints and completion evidence names. | `cargo test task_contract`; `python3 tests/test_eval_report.py` | Reported fields identify constraints and completion evidence for C01 cases. | closed_proven |
| P22-C01-003 | C01 | task contract / session boundary | Cross-command persistence boundary is not explicitly proven or documented. | `task_contract`, `plan_store`, `session` | Prove existing persistence or document bounded non-persistence. | `docs/architecture.md`; `cargo test task_contract` | Persistence boundary is explicit: no hidden cross-command task memory; contracts are visible in plan/session/evidence/eval artifacts and reconstructed from inputs. | closed_proven |
| P22-C02-001 | C02 | plan input / task contract | Request signal extraction is too coarse for ambiguous profile/task input. | `plan_input`, `task_contract` | Add deterministic signal structure and tests. | `cargo test task_contract` | New/modify/docs/data/investigation/ambiguous signals classify deterministically. | closed_proven |
| P22-C02-002 | C02 | plan lint | Admission partial/conflict is not strongly connected to correction evidence. | `plan_lint`, `correction_evidence` | Emit structured evidence when admission affects plan ownership. | `cargo test plan_lint` | Conflicting input does not proceed as admitted without evidence when artifact ownership matters. | closed_proven |
| P22-C02-003 | C02 | eval | Focused task-admission expected fields may not cover new admission data. | focused eval | Update focused case/report assertions. | focused `task-contract-admission` root `eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658` | Focused report proves admission status and request kind. | closed_proven |
| P22-C03-001 | C03 | task contract / profiles | Behavior obligations do not cover enough deterministic sources. | `task_contract`, `profiles`, `deliverable_obligation` | Add behavior-delta obligation projection for selected common profiles. | `cargo test task_contract` | Next.js/docs/data obligations project without semantic guessing. | closed_proven |
| P22-C03-002 | C03 | plan lint | Missing behavior obligation owner is not enforced for all deterministic obligation classes. | `plan_lint` | Add plan lint owner checks and correction evidence. | `cargo test plan_lint` | Missing owner steps produce structured correction evidence. | closed_proven |
| P22-C03-003 | C03 | eval report | Behavior obligation report fields do not yet prove owner/path/status. | `scripts/eval_report.py`, focused eval | Add or update report fields and focused expectations. | `python3 tests/test_eval_report.py`; focused `behavior-obligation-projection` root `eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658` | Report identifies obligation code, owner, status, target path, and missing owner where applicable. | closed_proven |

## Review Result

Review findings applied:

- Split C01 persistence from lifecycle/constraint work so it cannot be missed.
- Split C02 request inference from correction evidence and focused proof.
- Split C03 projection, lint enforcement, and eval reporting.
- No blocker uses model-throughput, provider throughput, or broad sign-off as
  a row-level closure condition.
