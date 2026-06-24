# 0624_03 Local LLM Eval Summary

## Execution

- Date: 2026-06-24
- Provider: `ollama`
- Model: `qwen3.6:35b-a3b-coding-nvfp4`
- Runs: `1`
- Binary: `target/release/commandagent`

## Roots

| Suite | Root | Result |
| --- | --- | --- |
| minimal | `workspace/mvp/gptpro_fb/eval/0624_03/minimal/20260624T181254` | 1/2 |
| planner | `workspace/mvp/gptpro_fb/eval/0624_03/planner/20260624T181314` | 2/2 |
| small | `workspace/mvp/gptpro_fb/eval/0624_03/small/20260624T181330` | 4/4 |
| large | `workspace/mvp/gptpro_fb/eval/0624_03/large/20260624T181425` | 0/6 |
| large-gold | `workspace/mvp/gptpro_fb/eval/0624_03/large-gold/20260624T182107` | 2/6 |
| recovery | `workspace/mvp/gptpro_fb/eval/0624_03/recovery_fixed/20260624T183136` | 7/83 |

`workspace/mvp/gptpro_fb/eval/0624_03/recovery/20260624T182834` was superseded.
The first recovery run counted deterministic fixtures as `missing_plan` for
plan quality even though fixtures do not generate plans. The harness was fixed
so fixture and dry-run rows are `plan_quality_status=not_applicable`, then
recovery was rerun as `recovery_fixed`.

## Coverage

Rows by component:

- `minimal_loop`: 2
- `planner`: 7
- `full_agent_plan`: 4
- `full_agent_ultra`: 6
- `worker`: 6
- `recovery`: 78

Rows by proof mode:

- `real_llm`: 28
- `deterministic_fixture`: 75

## Contract State

Rows by contract layer:

- `ok`: 16
- `planning_contract`: 11
- `verification_contract`: 21
- `profile_contract`: 8
- `setup_bootstrap_contract`: 7
- `execution_contract`: 6
- `eval_success_contract`: 15
- `dev_server_port_contract`: 1
- `unknown_contract`: 18

Rows by terminal state:

- `ok`: 16
- `missing_deliverable`: 5
- `plan_lint_failed`: 4
- `profile_contract_failed`: 8
- `verifier_command_failed`: 20
- `dependency_missing`: 5
- `setup_failed`: 2
- `tool_protocol_failed`: 4
- `step_policy_failed`: 3
- `explicit_stop`: 17
- `eval_assertion_failed`: 8
- `evidence_binding_failed`: 3
- `missing_evidence`: 3
- `provider_transport_failed`: 1
- `port_in_use`: 1
- `progress_budget_exhausted`: 1
- `repair_exhausted`: 1
- `stale_evidence`: 1

## Plan Quality

Rows by plan quality status:

- `pass`: 13
- `warn`: 2
- `fail`: 5
- `not_applicable`: 83

The planner-only and small plan-run cases passed plan quality. Large
`ultra-plan-run` exposed plan quality failures: five `fail` rows and one
`warn` row.

## Notable Results

- minimal: `minimal-docs-direct-create` failed the eval assertion because
  `README.md` did not match the expected `minimal loop` content signal.
- planner: `/plan-steps` and `/ultra-plan` planner-only cases both passed.
- small: all four plan-run task cases passed.
- large: all six ultra-plan-run task cases failed. Primary contract layers were
  planning, profile, and verification.
- large-gold: two of six run-plan cases passed. This shows worker behavior
  improves when planner generation is removed, but verifier and step-policy
  failures remain.
- recovery: focused assertions passed 81/83 after the plan-quality fixture
  accounting fix. The two assertion failures were
  `focused-nextjs-tailwind-manifest-drift` and
  `focused-python-missing-test-artifact`.
