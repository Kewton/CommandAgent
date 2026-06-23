# Phase37 Row To Case Proof Matrix

Date: 2026-06-24 JST

Status: closed / current proof reconciled

## Purpose

This matrix closes Phase32 recovery task P32-R009 by making the proof chain
explicit:

```text
coverage row -> current or accepted proof -> proof root -> result -> disposition
```

It does not declare final migration completion. Phase38 still owns sign-off
root admission, and Phase39 still owns final closure reporting.

## Current Proof Roots

| family | root | current result used by Phase37 |
| --- | --- | --- |
| smoke | `eval/runs/current-all-local-llm/smoke/20260623T203030` | 3 cases, all success |
| focused/control-recovery | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` | 82 cases, `passed_recheck=82` |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` | 6 cases, all `closed_owned_failure` large dispositions |

Historical phase fixture roots remain regression evidence. They are not the
only closure proof for any adopted row that has a current case.

## Disposition Summary

| disposition | rows |
| --- | ---: |
| `current_eval_proven` | 44 |
| `unit_or_fixture_proven` | 1 |
| `excluded_with_rationale` | 9 |
| `proof_gap` | 0 |
| `split_forward` | 0 |

## Coverage Row Matrix

| coverage id | responsibility | status / adoption | owning proof phase | current proof binding | proof root / authority | disposition |
| --- | --- | --- | --- | --- | --- | --- |
| C01 | Task contract core | Implemented / Adopt | Phase22 | `focused-task-contract-admission`, `focused-behavior-obligation-projection`, `focused-plan-parser-block-scalar-chomp` | current focused root plus Phase22 task-contract tests | `current_eval_proven` |
| C02 | Task contract inference and admission | Implemented / Partial | Phase22 | `focused-task-contract-admission`, `focused-plan-parser-block-scalar-chomp` | current focused root plus Phase22 request/admission tests | `current_eval_proven` |
| C03 | Objective and behavior contract projection | Implemented / Adopt | Phase22 | `focused-behavior-obligation-projection`, Next.js behavior/setup focused rows | current focused root plus Phase22 behavior-obligation tests | `current_eval_proven` |
| C04 | Artifact role taxonomy | Implemented / Adopt | Phase23 | `focused-artifact-role-scope-ownership` | current focused root plus Phase23 profile-artifact tests | `current_eval_proven` |
| C05 | Task workspace scope | Implemented / Adopt | Phase23 | `focused-artifact-role-scope-ownership`, `focused-out-of-scope-target-rejection` | current focused root plus Phase23 workspace-scope tests | `current_eval_proven` |
| C06 | Artifact ownership | Implemented / Adopt | Phase23 | `focused-artifact-role-scope-ownership`, `focused-out-of-scope-target-rejection` | current focused root plus Phase23 ownership/target tests | `current_eval_proven` |
| C07 | Artifact ledger | Implemented / Adopt | Phase24 | `focused-artifact-ledger-producers` | current focused root plus Phase24 artifact-ledger tests | `current_eval_proven` |
| C08 | Completion evidence | Implemented / Adopt | Phase24 | `focused-completion-evidence-producers`, `focused-missing-evidence` | current focused root plus Phase24 completion-evidence tests | `current_eval_proven` |
| C09 | Evidence binding | Implemented / Adopt | Phase24 | `focused-evidence-binding-producers`, `focused-evidence-binding-failure`, language binding rows | current focused root plus Phase24 evidence-binding tests | `current_eval_proven` |
| C10 | Deliverable obligation audit | Implemented / Adopt | Phase24 | `focused-deliverable-obligation-freshness` | current focused root plus Phase24 deliverable-obligation tests | `current_eval_proven` |
| C11 | Active job arbiter | Implemented / Adopt | Phase25 | all `focused-dispatch-*` rows | current focused root plus Phase25 active-job tests | `current_eval_proven` |
| C12 | Recovery owner / dispatch gate | Implemented / Adopt | Phase25 | all `focused-dispatch-*` rows | current focused root plus Phase25 recovery-orchestration tests | `current_eval_proven` |
| C13 | Recovery messages and packets | Implemented / Partial | Phase26 | `focused-phase26-safe-stop-evidence-binding`, recovery safe-stop focused rows | current focused root plus Phase26 recovery-task tests | `current_eval_proven` |
| C14 | Setup bootstrap | Implemented / Partial | Phase26 | setup, manifest, and dependency focused rows | current focused root plus Phase26 setup lifecycle tests | `current_eval_proven` |
| C15 | Project probe/profile/scaffold profile | Implemented / Partial | Phase26 | profile/scaffold and Next.js profile focused rows | current focused root plus Phase26 profile output tests | `current_eval_proven` |
| C16 | Profile failure to recovery job | Implemented / Partial | Phase26 | `focused-phase26-profile-failure-mapping`, Next.js route/manifest focused rows | current focused root plus Phase26 profile mapping tests | `current_eval_proven` |
| C17 | Semantic failure report | Implemented / Adopt | Phase26 | `focused-phase26-semantic-conflict-object`, verifier diagnostic rows, large verifier rows | current focused and large roots plus Phase26 semantic-failure tests | `current_eval_proven` |
| C18 | Semantic repair plan | Implemented / Adopt | Phase26 | `focused-phase26-semantic-repair-cluster-exhaustion` | current focused root plus Phase26 repair-state tests | `current_eval_proven` |
| C19 | Repair brief | Implemented / Adopt | Phase26 | `focused-phase26-repair-brief-rendering` | current focused root plus Phase26 repair-brief tests | `current_eval_proven` |
| C20 | Repair action space | Implemented / Adopt | Phase26 | `focused-phase26-action-envelope-admission`, `focused-phase26-action-envelope-rejection` | current focused root plus Phase26 action-envelope tests | `current_eval_proven` |
| C21 | Repair target decision/admission | Implemented / Adopt | Phase27 | `phase27-target-admission-matrix`, stale/out-of-scope/large target rows | current focused and large roots plus Phase27 target-admission tests | `current_eval_proven` |
| C22 | Repair target prioritization | Implemented / Adopt | Phase27 | `phase27-target-priority-tie-stop` | current focused root plus Phase27 target-priority tests | `current_eval_proven` |
| C23 | Repair job state machine | Implemented / Adopt | Phase27 | `phase27-repair-lifecycle-rerun` | current focused root plus Phase27 repair-job tests | `current_eval_proven` |
| C24 | Repair attempt ledger | Implemented / Adopt | Phase27 | `phase27-attempt-ledger-outcomes` | current focused root plus Phase27 attempt-ledger tests | `current_eval_proven` |
| C25 | No-progress recovery | Implemented / Adopt | Phase27 | `phase27-no-progress-deferral`, `focused-no-progress-target-switch` | current focused root plus Phase27 no-progress tests | `current_eval_proven` |
| C26 | Verifier diagnostic assessment | Implemented / Adopt | Phase27 | `phase27-verifier-diagnostic-assessment`, language verifier rows, large verifier rows | current focused and large roots plus Phase27 verifier-diagnostic tests | `current_eval_proven` |
| C27 | Verifier orchestration | Implemented / Adopt | Phase27 | `phase27-verifier-orchestration-safe-stop`, `focused-nextjs-endpoint-smoke` | current focused root plus Phase27 verifier orchestration tests | `current_eval_proven` |
| C28 | Verifier command policy | Implemented / Adopt | Phase27 | `phase27-verifier-command-policy`, `focused-generated-test-weakening-rejection` | current focused root plus Phase27 verifier policy tests | `current_eval_proven` |
| C29 | Artifact completion job | Implemented / Adopt | Phase27 | `phase27-artifact-completion-job`, `focused-missing-artifact-completion` | current focused root plus Phase27 artifact-completion tests | `current_eval_proven` |
| C30 | Focused edit recovery | Implemented / Adopt | Phase27 | `phase27-focused-edit-stale-rejection`, `focused-edit-recovery`, `focused-stale-edit-target` | current focused root plus Phase27 focused-edit tests | `current_eval_proven` |
| C31 | Forced small edit / deterministic fallback | Implemented / Adopt | Phase27 | `phase27-mechanical-fallback-admission` | current focused root plus Phase27 mechanical-repair tests | `current_eval_proven` |
| C32 | Repair patch executor/validation | Implemented / Adopt | Phase27 | `phase27-patch-validation-rollback` | current focused root plus Phase27 patch-validation tests | `current_eval_proven` |
| C33 | Contract conflict job | Implemented / Adopt | Phase28 | all `phase28-*` rows plus `focused-contract-conflict-explicit-stop` | current focused root plus Phase28 contract-conflict tests | `current_eval_proven` |
| C34 | Language-specific mechanical repair | Implemented / Adopt | Phase29 | `phase29-language-repair-adapter`, Python/Rust language focused rows, large language rows | current focused and large roots plus Phase29 runtime-support tests | `current_eval_proven` |
| C35 | Tool policy and effective policy | Implemented / Adopt | Phase29 | `phase29-effective-tool-policy`, large tool-policy evidence where present | current focused and large roots plus Phase29 tool policy tests | `current_eval_proven` |
| C36 | Tool failure recovery | Implemented / Adopt | Phase29 | `phase29-tool-failure-recovery`, `focused-tool-protocol-missing-write-path`, large tool-protocol rows | current focused and large roots plus Phase29 tool failure tests | `current_eval_proven` |
| C37 | Bash/setup command classification | Implemented / Adopt | Phase29 | `phase29-setup-command-classification`, setup-policy large evidence where present | current focused and large roots plus Phase29 command classification tests | `current_eval_proven` |
| C38 | Workspace candidates/walk | Implemented / Adopt | Phase29 | `phase29-workspace-candidate-policy` | current focused root plus Phase29 workspace snapshot tests | `current_eval_proven` |
| C39 | Job report / progress events | Implemented / Adopt | Phase29 | `phase29-job-report` | current focused root plus Phase29 eval report tests | `current_eval_proven` |
| C40 | Scaffold pipeline | Implemented / Adopt | Phase29 | `phase29-scaffold-contract`, `focused-phase26-profile-scaffold-facts` | current focused root plus Phase29 scaffold proof | `current_eval_proven` |
| C41 | Data/docs/research/ops evidence | Implemented / Adopt | Phase29 | `phase29-noncoding-evidence`, `focused-data-schema-completion`, smoke docs case | current focused and smoke roots plus Phase29 noncoding evidence proof | `current_eval_proven` |
| C42 | Answer-only and work-mode gating | Implemented / Adopt | Phase29 | `phase29-answer-work-mode` | current focused root plus Phase29 answer/work-mode tests | `current_eval_proven` |
| C43 | Interruption, lifecycle, turn state | Implemented / Adopt | Phase29 | `phase29-lifecycle-projection` | current focused root plus Phase29 lifecycle projection proof | `current_eval_proven` |
| C44 | Provider/model request plumbing | Implemented / Adopt | Phase29 | `phase29-provider-boundary` | current focused root plus provider-boundary docs/tests | `current_eval_proven` |
| C45 | Provider transport parser | Implemented / Adopt | Provider boundary | no direct current case; parser proof is unit/fixture transport proof plus C36/C44 regression coverage | provider parser tests/docs and existing Gemini/native parser proof | `unit_or_fixture_proven` |
| C46 | Working memory/reminders | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |
| C47 | Case record and anti-pattern corpora | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |
| C48 | PAM/Photon advisory | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |
| C49 | Quality classification/confirmation | Excluded / Excluded | Phase30 | no current case required | Phase30 exclusion decision and coverage rationale | `excluded_with_rationale` |
| C50 | Slash/plan/command UI helpers | Excluded / Excluded | Phase30 | no current case required | Phase30 exclusion decision and coverage rationale | `excluded_with_rationale` |
| C51 | Legacy engine selector | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |
| C52 | Hidden or unbounded repair loop | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |
| C53 | Provider/model-specific behavioral policy | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |
| C54 | Model-issued dependency installation | Excluded / Excluded | Coverage decision | no current case required | coverage-table exclusion rationale | `excluded_with_rationale` |

