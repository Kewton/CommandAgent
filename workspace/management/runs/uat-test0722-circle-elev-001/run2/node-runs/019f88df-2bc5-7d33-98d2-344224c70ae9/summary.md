Build: f3e7605+dirty 2026-07-22T07:46:37Z
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
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=0 retries=3 quality_warnings=0 quality_issues=4
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
Stop reason: investigation reproducer was not identified
Profile: data
Assurance: static (investigation_probe_not_run)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Failure kind: direct_cli_command_failed
TUI command failed: investigation reproducer was not identified

Time profile: provider 100% [prefill 2% · generation 98% · load 0%] · installs 0% · builds 0% · probe 0% · total 8m32s

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| reproduce-candidate | 8m32s | 8m32s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| planner | 17062 | 8m32s | 3 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| prose-only | 17062 | 8m32s | 3 | 0B | 0B |

Completed phases:
- none

Failed phases:
- reproduce-candidate (failed)

Pending phases:
- phase 2 (pending)
- phase 3 (pending)
---

Status: failed
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Action: UltraPlanRun("『data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。』の実行が失敗しました。まず output/diagnosis.md を作成し、調査の進展に応じて更新すること。原因を調査し、検証可能な再現手順と診断レポート（output/diagnosis.md）を作成してください。修正は行わないでください。")
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
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=0 retries=3 quality_warnings=0 quality_issues=4
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
Stop reason: investigation reproducer was not identified
Profile: data
Assurance: static (investigation_probe_not_run)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Failure kind: process_failure
