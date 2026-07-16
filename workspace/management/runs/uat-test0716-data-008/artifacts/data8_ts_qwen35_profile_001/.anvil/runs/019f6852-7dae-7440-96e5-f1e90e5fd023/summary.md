Build: fcb9ac8 2026-07-16T00:25:36Z
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
Planner release risk: true
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=16
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
Stop reason: phase data-cleaning failed: artifact recovery exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
Profile: data
Assurance: static (data_profile_probe_not_run)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Recovery handoff:
- Recovery prompt saved: .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
- Suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md)"
- Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
Failure kind: direct_cli_command_failed
TUI command failed: phase data-cleaning failed: artifact recovery exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true

Time profile: provider 100% [prefill 2% · generation 98% · load 1%] · installs 0% · builds 0% · probe 0% · total 20m37s

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| data-cleaning | 8m05s | 8m05s | 0s | 0s | 0s | 0s |
| data-inspection | 12m32s | 12m32s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 30029 | 5m23s | 11 |
| planner | 8790 | 4m33s | 2 |
| repair | 59545 | 10m41s | 12 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| full-file Write | 10317 | 1m51s | 5 | 6088B | 0B |
| prose-only | 82518 | 17m43s | 11 | 0B | 0B |
| tool-call | 5529 | 1m04s | 9 | 0B | 0B |

Completed phases:
- data-inspection (completed)

Failed phases:
- data-cleaning (failed)

Pending phases:
- data-aggregation (pending)
- data-reporting (pending)
- data-validation (pending)
---

Status: failed
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Action: UltraPlanRun("data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。")
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
Planner release risk: true
Planner diagnostics: normalizations=0 retries=0 quality_warnings=0 quality_issues=16
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
Stop reason: phase data-cleaning failed: artifact recovery exhausted; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
Profile: data
Assurance: static (data_profile_probe_not_run)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Recovery handoff:
- Recovery prompt saved: .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
- Suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-data-cleaning-019f6865-5d31-7fb3-bc81-69357b582c7b.md)"
- Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-data-cleaning-019f6865-5d31-7fb3-bc81-6947a191db26.yaml
Failure kind: process_failure
