# Phase27 Implementation Report

Date: 2026-06-23 JST

Status: completed / closed_proven

## Scope Closed

Phase27 closed `P20-COV-003` / KI-006 rows C21-C32:

- C21 target admission;
- C22 target prioritization;
- C23 repair job lifecycle;
- C24 repair attempt ledger;
- C25 no-progress strategy and Phase28 conflict deferral;
- C26 verifier diagnostic assessment;
- C27 verifier orchestration and rerun reporting;
- C28 verifier command policy;
- C29 artifact completion job;
- C30 focused edit recovery;
- C31 mechanical fallback admission;
- C32 patch validation and rollback admission.

## Runtime And Eval Changes

- Added Phase27 focused fixture cases under
  `eval/cases/focused/control-recovery/target-verifier-patch/`.
- Added expected-field parsing for target count/current-excerpt/patch/
  mechanical/rollback focused assertions.
- Strengthened mechanical fallback admission so it requires owner, action,
  target, target role, verifier/source-of-truth authority, and allowed change
  kind before rendering a bounded hint.
- Added representative unit tests for route/source/test/docs/setup/evidence
  target admission, no-progress contract-conflict deferral, Next.js/port/
  self-referential verifier diagnostics, mechanical fallback rejection, and
  patch validation/rollback reporting.

## Proof

Focused fixture root:

```text
eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917
```

Focused recheck:

```text
passed_recheck: 12
unknown/raw failure coverage defects: none
```

Broad sign-off:

```text
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149

status: pass
```

Targeted proof families:

- `cargo test target_admission`
- `cargo test repair_job`
- `cargo test verifier_diagnostic`
- `cargo test verifier_selection`
- `cargo test integrity_guard`
- `cargo test artifact_completion`
- `cargo test evidence_authority`
- `cargo test mechanical_repair`
- `cargo test repair_action_plan`
- `cargo test recovery_orchestration`
- `cargo test repair_loop`
- `python3 tests/test_eval_report.py`

Full local verification:

- `cargo fmt --check`
- `cargo test`
- `cargo build --release`

CI results are recorded in the closing turn.

## Row Disposition

| coverage id | final disposition | proof |
| --- | --- | --- |
| C21 | closed_proven | target-admission unit tests and C21 focused target matrix |
| C22 | closed_proven | ambiguous target-priority stop fixture and target priority tests |
| C23 | closed_proven | repair-job tests and C23 lifecycle/rerun fixture |
| C24 | closed_proven | attempt-ledger tests and C24 focused attempt fixture |
| C25 | closed_proven | no-progress tests and C25 Phase28 deferral fixture |
| C26 | closed_proven | verifier-diagnostic tests and C26 focused diagnostic fixture |
| C27 | closed_proven | repair lifecycle tests and C27 verifier-rerun safe-stop fixture |
| C28 | closed_proven | verifier-selection/integrity tests and C28 verifier-policy fixture |
| C29 | closed_proven | artifact-completion/evidence-authority tests and C29 completion fixture |
| C30 | closed_proven | target-admission focused edit tests and C30 stale-target fixture |
| C31 | closed_proven | mechanical-repair tests and C31 fallback fixture |
| C32 | closed_proven | patch-validation tests and C32 rollback fixture |

## Remaining Boundaries

- C33 contract-conflict resolution remains Phase28. Phase27 only proves that
  no-progress can defer to Phase28 without falling back to generic source
  repair.
- C34-C44 language/profile/tool/workspace/runtime-support expansion remains
  Phase29.
- C49-C50 priority decisions remain Phase30.
- Large timeout proof remains Phase31.

## Review Result

- No hidden retry, hidden continuation, provider/model-specific branch,
  profile workflow engine, or implicit dependency setup was introduced.
- Mechanical fallback became stricter, not broader.
- Coverage table changes were applied only after unit and focused fixture
  proof.
