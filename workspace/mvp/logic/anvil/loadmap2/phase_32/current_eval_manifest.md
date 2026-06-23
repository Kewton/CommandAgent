# Phase32 Current Eval Manifest

Date: 2026-06-23 JST

Status: recovery evidence / current eval reconciliation failed

## Purpose

This file records the fresh current eval surface that invalidated the previous
Phase32 completion decision.

The Phase32 closure used historical accepted proof roots. Those roots covered
47 unique cases. A fresh current eval run covered 91 cases. The 44-case gap
means Phase32 cannot declare migration completion until the current case set is
reconciled.

## Current Eval Roots

| family | root | result |
| --- | --- | --- |
| smoke | `eval/runs/current-all-local-llm/smoke/20260623T203030` | 3/3 success |
| small | `eval/runs/current-all-local-llm/small/20260623T203216` | 0/0, no YAML cases |
| focused/control-recovery | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` | 9/82 success |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` | 0/6 success |

Local LLM:

```text
provider=ollama
model=qwen3.6:27b-coding-nvfp4
```

## Recheck Result

| family | recheck result | current blocker summary |
| --- | ---: | --- |
| smoke | 3/3 success | none |
| small | 0/0 | no cases |
| focused/control-recovery | 9/82 success | 35 focused assertion failures remain after fixture-field recheck repair; raw `rc:1`/diagnostic coverage still appears in 40 focused rows |
| large | 0/6 success | 6 real LLM rows fail; `large-rust-app-new` still has raw `rc_1` and missing target evidence |

Current broad signoff using these roots:

```text
status: fail
```

## Accepted Root Coverage Gap

| metric | count |
| --- | ---: |
| Unique cases in previous accepted Phase32 signoff roots | 47 |
| Unique cases in current eval roots | 91 |
| Current cases missing from previous accepted signoff roots | 44 |

## Cases Missing From Previous Phase32 Signoff

| case | matrix row | proof mode | current category | current terminal state |
| --- | --- | --- | --- | --- |
| `focused-artifact-ledger-producers` | `phase24-c07-artifact-ledger-producers` | `deterministic_fixture` | `ok` | `ok` |
| `focused-artifact-role-scope-ownership` | `artifact-role-scope-ownership` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `focused-completion-evidence-producers` | `phase24-c08-completion-evidence-producers` | `deterministic_fixture` | `quality` | `missing_evidence` |
| `focused-deliverable-obligation-freshness` | `phase24-c10-deliverable-obligation-freshness` | `deterministic_fixture` | `quality` | `stale_evidence` |
| `focused-dispatch-ambiguous-tie-stop` | `phase25-dispatch-ambiguous-tie-stop` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `focused-dispatch-docs-literal` | `phase25-dispatch-docs-literal` | `deterministic_fixture` | `quality` | `eval_assertion_failed` |
| `focused-dispatch-evidence-binding` | `phase25-dispatch-evidence-binding` | `deterministic_fixture` | `quality` | `evidence_binding_failed` |
| `focused-dispatch-manifest-repair` | `phase25-dispatch-manifest-repair` | `deterministic_fixture` | `profile` | `profile_contract_failed` |
| `focused-dispatch-no-owner-stop` | `phase25-dispatch-no-owner-stop` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `focused-dispatch-route-integration` | `phase25-dispatch-route-integration` | `deterministic_fixture` | `profile` | `profile_contract_failed` |
| `focused-dispatch-setup-bootstrap` | `phase25-dispatch-setup-bootstrap` | `deterministic_fixture` | `setup` | `dependency_missing` |
| `focused-dispatch-source-diagnostic` | `phase25-dispatch-source-diagnostic` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `focused-dispatch-tool-protocol` | `phase25-dispatch-tool-protocol` | `deterministic_fixture` | `tool_protocol` | `tool_protocol_failed` |
| `focused-dispatch-verifier-contract` | `phase25-dispatch-verifier-contract` | `deterministic_fixture` | `planning` | `plan_lint_failed` |
| `focused-evidence-binding-producers` | `phase24-c09-evidence-binding-producers` | `deterministic_fixture` | `quality` | `evidence_binding_failed` |
| `focused-phase26-action-envelope-admission` | `phase26-c20-action-envelope-admission` | `deterministic_fixture` | `profile` | `profile_contract_failed` |
| `focused-phase26-action-envelope-rejection` | `phase26-c20-action-envelope-rejection` | `deterministic_fixture` | `tool_protocol` | `tool_protocol_failed` |
| `focused-phase26-profile-failure-mapping` | `phase26-c16-profile-failure-mapping` | `deterministic_fixture` | `profile` | `profile_contract_failed` |
| `focused-phase26-profile-scaffold-facts` | `phase26-c15-profile-scaffold-facts` | `deterministic_fixture` | `profile` | `profile_contract_failed` |
| `focused-phase26-repair-brief-rendering` | `phase26-c19-repair-brief-rendering` | `deterministic_fixture` | `profile` | `profile_contract_failed` |
| `focused-phase26-safe-stop-evidence-binding` | `phase26-c13-safe-stop-evidence-binding` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `focused-phase26-semantic-conflict-object` | `phase26-c17-semantic-conflict-object` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `focused-phase26-semantic-repair-cluster-exhaustion` | `phase26-c18-semantic-repair-cluster-exhaustion` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `focused-phase26-setup-node-readiness` | `phase26-c14-setup-node-readiness` | `deterministic_fixture` | `setup` | `dependency_missing` |
| `focused-phase26-setup-python-import` | `phase26-c14-setup-python-import` | `deterministic_fixture` | `setup` | `dependency_missing` |
| `focused-phase26-setup-rust-manifest` | `phase26-c14-setup-rust-manifest` | `deterministic_fixture` | `setup` | `setup_failed` |
| `phase27-artifact-completion-job` | `C29-artifact-completion` | `deterministic_fixture` | `quality` | `missing_evidence` |
| `phase27-attempt-ledger-outcomes` | `C24-attempt-ledger` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase27-focused-edit-stale-rejection` | `C30-focused-edit` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase27-mechanical-fallback-admission` | `C31-mechanical-fallback` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase27-no-progress-deferral` | `C25-no-progress` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase27-patch-validation-rollback` | `C32-patch-validation` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase27-repair-lifecycle-rerun` | `C23-repair-lifecycle` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase27-target-admission-matrix` | `C21-target-admission` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase27-target-priority-tie-stop` | `C22-target-priority` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase27-verifier-command-policy` | `C28-verifier-policy` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase27-verifier-diagnostic-assessment` | `C26-verifier-diagnostic` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase27-verifier-orchestration-safe-stop` | `C27-verifier-orchestration` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase28-ambiguous-authority-safe-stop` | `C33-ambiguous-authority-safe-stop` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase28-docs-api-vs-source` | `C33-docs-api-vs-source` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase28-phase27-no-progress-handoff` | `C33-phase27-no-progress-handoff` | `deterministic_fixture` | `unknown` | `explicit_stop` |
| `phase28-source-vs-generated-test` | `C33-source-vs-generated-test` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase28-source-vs-preexisting-test` | `C33-source-vs-preexisting-test` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |
| `phase28-weak-verifier-contract` | `C33-weak-verifier-contract` | `deterministic_fixture` | `verifier` | `verifier_command_failed` |

## Recovery Implication

Phase32 is not allowed to declare migration completion until this manifest gap
is closed. The previous accepted signoff roots remain historical evidence, but
they are no longer sufficient final closure proof.
