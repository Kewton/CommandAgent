# Phase29 Implementation Report

Date: 2026-06-23 JST

Status: completed / closed_proven

## Summary

Phase29 closes KI-008 / C34-C44 by adding deterministic runtime-support
projections and row-level eval proof. The implementation exposes support facts
needed by recovery/eval without adding a second executor, hidden continuation,
implicit setup execution, provider-owned policy, or profile workflow engine.

## Implemented Boundaries

| row | status | implemented boundary |
| --- | --- | --- |
| C34 | closed_proven | `language_repair_adapter_status` projection from existing mechanical adapter evidence. |
| C35 | closed_proven | `effective_tool_policy` and `effective_tool_policy_status` projection from selected owner/action/job. |
| C36 | closed_proven | `tool_failure_recovery_status=bounded_correction` for bounded tool failure recovery evidence. |
| C37 | closed_proven | deterministic shell command classification and setup command authority projection. |
| C38 | closed_proven | workspace candidate status, ignored-dir single source policy, and ignored reasons. |
| C39 | closed_proven | job report status and owner/action projection. |
| C40 | closed_proven | scaffold status as artifact obligation. |
| C41 | closed_proven | generic non-coding evidence status. |
| C42 | closed_proven | answer/work-mode deterministic gate status. |
| C43 | closed_proven | explicit lifecycle projection status. |
| C44 | closed_proven | provider boundary status as transport-only proof. |

## Changed Code

- `src/agent/step_runner/command_classification.rs`
- `src/agent/step_runner/runtime_support.rs`
- `src/agent/step_runner/recovery_orchestration.rs`
- `src/agent/step_runner/setup_lifecycle.rs`
- `src/agent/step_runner/workspace_snapshot.rs`
- `scripts/eval_case_schema.py`
- `scripts/eval_agent_slice.sh`
- `scripts/eval_report.py`
- `tests/test_eval_report.py`
- `eval/cases/focused/control-recovery/runtime-support/*.yaml`

## Proof

Targeted checks:

- `cargo fmt --check`
- `cargo test command_classification --lib`
- `cargo test runtime_support --lib`
- `cargo test setup_lifecycle --lib`
- `cargo test workspace_snapshot --lib`
- `cargo test recovery_orchestration --lib`
- `python3 tests/test_eval_report.py`

Focused fixture root:

```text
eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335
```

Recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --cases-dir eval/cases/focused/control-recovery/runtime-support \
  --recheck
```

Result: `passed_recheck: 11`.

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917 \
  --root supplemental=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result: `status: pass`.

Full checks:

- `cargo test`: passed, 747 library tests plus integration/doc checks.
- `cargo build --release`: passed.

## Design Boundary

Accepted:

- deterministic evidence projection;
- row-visible report fields;
- bounded tool failure visibility;
- command classification without execution;
- provider-boundary proof that providers remain transport-only.

Not accepted:

- Anvil actor-loop lifecycle machinery;
- hidden future-phase selection;
- unbounded retries or hidden continuation;
- model-issued dependency installation;
- scaffold workflow engine;
- profile-owned recovery policy;
- provider/model-specific behavior policy.

## Roadmap Reconciliation

- `docs/eval/legacy-control-stack-coverage-20260621.md`: C34-C44 moved to
  `Implemented / Adopt`.
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`: KI-008
  moved to `closed_proven`.
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`: Phase29 marked
  completed / closed_proven.
- `workspace/mvp/logic/anvil/loadmap2/README.md`: Phase29 marked completed /
  closed_proven.

## Remaining Work

Phase29 does not close Phase30 C49/C50, Phase31 external timeout proof, or
Phase32 final migration declaration. Those remain assigned to their own
phases.
