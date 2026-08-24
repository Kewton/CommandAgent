Build: b3d730e 2026-07-18T10:19:42Z
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
Contract origin: fix_intent_v0
Runtime acceptance: failed
Final acceptance: failed
Release gate: not_applicable
Requested port: missing
Evidence arbitration: missing
Depth profile: missing
completion_contract_verification_enabled=true
completion_contract_path_merge_enabled=false
completion_contract_path=docs/fix-intent-contract.md
completion_contract_generated=false
external_contract_checked=true
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
Stop reason: phase repair failed: path does not exist: docs/fix-intent-contract.md; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
Profile: data
Assurance: failed (after_not_executed)
Unverified (probe required):
- after_passes:not_executed
- no_regression:not_executed
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Recovery handoff:
- Recovery prompt saved: .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
- Suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md)"
- Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
Failure kind: direct_cli_command_failed
TUI command failed: phase repair failed: path does not exist: docs/fix-intent-contract.md; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true

Time profile: provider 100% [prefill 17% · generation 82% · load 1%] · installs 0% · builds 0% · probe 0% · total 13s

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| isolate-cause | 3s | 3s | 0s | 0s | 0s | 0s |
| repair | 11s | 11s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 1027 | 13s | 4 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| tool-call | 1027 | 13s | 4 | 0B | 0B |

Completed phases:
- reproduce-before (completed)
- isolate-cause (completed)

Failed phases:
- repair (failed)

Pending phases:
- verify-regressions (pending)
---

Status: failed
Lifecycle: process
Process: REPL exited cleanly (not task status)
Session/REPL status: process_exited
Action: UltraPlanRun("output/results.json がデータ契約のスキーマ検証に失敗します。パイプラインを修正して正しい results.json を再生成し、既存検証が通ることを確認してください。")
Command status: failed
Command completion: failed
Task status: failed
Effective profile: data
Prompt layout: legacy
Contract origin: fix_intent_v0
Runtime acceptance: failed
Final acceptance: failed
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
Stop reason: phase repair failed: path does not exist: docs/fix-intent-contract.md; incomplete; Recovery artifact check: prompt_parse_ok=true, yaml_parse_ok=true, command_targets_valid=true
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
Profile: data
Assurance: failed (after_not_executed)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Recovery handoff:
- Recovery prompt saved: .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
- Suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-repair-019f74c2-0fea-79a2-bed7-efe658ad3bc2.md)"
- Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-repair-019f74c2-0fea-79a2-bed7-eff2e338cfe9.yaml
Failure kind: process_failure
