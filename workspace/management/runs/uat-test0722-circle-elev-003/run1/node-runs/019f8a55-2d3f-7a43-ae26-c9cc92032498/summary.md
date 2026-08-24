Build: 922d08d+dirty 2026-07-22T14:52:18Z
Status: failed
Completion status: incomplete
Lifecycle: tui_command
Process: REPL exited cleanly (not task status)
Session/REPL status: repl_ready
Command: --ultra-plan-run
Command status: failed
Command completion: failed
Task status: failed
Effective profile: data
Prompt layout: legacy
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: not_checked
Release gate: not_applicable
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
Release quality completion: release_ready
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
Next action: fix_command_failure
Recovery next action: fix_command_failure
Stop reason: investigation final acceptance failed: diagnosis_unbound
Profile: data
Assurance: failed (diagnosis_unbound)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Failure kind: direct_cli_command_failed
TUI command failed: investigation final acceptance failed: diagnosis_unbound

Time profile: provider 100% [prefill 0% · generation 0% · load 0%] · installs 0% · builds 0% · probe 0% · total 6s

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| diagnose | 6s | 6s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 39 | 3s | 3 |
| repair | 275 | 4s | 1 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| full-file Write | 275 | 4s | 1 | 825B | 0B |
| tool-call | 39 | 3s | 3 | 0B | 0B |

Completed phases:
- reproduce-candidate (completed)
- diagnose (completed)
- bind-verify (completed)

Failed phases:
- none

Pending phases:
- none
---

Status: failed
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Action: UltraPlanRun("『data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。』の実行が失敗しました。まず output/diagnosis.md を作成し、調査の進展に応じて更新すること。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。")
Command status: failed
Command completion: failed
Task status: failed
Effective profile: data
Prompt layout: legacy
Contract origin: initial
Runtime acceptance: not_checked
Final acceptance: not_checked
Release gate: not_applicable
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
Release quality completion: release_ready
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
Next action: fix_command_failure
Recovery next action: fix_command_failure
Stop reason: investigation final acceptance failed: diagnosis_unbound
Profile: data
Assurance: failed (diagnosis_unbound)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Failure kind: process_failure
