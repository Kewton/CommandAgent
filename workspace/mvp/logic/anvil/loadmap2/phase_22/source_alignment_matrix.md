# Phase22 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C01 | `task_contract.rs`, `task_contract_core.rs`, `task_contract_taxonomy.rs`, `task_contract_display.rs` | Represent task purpose, kind, required artifacts, deterministic constraints, lifecycle state, expected completion evidence, and rendered contract facts. | Full Anvil actor-loop lifecycle, advisory memory, hidden orchestration, and any semantic policy that requires model classification. | `src/agent/step_runner/task_contract.rs`, `plan_prompt.rs`, `evidence.rs`, `scripts/eval_report.py` | Unit tests for task contract construction/rendering, eval report tests, focused fixture field checks. |
| C02 | `task_contract_request_inference.rs`, `contract_request_signals.rs`, `task_contract_admission.rs`, `task_kind_confirm.rs`, `classify_confirm_flow.rs` | Deterministically infer request signals from goal/profile/intent/required artifacts, admit or mark partial/conflict, and produce correction evidence when admission affects plan ownership. | Interactive confirmation flow, semantic secondary classifier, and provider/model-specific prompt branch. | `src/agent/step_runner/task_contract.rs`, `plan_input.rs`, `plan_lint/mod.rs`, focused planning fixtures | Unit tests for request signals/admission, plan lint evidence tests, focused `task-contract-admission`. |
| C03 | `objective_contract_projection.rs`, `behavior_contract_projection_e2e_tests.rs`, `required_behavior.rs`, `behavior_delta_obligation.rs`, `contract_bound_generation.rs`, `contract_generation_expectations.rs` | Project user-visible behavior into deterministic obligations for manifest/setup/build/dev-port/route/docs/data/test/source completion and enforce missing plan owners where deterministic. | Broad semantic expectation generation, hidden behavior repair workflows, and unbounded model-issued setup actions. | `src/agent/step_runner/task_contract.rs`, `deliverable_obligation.rs`, `profiles.rs`, `plan_lint/mod.rs`, `scripts/eval_report.py`, focused planning fixtures | Unit tests for obligation projection/lint, eval report tests, focused `behavior-obligation-projection`. |

## Review Result

Review findings applied:

- Mapped every Phase22 coverage row to explicit Anvil files and CommandAgent
  target modules.
- Marked omitted Anvil behavior so implementation does not accidentally import
  confirmation/advisory/hidden orchestration surfaces.
- Required proof per row, not just a shared broad sign-off.

## Final Proof Root

```text
eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658
```

Broad sign-off returned `status: pass` after C01-C03 proof completed.
