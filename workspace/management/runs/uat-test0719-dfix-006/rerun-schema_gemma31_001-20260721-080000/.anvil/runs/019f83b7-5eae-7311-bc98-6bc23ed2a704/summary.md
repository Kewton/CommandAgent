Build: 4127673 2026-07-21T07:47:54Z
Status: interrupted
Completion status: incomplete
Lifecycle: tui_command
Process: interrupted
Session/REPL status: repl_ready
Command: --ultra-plan-run
Command status: interrupted
Command completion: interrupted
Task status: interrupted
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
Next action: resume_or_rerun_command
Recovery next action: resume_or_rerun_command
Stop reason: interrupted by user
Profile: data
Assurance: static (data_profile_probe_not_run)
Host env: NODE_ENV=production detected (verifiers ran with a cleaned environment)
Failure kind: direct_cli_command_interrupted

Time profile: provider 100% [prefill 9% · generation 85% · load 7%] · installs 0% · builds 0% · probe 0% · total 1m24s

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| isolate-cause | 15s | 15s | 0s | 0s | 0s | 0s |
| repair | 1m10s | 1m10s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 1898 | 1m24s | 8 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| tool-call | 1898 | 1m24s | 8 | 0B | 0B |

Completed phases:
- reproduce-before (completed)
- isolate-cause (completed)

Failed phases:
- repair (interrupted)

Pending phases:
- phase 4 (pending)
