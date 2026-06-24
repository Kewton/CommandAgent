# Phase 20 Coverage Closure

Date: 2026-06-23 JST

## Summary

Phase20 reconciled the current legacy-control coverage table against the
Phase1-Phase19 implementation and eval reports.

The broad sign-off proof is green, but the coverage table itself still has
adopted responsibilities in `Partial` or `Missing` state. Under the recovery
plan, those rows cannot be converted to `Implemented` without row-specific
proof. Therefore Phase20 cannot declare pure migration completion.

## Counts

Current implementation status in
`docs/eval/legacy-control-stack-coverage-20260621.md`:

| Status | Count |
| --- | ---: |
| Implemented | 1 |
| Partial | 44 |
| Missing | 2 |
| Excluded | 7 |

Adoption decision:

| Adoption decision | Count |
| --- | ---: |
| Adopt | 33 |
| Partial | 12 |
| Missing | 2 |
| Excluded | 7 |

Phase20 closure candidates:

| Candidate | Count |
| --- | ---: |
| Implemented | 1 |
| Excluded | 7 |
| Unresolved adopted/partial surface | 44 |
| Unresolved non-adopted priority-decision surface | 2 |

## Closure Table

| id | source mechanism | prior status | adoption decision | final status candidate | evidence source | owner layer | final decision impact |
| --- | --- | --- | --- | --- | --- | --- | --- |
| C01 | Task contract core | Partial | Adopt | Unresolved | Coverage table says richer constraints, expected completion evidence, lifecycle state, and cross-command persistence remain. Phase1 reports projection work but not full parity. | Step runner plan schema, profiles, artifact graph, TaskContract projection | Blocks `migration_complete`. |
| C02 | Task contract inference and admission | Partial | Partial | Unresolved | Coverage table says deterministic task-kind/request signals remain where ambiguity causes wrong workflow. | Plan input, profiles, plan lint | Blocks `migration_complete`. |
| C03 | Objective and behavior contract projection | Partial | Adopt | Unresolved | Coverage table says richer completion checks and behavior-delta obligations remain beyond deterministic path/profile facts. | Plan prompt, plan lint, profile verification, TaskContract projection | Blocks `migration_complete`. |
| C04 | Artifact role taxonomy | Partial | Adopt | Unresolved | Coverage table says role SSOT still needs unification across profile verification, verifier repair, and recovery admission. | ArtifactGraph / profiles / TaskContract projection | Blocks `migration_complete`. |
| C05 | Task workspace scope | Partial | Adopt | Unresolved | Coverage table says persistent task-scope admission and richer profile-selected root handling remain. | Safety/path confinement, recovery contract label | Blocks `migration_complete`. |
| C06 | Artifact ownership | Partial | Adopt | Unresolved | Coverage table says ownership decisions still need more completion-evidence producers and repeated-target exclusion. | ArtifactGraph / recovery contract | Blocks `migration_complete`. |
| C07 | Artifact ledger | Partial | Adopt | Unresolved | Coverage table says focused eval cases and stronger pass-side completion authority remain. | Minimal loop result / step runner evidence | Blocks `migration_complete`. |
| C08 | Completion evidence | Partial | Adopt | Unresolved | Coverage table says richer docs/data/report/profile-wide producers and deeper profile-specific bindings remain. | Step verifier, final-answer guard, eval | Blocks `migration_complete`. |
| C09 | Evidence binding | Partial | Adopt | Unresolved | Coverage table says concrete producers for manifest identity, docs section, schema output, source citation, and route/import binding checks remain. | Verifier/profile/setup | Blocks `migration_complete`. |
| C10 | Deliverable obligation audit | Partial | Adopt | Unresolved | Coverage table says obligation projection into plan/profile/eval producers and read-only freshness checks remain. | Plan lint / eval / profile | Blocks `migration_complete`. |
| C11 | Active job arbiter | Partial | Adopt | Unresolved | Coverage table says lifecycle state and deeper attempt-progress transitions remain. | Recovery orchestration | Blocks `migration_complete`. |
| C12 | Recovery owner / dispatch gate | Partial | Adopt | Unresolved | Coverage table says broader focused E2E proof and remaining profile-specific candidate producers remain. | Recovery orchestration | Blocks `migration_complete`. |
| C13 | Recovery messages and packets | Partial | Partial | Unresolved | Coverage table says safe-stop payload coverage for evidence binding and completion-authority failures remains. | Recovery task / repair packet / final error | Blocks `migration_complete`. |
| C14 | Setup bootstrap | Partial | Partial | Unresolved | Coverage table says candidate-content validation, broader setup result ledger coverage, and non-Node setup policies remain partial. | Setup runtime / setup lifecycle / recovery orchestration | Blocks `migration_complete`. |
| C15 | Project probe/profile/scaffold profile | Partial | Partial | Unresolved | Coverage table says bounded scaffold materialization and completion evidence remain partial. | Profiles / artifact graph | Blocks `migration_complete`. |
| C16 | Profile failure to recovery job | Partial | Partial | Unresolved | Coverage table says fuller typed failure-specific mapping across profiles remains. | Recovery orchestration | Blocks `migration_complete`. |
| C17 | Semantic failure report | Partial | Adopt | Unresolved | Coverage table says conflict objects, richer cluster target ranking, and broader live eval evidence remain. | Evidence / recovery contract | Blocks `migration_complete`. |
| C18 | Semantic repair plan | Partial | Adopt | Unresolved | Coverage table says deeper cluster exhaustion and role-strategy transitions from live repair outcomes remain. | Recovery task contract | Blocks `migration_complete`. |
| C19 | Repair brief | Partial | Adopt | Unresolved | Coverage table says broader E2E evidence is still needed. | Recovery task contract | Blocks `migration_complete`. |
| C20 | Repair action space | Partial | Adopt | Unresolved | Coverage table says lifecycle transitions and broader live eval evidence for all action families remain. | Recovery orchestration | Blocks `migration_complete`. |
| C21 | Repair target decision/admission | Partial | Adopt | Unresolved | Coverage table says broader live eval evidence across route/source/test/docs/setup/evidence-binding remains. | Recovery orchestration / ArtifactGraph / ArtifactLedger / TargetAdmission | Blocks `migration_complete`. |
| C22 | Repair target prioritization | Partial | Adopt | Unresolved | Coverage table says more failure-kind-specific ranking and authority-based decisions remain. | Recovery orchestration / TargetAdmission | Blocks `migration_complete`. |
| C23 | Repair job state machine | Partial | Adopt | Unresolved | Coverage table says broader live eval evidence and deeper lifecycle transition reporting remain. | Repair loop / recovery task | Blocks `migration_complete`. |
| C24 | Repair attempt ledger | Partial | Adopt | Unresolved | Coverage table says broader focused live eval coverage across profile families remains. | Recovery task / repair loop | Blocks `migration_complete`. |
| C25 | No-progress recovery | Partial | Adopt | Unresolved | Coverage table says live eval still needs to prove strategy branches across profile families. | Repair loop / target admission | Blocks `migration_complete`. |
| C26 | Verifier diagnostic assessment | Partial | Adopt | Unresolved | Coverage table says deeper language-specific assessment, richer weak target filters, and verifier attempt flow parity remain. | Verifier / evidence | Blocks `migration_complete`. |
| C27 | Verifier orchestration | Partial | Adopt | Unresolved | Coverage table says verifier repair flow, job attempt limits, rerun outcome events, binding scope, and safe-stop report remain. | Step runner verifier / repair loop | Blocks `migration_complete`. |
| C28 | Verifier command policy | Partial | Adopt | Unresolved | Coverage table says generated-test preflight, self-referential verifier detection, unsupported assertion filtering, and expectation audit remain. | Plan lint / verifier selection | Blocks `migration_complete`. |
| C29 | Artifact completion job | Partial | Adopt | Unresolved | Coverage table says binding completion to artifact ledger, ownership, freshness, and missing-evidence distinction remains. | ArtifactCompletionJob / recovery orchestration | Blocks `migration_complete`. |
| C30 | Focused edit recovery | Partial | Adopt | Unresolved | Coverage table says live focused eval still needs to prove model repair behavior after admission. | TargetAdmission / ArtifactLedger eval fields | Blocks `migration_complete`. |
| C31 | Forced small edit / deterministic fallback | Partial | Adopt | Unresolved | Coverage table says deterministic fallback edit execution remains absent until patch admission and rollback data can prove safe mutation. | MechanicalRepairAdapter / RecoveryTaskContract | Blocks `migration_complete`. |
| C32 | Repair patch executor/validation | Partial | Adopt | Unresolved | Coverage table says direct patch executor parity and verifier-proven rollback execution remain partial. | PatchValidation Contract / repair loop / attempt ledger | Blocks `migration_complete`. |
| C33 | Contract conflict job | Missing | Adopt | Unresolved | Coverage table says conflict object, source-of-truth decision, spec authority, and ambiguous-authority safe stop are missing. | None | Blocks `migration_complete`. |
| C34 | Language-specific mechanical repair | Partial | Adopt | Unresolved | Coverage table says broader language-specific repair families and live eval proof remain. | MechanicalRepairAdapter / verifier diagnostic payload | Blocks `migration_complete`. |
| C35 | Tool policy and effective policy | Partial | Adopt | Unresolved | Coverage table says broader E2E coverage and later lifecycle transitions remain. | Minimal loop guards, step policy, recovery task | Blocks `migration_complete`. |
| C36 | Tool failure recovery | Partial | Adopt | Unresolved | Coverage table says broader live E2E coverage for provider parse and stale-edit branches remains. | Provider parser, minimal loop guards, recovery task | Blocks `migration_complete`. |
| C37 | Bash/setup command classification | Partial | Partial | Unresolved | Coverage table says evidence binding and setup command authority remain before broader setup recovery. | Bash tool, verifier, setup runtime | Blocks `migration_complete`. |
| C38 | Workspace candidates/walk | Partial | Adopt | Unresolved | Coverage table says scope-aware workspace walk, ignored-dir SSOT, and candidate discovery remain. | Eval/workspace scans, ArtifactGraph inputs | Blocks `migration_complete`. |
| C39 | Job report / progress events | Partial | Partial | Unresolved | Coverage table says job-level report schema, active owner, repair action plan status, and attempt outcome events remain. | Runtime events / eval reports | Blocks `migration_complete`. |
| C40 | Scaffold pipeline | Partial | Partial | Unresolved | Coverage table says scaffold profile remains setup/artifact contract work, not complete parity. | Profiles / plan artifacts | Blocks `migration_complete`. |
| C41 | Data/docs/research/ops evidence | Partial | Partial | Unresolved | Coverage table says generic evidence binding/completion evidence remains before profile-specific expansion. | Profiles / eval | Blocks `migration_complete`. |
| C42 | Answer-only and work-mode gating | Partial | Partial | Unresolved | Coverage table says this should remain policy-gated and not broaden into normal coding repair. | Minimal loop final-answer guard / step policy | Blocks `migration_complete`. |
| C43 | Interruption, lifecycle, turn state | Partial | Partial | Unresolved | Coverage table says full actor-loop complexity should not be ported, but explicit recovery-state needs remain. | CLI/repl/minimal loop/session | Blocks `migration_complete`. |
| C44 | Provider/model request plumbing | Partial | Partial | Unresolved | Coverage table says transport/prompt should remain clean and recovery policy should stay outside provider branches. | Providers / minimal loop prompt | Blocks `migration_complete`. |
| C45 | Provider transport parser | Implemented | Adopt | Implemented | Coverage table marks this as implemented; provider native parsing is separated from planning/recovery and XML remains fallback. | Providers | Compatible with `migration_complete`. |
| C46 | Working memory/reminders | Excluded | Excluded | Excluded | Coverage table records exclusion because memory/advisory systems are outside the MVP unless a separate design decision admits them. | None | Compatible with `migration_complete_with_explicit_exclusions`. |
| C47 | Case record and anti-pattern corpora | Excluded | Excluded | Excluded | Coverage table records exclusion because case-memory/advisory retrieval is outside the MVP. | None | Compatible with `migration_complete_with_explicit_exclusions`. |
| C48 | PAM/Photon advisory | Excluded | Excluded | Excluded | Coverage table records exclusion because advisory sidecars would reintroduce a separate control stack. | None | Compatible with `migration_complete_with_explicit_exclusions`. |
| C49 | Quality classification/confirmation | Missing | Missing | Unresolved | Coverage table marks this as lower-priority quality-gate work needing a priority decision. | Eval/profile/final guard | Outside accepted migration surface until a priority decision is made. |
| C50 | Slash/plan/command UI helpers | Partial | Missing | Unresolved | Coverage table says this is not a recovery-parity target unless UX/eval evidence shows a gap. | CLI/repl/slash command | Outside accepted migration surface until a priority decision is made. |
| C51 | Legacy engine selector | Excluded | Excluded | Excluded | Coverage table records exclusion because CommandAgent has one execution engine. | None | Compatible with `migration_complete_with_explicit_exclusions`. |
| C52 | Hidden or unbounded repair loop | Excluded | Excluded | Excluded | Coverage table records exclusion because repair must remain bounded and user-visible. | None | Compatible with `migration_complete_with_explicit_exclusions`. |
| C53 | Provider/model-specific behavioral policy | Excluded | Excluded | Excluded | Coverage table records exclusion because shared behavior must stay outside provider/model-specific branches. | None | Compatible with `migration_complete_with_explicit_exclusions`. |
| C54 | Model-issued dependency installation | Excluded | Excluded | Excluded | Coverage table records exclusion because setup must be explicit policy/evidence, not implicit model-issued dependency installation. | None | Compatible with `migration_complete_with_explicit_exclusions`. |

## Decision Impact

Phase20 cannot declare `migration_complete` or
`migration_complete_with_explicit_exclusions` from the current table because
44 rows in the accepted migration surface remain unresolved. Two additional
rows remain outside the accepted migration surface but still require a priority
decision before a stronger parity claim can be made.

The correct final decision is therefore `migration_not_complete`.