## Current Eval Case Binding

The current eval surface contains 91 cases. Phase37 binds them as grouped
case sets below. Each case id is either mapped to C rows or marked as
supplemental regression evidence for the current root set.

| current case set | count | binding |
| --- | ---: | --- |
| `smoke-docs-readme`, `smoke-python-script`, `smoke-rust-cli` | 3 | Supplemental smoke regression for C34/C41/C44 and overall current-root health. |
| `focused-artifact-ledger-producers`, `focused-artifact-role-scope-ownership`, `focused-behavior-obligation-projection`, `focused-completion-evidence-producers`, `focused-contract-conflict-explicit-stop`, `focused-data-schema-completion`, `focused-deliverable-obligation-freshness`, `focused-docs-literal-mismatch`, `focused-edit-recovery`, `focused-evidence-binding-failure`, `focused-evidence-binding-producers`, `focused-generated-test-weakening-rejection`, `focused-missing-artifact-completion`, `focused-missing-evidence`, `focused-no-progress-target-switch`, `focused-out-of-scope-target-rejection`, `focused-plan-parser-block-scalar-chomp`, `focused-python-fastapi-assertion-mismatch`, `focused-python-import-binding`, `focused-python-missing-test-artifact`, `focused-rust-cargo-verifier-binding`, `focused-rust-compile-diagnostic-target`, `focused-setup-manifest-invalid`, `focused-stale-edit-target`, `focused-task-contract-admission`, `focused-tool-protocol-missing-write-path` | 26 | Direct current focused proof for C01-C10, C13, C21, C25-C30, C33-C34, C36, and C41. |
| `focused-nextjs-dependency-setup`, `focused-nextjs-dev-server-port-conflict`, `focused-nextjs-endpoint-smoke`, `focused-nextjs-manifest-repair`, `focused-nextjs-route-integration`, `focused-nextjs-tailwind-manifest-drift` | 6 | Direct current focused proof for C03, C14-C16, C20-C21, C27, and setup/profile/dev-server surfaces closed by Phase35. |
| `focused-dispatch-ambiguous-tie-stop`, `focused-dispatch-docs-literal`, `focused-dispatch-evidence-binding`, `focused-dispatch-manifest-repair`, `focused-dispatch-no-owner-stop`, `focused-dispatch-route-integration`, `focused-dispatch-setup-bootstrap`, `focused-dispatch-source-diagnostic`, `focused-dispatch-tool-protocol`, `focused-dispatch-verifier-contract` | 10 | Direct current focused proof for C11-C12 dispatch and owner/action selection. |
| `focused-phase26-action-envelope-admission`, `focused-phase26-action-envelope-rejection`, `focused-phase26-profile-failure-mapping`, `focused-phase26-profile-scaffold-facts`, `focused-phase26-repair-brief-rendering`, `focused-phase26-safe-stop-evidence-binding`, `focused-phase26-semantic-conflict-object`, `focused-phase26-semantic-repair-cluster-exhaustion`, `focused-phase26-setup-node-readiness`, `focused-phase26-setup-python-import`, `focused-phase26-setup-rust-manifest` | 11 | Direct current focused proof for C13-C20. |
| `phase27-artifact-completion-job`, `phase27-attempt-ledger-outcomes`, `phase27-focused-edit-stale-rejection`, `phase27-mechanical-fallback-admission`, `phase27-no-progress-deferral`, `phase27-patch-validation-rollback`, `phase27-repair-lifecycle-rerun`, `phase27-target-admission-matrix`, `phase27-target-priority-tie-stop`, `phase27-verifier-command-policy`, `phase27-verifier-diagnostic-assessment`, `phase27-verifier-orchestration-safe-stop` | 12 | Direct current focused proof for C21-C32. |
| `phase28-ambiguous-authority-safe-stop`, `phase28-docs-api-vs-source`, `phase28-phase27-no-progress-handoff`, `phase28-source-vs-generated-test`, `phase28-source-vs-preexisting-test`, `phase28-weak-verifier-contract` | 6 | Direct current focused proof for C33. |
| `phase29-answer-work-mode`, `phase29-effective-tool-policy`, `phase29-job-report`, `phase29-language-repair-adapter`, `phase29-lifecycle-projection`, `phase29-noncoding-evidence`, `phase29-provider-boundary`, `phase29-scaffold-contract`, `phase29-setup-command-classification`, `phase29-tool-failure-recovery`, `phase29-workspace-candidate-policy` | 11 | Direct current focused proof for C34-C44. |
| `large-fastapi-app-modify`, `large-fastapi-app-new`, `large-nextjs-app-modify`, `large-nextjs-app-new`, `large-rust-app-modify`, `large-rust-app-new` | 6 | Supplemental large regression proof for C17, C21, C26, C34-C37 and Phase36 large ownership/disposition closure. |

## Historical Omission Reconciliation

The 44 current cases omitted from the earlier Phase32 accepted sign-off roots
are no longer hidden:

- Phase24 omitted focused cases are represented under C07-C10.
- Phase25 omitted dispatch cases are represented under C11-C12.
- Phase26 omitted recovery/setup/profile cases are represented under C13-C20.
- Phase27 omitted target/verifier/repair cases are represented under C21-C32.
- Phase28 omitted conflict cases are represented under C33.
- Current large cases are represented as supplemental regression proof and
  Phase36 row dispositions.

No adopted coverage row remains closed only by historical roots that omitted a
current case.

## Phase37 Closure

| gate | result |
| --- | --- |
| C01-C54 represented | pass |
| C01-C45 adopted rows have current or accepted proof | pass |
| C46-C54 excluded rows have rationale | pass |
| 91 current cases mapped or supplemental | pass |
| historical-only adopted row closure | none |
| open `proof_gap` rows | 0 |
