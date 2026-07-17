Build: a89db52 2026-07-17T13:35:25Z
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
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=0 retries=3 quality_warnings=1 quality_issues=12
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
Stop reason: phase isolate-cause failed: step verify-top-level-keys failed verification after bounded repair: command failed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" outcome: CommandFailed status: exit status: 1 elapsed_ms: 146 summary: command did not succeed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" stdout: stderr: ; Paths: -
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
Profile: data
Assurance: failed (after_not_executed)
Unverified (probe required):
- after_passes:not_executed
- no_regression:not_executed
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Recovery handoff:
- Recovery prompt saved: .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
- Suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md)"
- Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
Failure kind: direct_cli_command_failed
TUI command failed: phase isolate-cause failed: step verify-top-level-keys failed verification after bounded repair: command failed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" outcome: CommandFailed status: exit status: 1 elapsed_ms: 146 summary: command did not succeed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" stdout: stderr: ; Paths: -

Time profile: provider 100% [prefill 4% · generation 95% · load 1%] · installs 0% · builds 0% · probe 0% · total 9m23s

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| isolate-cause | 8m02s | 8m02s | 0s | 0s | 0s | 0s |
| reproduce-before | 1m22s | 1m22s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 689 | 46s | 5 |
| planner | 17397 | 8m37s | 4 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| prose-only | 17397 | 8m37s | 4 | 0B | 0B |
| tool-call | 689 | 46s | 5 | 0B | 0B |

Completed phases:
- reproduce-before (completed)

Failed phases:
- isolate-cause (failed)

Pending phases:
- repair (pending)
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
Planner repaired: true
Planner release risk: true
Planner diagnostics: normalizations=0 retries=3 quality_warnings=1 quality_issues=12
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
Stop reason: phase isolate-cause failed: step verify-top-level-keys failed verification after bounded repair: command failed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" outcome: CommandFailed status: exit status: 1 elapsed_ms: 146 summary: command did not succeed: python -c "import json,sys;d=json.load(open('output/results.json'));sys.exit(0 if 'reconciliation' in d and 'values' in d else 1)" stdout: stderr: ; Paths: -
Paths:
- repair prompt saved: .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
Commands:
- suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md)"
- suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
Profile: data
Assurance: failed (after_not_executed)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Recovery handoff:
- Recovery prompt saved: .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md
- Recovery UltraPlan YAML saved: .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
- Suggested command: /ultra-plan-run --profile data "$(cat .anvil/repairs/repair-phase-isolate-cause-019f707e-9d12-70b0-9936-a97afed76c34.md)"
- Suggested YAML command: /run-ultra-plan .anvil/plans/recovery-ultra-plan-phase-isolate-cause-019f707e-9d12-70b0-9936-a98410d49688.yaml
Failure kind: process_failure
