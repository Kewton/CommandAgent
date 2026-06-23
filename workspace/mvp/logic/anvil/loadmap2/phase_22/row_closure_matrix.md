# Phase22 Row Closure Matrix

Date: 2026-06-23 JST

| coverage id | current status | adoption | owner layer | missing contract | target modules | required proof | closure condition | disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C01 | Implemented | Adopt | task contract / eval report | Closed: lifecycle, constraints, expected completion evidence, and persistence boundary are deterministic and reported. | `task_contract.rs`, `plan_prompt.rs`, `evidence.rs`, `scripts/eval_report.py` | `cargo test task_contract`, `python3 tests/test_eval_report.py`, focused field proof, broad sign-off regression | Contract fields are deterministic, rendered, reported, and bounded by explicit documented persistence decision. | closed_proven |
| C02 | Implemented | Partial | plan input / task contract / plan lint | Closed: request signals and admission status cover clear, ambiguous, and conflicting deterministic input. | `task_contract.rs`, `plan_input.rs`, `plan_lint/mod.rs` | unit tests for request signals/admission, focused `task-contract-admission`, broad sign-off regression | Clear inputs admit deterministically; ambiguous/conflicting inputs produce structured partial/conflict evidence without hidden confirmation flow. | closed_proven |
| C03 | Implemented | Adopt | task contract / plan lint / profiles / eval report | Closed: behavior-delta obligations are projected, owner-checked, and reported across common deterministic sources. | `task_contract.rs`, `deliverable_obligation.rs`, `profiles.rs`, `plan_lint/mod.rs`, `scripts/eval_report.py` | obligation projection tests, plan lint owner tests, focused `behavior-obligation-projection`, broad sign-off regression | Deterministic behavior obligations produce owner requirements, evidence/report fields, and missing-owner correction evidence. | closed_proven |

## Closure Rules

- `closed_proven` requires row-specific unit or fixture proof plus focused
  proof where listed.
- `split_forward` is allowed only for a narrower same-surface blocker with
  failed proof evidence.
- Broad sign-off is regression evidence, not row proof.

## Review Result

Review findings applied:

- Kept C01, C02, and C03 as separate closure rows.
- Added explicit persistence-boundary proof to C01.
- Added ambiguous/conflict behavior proof to C02.
- Added cross-profile obligation reporting proof to C03.
