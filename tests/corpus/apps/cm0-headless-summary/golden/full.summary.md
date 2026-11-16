Build: <BUILD>
# 実行結果

- 結果: 完了
- 保証: full
- ゲート: 合格
- 停止理由: 完了
- 次の一手: 追加操作なし
- 変更ファイル: 0
- 検証: 1 合格
- 終了コード: 0

## 変更ファイル

- なし

## 検証コマンド

- `cargo test --test smoke`: passed

## Machine details

Status: completed
Completion status: complete
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Command: --prompt
Command status: completed
Command completion: completed
Task status: complete
Effective profile: missing
Prompt layout: missing
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: not_checked
Release gate: pass
Requested port: missing
Evidence arbitration: missing
Depth profile: missing
completion_contract_verification_enabled=false
completion_contract_path_merge_enabled=false
completion_contract_path=missing
completion_contract_generated=false
external_contract_checked=false
external_contract_ok=false
browser_readiness_applicable=false
browser_readiness_execution_status=not_applicable
interaction_evidence_applicable=false
interaction_evidence_execution_status=not_applicable
Planner repaired: false
Planner release risk: false
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=0
Context truncation warning: none
Release quality completion: not_checked
Release gate reasons:
- none
Browser readiness: not_applicable
Browser readiness evidence: missing
Interaction evidence: not_applicable
Interaction evidence path: missing
State dimensions changed: none
Action hooks: none
Surface fit: missing
Text entry target: missing
Typed token: missing
Token echoed: missing
Text input state change: missing
Next action: run_acceptance_or_review_changes
Recovery next action: run_acceptance_or_review_changes
Stop reason: completed
Assurance: partial (effective_profile_unknown)

Time profile: provider 100% [prefill 25% · generation 63% · load 13%] · installs 0% · builds 0% · probe 0% · total 13s

Provider usage by role:
| Role | Provider time | Prompt tokens | Generation tokens | Thinking tokens | Prefill ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| executor | 8s | 1100 | 240 | n/a | 25% |
| planner | 5s | 700 | 120 | 40 | 25% |

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| unscoped | 13s | 13s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 240 | 8s | 1 |
| planner | 120 | 5s | 1 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| prose-only | 360 | 13s | 2 | 0B | 0B |

Completed phases:
- none

Failed phases:
- none

Pending phases:
- none
