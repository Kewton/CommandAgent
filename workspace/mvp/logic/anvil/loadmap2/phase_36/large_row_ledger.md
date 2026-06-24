# Phase36 Large Row Ledger

Date: 2026-06-24 JST

Status: closed

## Source Root

| family | root |
| --- | --- |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` |

This ledger is based on `recheck_summary.tsv` regenerated after the Phase36
eval/report projection change. It is a migration-accounting ledger, not proof
that the large user tasks succeeded.

## Disposition Vocabulary

| disposition | Phase36 meaning |
| --- | --- |
| `closed_owned_failure` | The row still failed, but has consistent owner/action/target/evidence and can be carried as an owned large failure. |
| `implementation_blocker` | The row exposes a CommandAgent or eval/report bug and must fail sign-off until fixed. |
| `accepted_external_limitation` | The row is blocked by provider/model throughput, network, or environment evidence. |
| `split_forward` | The row belongs to a later phase with a named closure proof. |

## Row Ledger

| case | terminal | diagnostic | active job | owner/action | target | evidence | disposition | reason | proof |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `large-fastapi-app-modify` | `verifier_command_failed` | `unknown_verifier_failure` | `source_implementation_repair` | `source` / `repair_source_error` | `app/main.py` (`entrypoint`) | `bound` / `failed` / `not_attempted` | `closed_owned_failure` | `owned_weak_verifier_failure_with_command` | verifier command: `python -m pytest tests/test_app.py -v`; target: `app/main.py` |
| `large-fastapi-app-new` | `tool_protocol_failed` | `tool_args_missing_required_field` | `tool_protocol_correction` | `tool_protocol` / `correct_tool_protocol` | `app/main.py` (`entrypoint`) | `bound` / `failed` / `not_attempted` | `closed_owned_failure` | `owned_tool_protocol_failure` | failed tool: `Edit`; missing field: `old`; verifier command: `python -m pytest tests/test_app.py -v` |
| `large-nextjs-app-modify` | `verifier_command_failed` | `edit_target_not_found` | `tool_protocol_correction` | `tool_protocol` / `correct_tool_protocol` | `app/page.tsx` (`entrypoint`) | `bound` / `failed` / `not_attempted` | `closed_owned_failure` | `owned_tool_protocol_failure` | failed tool: `Edit`; target correction action: `emit_tool_call_with_existing_target`; verifier command: `npm run build` |
| `large-nextjs-app-new` | `step_policy_failed` | `read_only_step_mutation` | `explicit_stop` | `explicit_stop` / `stop_with_structured_evidence` | `app/page.tsx` | `bound` / `failed` / `not_attempted` | `closed_owned_failure` | `owned_explicit_stop:read_only_step_mutation` | explicit stop reason: `read_only_step_mutation`; verifier command: `npm run build` |
| `large-rust-app-modify` | `verifier_command_failed` | `edit_target_not_found` | `tool_protocol_correction` | `tool_protocol` / `correct_tool_protocol` | `src/main.rs` (`entrypoint`) | `bound` / `failed` / `not_attempted` | `closed_owned_failure` | `owned_tool_protocol_failure` | failed tool: `Edit`; target correction action: `emit_tool_call_with_existing_target`; verifier evidence: `tests::test_io_error_conversion` |
| `large-rust-app-new` | `verifier_command_failed` | `blocked_bash_command_policy` | `source_implementation_repair` | `source` / `edit_source_for_diagnostic` | `src/main.rs` (`implementation`) | `bound` / `failed` / `not_attempted` | `closed_owned_failure` | `owned_tool_policy_failure` | diagnostic: `blocked_bash_command_policy`; target: `src/main.rs`; evidence: `bound/failed` |

## Disposition Counts

| disposition | count |
| --- | ---: |
| `closed_owned_failure` | 6 |
| `implementation_blocker` | 0 |
| `accepted_external_limitation` | 0 |
| `split_forward` | 0 |

## Review Notes

- No large row is labeled as external limitation.
- The two `edit_target_not_found` rows are tool-protocol-owned because their
  observed failure signatures identify `Edit` tool target failures.
- The read-only mutation row remains an explicit stop. Phase36 does not add a
  hidden repair turn after a read-only step-policy violation.
- `large-fastapi-app-modify` still has a weak verifier diagnostic, but the row
  is owned by source repair with target and verifier command evidence. It is
  not a migration blocker for ownership/sign-off accounting.
