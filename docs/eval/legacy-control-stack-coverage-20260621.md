# Legacy Control Stack Coverage - 2026-06-21

This note is the current coverage map between the source repository's recovery
control stack and CommandAgent. It intentionally separates three questions:

1. Which responsibility exists in the source stack?
2. Which CommandAgent layer currently owns that responsibility?
3. Is the responsibility fully implemented, partially projected, missing, or
   intentionally excluded?

## Terminology

"Contract projection" means CommandAgent stores and renders explicit contract
data for the already-detected failure: active job, target, allowed change kind,
source of truth, evidence delta, scope, ownership, and rerun authority.

It does not mean the mechanism should stay small for its own sake. The
important boundary is this:

- Good: typed, observable control data that selects one bounded repair action.
- Bad: hidden autonomous work, unbounded retry, or provider/model-specific
  behavioral policy.

The latest eval failures show that several source mechanisms were projected too
thinly. Those should be implemented as explicit CommandAgent contracts, not kept
lightweight merely to avoid code.

## Source Baseline

This coverage table is keyed to the following Anvil source checkout:

| Field | Value |
| --- | --- |
| Repository | `/Users/maenokota/share/work/github_kewton/Anvil-develop` |
| HEAD | `b3ca3d330546a10bf90d8dd46bd3e102f1710573` |
| Dirty state | dirty at inventory time |

Dirty files observed at the time of this clarification:

- `scripts/codex_orchestrate.py`
- `tests/test_codex_orchestrate.py`
- untracked command/skill/prompt helper files under `.agents/`, `.claude/`,
  `.codex/`, and `scripts/`

Future parity reviews must either use this baseline or explicitly refresh the
baseline and update the coverage IDs.

Dirty/untracked file treatment is fixed in:

```text
workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md
```

## Status Legend

| Current status | Meaning |
| --- | --- |
| Implemented | CommandAgent has an equivalent responsibility and tests/docs for the MVP shape. |
| Partial | CommandAgent has a label, hint, or narrow projection, but not the full state/action/ledger responsibility. |
| Missing | No equivalent mechanism currently owns the responsibility. |
| Excluded | Deliberately not part of CommandAgent because it would violate the runtime boundary or product direction. |

| Adoption decision | Meaning |
| --- | --- |
| Adopt | Bring the responsibility into CommandAgent as an explicit contract, state machine, validator, or evaluator. |
| Partial | Keep the existing CommandAgent mechanism, but complete the missing contract/state/ledger responsibilities. |
| Missing | No implementation exists yet; keep it visible as an unowned gap until a priority decision is made. |
| Excluded | Do not port. The source mechanism conflicts with CommandAgent's product or runtime boundary. |

## Complete Coverage Table

| Coverage ID | Source mechanism | Representative source modules | Source responsibility | CommandAgent owner | Current status | Adoption decision | Current CommandAgent mapping | Missing from parity / next action |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C01 | Task contract core | `task_contract.rs`, `task_contract_core.rs`, `task_contract_taxonomy.rs`, `task_contract_display.rs` | Holds task purpose, kind, artifacts, constraints, and expected completion evidence. | Step runner plan schema, profiles, artifact graph, TaskContract projection | Implemented | Adopt | `TaskContract` now projects task kind, admission status, lifecycle, deterministic constraints, request signals, required artifacts, completion evidence expectations, behavior obligations, and artifact role facts into plan prompts, active step facts, plan-lint evidence, and eval report fields. Cross-command persistence is explicitly bounded to visible plan/session/evidence/eval artifacts; later commands reconstruct from public inputs and workspace facts. | Closed by Phase22 proof: `cargo test task_contract`, `python3 tests/test_eval_report.py`, focused fixture assertions, and broad sign-off pass. |
| C02 | Task contract inference and admission | `task_contract_request_inference.rs`, `contract_request_signals.rs`, `task_contract_admission.rs`, `task_kind_confirm.rs`, `classify_confirm_flow.rs` | Infers whether the request is coding/docs/data/ops/research and admits contract authority. | Plan input, profiles, plan lint | Implemented | Partial | Explicit intent, goal keywords, profile signals, required artifacts, and artifact roles now become deterministic request signals. Clear requests admit; ambiguous or conflicting inferred signals become partial/conflict evidence, and plan lint rejects non-admitted task contracts when artifact ownership would proceed. | Closed by Phase22 proof: request/admission unit tests, plan-lint correction evidence tests, focused `task-contract-admission`, and broad sign-off pass. |
| C03 | Objective and behavior contract projection | `objective_contract_projection.rs`, `behavior_contract_projection_e2e_tests.rs`, `required_behavior.rs`, `behavior_delta_obligation.rs`, `contract_bound_generation.rs`, `contract_generation_expectations.rs` | Projects user-visible required behavior into obligations and completion checks. | Plan prompt, plan lint, profile verification, TaskContract projection | Implemented | Adopt | Required artifacts, deliverable kinds, and profile obligations now become typed behavior obligations for dependency setup, manifest, build, dev-port, route integration, docs literal, data schema, test artifact, and source/artifact completion. Plan lint enforces deterministic owner steps and eval reports owner/path/status evidence. | Closed by Phase22 proof: behavior projection/lint unit tests, eval report tests, focused `behavior-obligation-projection`, and broad sign-off pass. |
| C04 | Artifact role taxonomy | `task_contract_artifact_contract.rs`, `task_contract_artifact_predicates.rs`, `task_contract_artifact_intent.rs`, `artifact_target_alignment.rs` | Classifies artifacts as setup, implementation, test, docs, data, route/integration targets. | ArtifactGraph / profiles / TaskContract projection | Implemented | Adopt | `ArtifactKind` now projects through a shared `ArtifactRole` boundary consumed by workspace snapshots, target admission, artifact completion, recovery admission, and eval/report fallbacks. The taxonomy distinguishes route/entrypoint, setup manifest/config, implementation, test, docs, raw input, derived output, generated/build output, and dependency cache. | Closed by Phase23 proof: profile-artifact/artifact-graph/target/completion tests, focused `artifact-role-scope-ownership`, and broad sign-off pass. |
| C05 | Task workspace scope | `task_workspace_scope.rs`, `workspace_access.rs`, `workspace_candidates.rs`, `workspace_walk.rs`, `workspace_paths.rs` | Decides which subtree this task may claim ownership over. | WorkspaceSnapshot / WorkspaceScope / ArtifactOwnership / TargetAdmission | Implemented | Adopt | `WorkspaceSnapshot` performs a bounded path walk, skips dependency/cache/build output paths, records manifests/lockfiles, and combines snapshot paths with `ArtifactGraph` for greenfield/single-project/explicit/ambiguous scope evidence. `WorkspaceScope` exposes roots and excluded paths, and ownership/target consumers reject paths outside that scope. | Closed by Phase23 proof: workspace-scope/snapshot/ownership/target tests, focused `artifact-role-scope-ownership`, and broad sign-off pass. |
| C06 | Artifact ownership | `artifact_ownership.rs`, `owned_test_projection.rs`, `artifact_state_projection.rs` | Distinguishes owned artifacts from candidate-only or out-of-scope files. | ArtifactOwnership / TargetAdmission / CompletionAuthority / Repair loop evidence | Implemented | Adopt | `ArtifactOwnershipDecision` carries ownership, reason/subreason, source of truth, workspace scope, candidate origin, repair admissibility, and role. Target admission rejects non-owned, stale, exhausted, raw-input, generated/cache, and out-of-scope targets. Completion authority now requires in-scope owned non-generated deliverables instead of accepting candidate-only reads or cache/generated/raw input observations. | Closed by Phase23 proof: ownership/target/completion/evidence-authority tests, focused `artifact-role-scope-ownership`, and broad sign-off pass. |
| C07 | Artifact ledger | `artifact_ledger.rs`, `artifact_ledger_state.rs`, `repo_edit_observation.rs`, `post_tool_reconciliation.rs` | Records per-turn artifact observations, edits, scaffold deltas, and verifier observations. | Minimal loop result / step runner evidence | Implemented | Adopt | `ArtifactLedgerSummary` records graph, read/write/edit tool records, verifier mentions, workspace observations, setup/scaffold deltas, and completion-authority inputs with role, lifecycle, ownership, scope, required/read/changed/created/verifier flags, source families, and eval-visible path fields. | Closed by Phase24 proof: artifact-ledger/evidence-authority tests, eval report assertions, focused `focused-artifact-ledger-producers`, and broad sign-off regression. |
| C08 | Completion evidence | `completion_evidence.rs`, `success.rs`, `completion_probe_gate.rs`, `objective_evidence.rs`, `evidence_observation.rs` | Converts actual tool/build/doc/data observations into completion authority. | Step verifier, final-answer guard, eval | Implemented | Adopt | Typed completion evidence now distinguishes verifier pass/fail, command observation, file layout pass, profile completion pass, missing evidence, failed evidence, and stale evidence. Observed completion facts are producer inputs, and completion authority/reporting expose status, source of truth, missing/failed/stale lists, and runner kind without hidden tool execution. | Closed by Phase24 proof: completion-evidence/evidence-producer/evidence-authority tests, focused `focused-completion-evidence-producers`, and broad sign-off regression. |
| C09 | Evidence binding | `evidence_binding.rs`, `evidence_runner.rs`, `evidence_binding` adapters | Checks whether a deliverable can bind to its evidence runner before execution. | Verifier/profile/setup | Implemented | Adopt | Evidence binding producers expose manifest/file-layout/docs/schema/source-citation/import/test/executable binding status and kind. Observed binding facts feed completion authority and eval fields, and failed bindings remain structured contract evidence rather than hidden repair dispatch. | Closed by Phase24 proof: evidence-binding/evidence-producer/evidence-authority tests, focused `focused-evidence-binding-producers`, and broad sign-off regression. |
| C10 | Deliverable obligation audit | `deliverable_obligation_audit.rs`, `task_contract_deliverable_projection.rs`, `task_contract_deliverable_lifecycle.rs`, `deliverable_freshness.rs` | Audits required deliverables, freshness, and non-coding artifact obligations. | Plan lint / eval / profile | Implemented | Adopt | `DeliverableObligation` now projects eval-visible kind/path/obligation fields and freshness decisions. Completion authority converts stale, missing, or read-only current-plan observations into distinct completion evidence so old observations cannot satisfy fresh deliverable obligations. | Closed by Phase24 proof: deliverable-obligation/task-contract/plan-lint/evidence-authority tests, focused `focused-deliverable-obligation-freshness`, and broad sign-off regression. |
| C11 | Active job arbiter | `active_job_arbiter.rs`, `active_job_emit.rs`, `actor_loop_phase_decision.rs`, `loop_phase.rs`, `model_request_phase.rs` | Selects the current recovery owner/job and loop control action before model action. | Recovery orchestration | Implemented | Adopt | Active-job candidates carry owner, job, action, source layer, source of truth, target hint, artifact role, rerun authority, tool policy, loop control action, lifecycle, and deterministic reason. Dispatch selects one owner/action or explicit no-owner, ambiguous-tie, explicit-stop, or conflict-stop state before repair prompt rendering. | Phase25 proof: active-job/recovery-orchestration/recovery-task tests plus focused dispatch root `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`. |
| C12 | Recovery owner / dispatch gate | `active_job_arbiter.rs`, `repair_job_dispatch.rs`, `artifact_recovery_flow.rs` | Prevents multiple recovery systems from acting at once. | Recovery orchestration | Implemented | Adopt | `active_job_lifecycle`, `recovery_owner`, `loop_control_action`, `dispatch_status`, `dispatch_reason`, `candidate_jobs`, and `tie_break_reason` are projected from a single dispatch gate into evidence, eval reports, and Recovery Task Contract rendering. Compatible same-owner candidates can merge deterministic metadata; competing owners stop with structured evidence. | Phase25 proof covers setup, manifest, route, source, docs, evidence-binding, verifier-contract, tool-protocol, no-owner, and ambiguous-tie dispatch fixtures. |
| C13 | Recovery messages and packets | `recovery_messages.rs`, `repair_packet.rs`, `failure_packet.rs`, `safe_stop_payload.rs`, `safe_stop_emit.rs` | Renders structured failure/repair/safe-stop information. | Recovery task / repair packet / final error | Implemented | Partial | Recovery task now renders owner, job, action, target, cluster, attempt outcome, required action, disallowed actions, rerun authority, repair brief status, action envelope status, and safe-stop payload fields for evidence binding, completion, setup, profile, semantic, and envelope failures. | Closed by Phase26 proof: recovery-task tests plus focused `recovery-task` fixture assertions and broad sign-off. |
| C14 | Setup bootstrap | `node_runner_manifest.rs`, `package_manifest_summary.rs`, `cargo_manifest_summary.rs`, `setup_artifact_validation.rs`, `node_request_helpers.rs`, `python_request_helpers.rs` | Treats dependency/toolchain setup and setup manifests as separate active jobs. | Setup runtime / setup lifecycle / recovery orchestration | Implemented | Partial | Setup is represented as a typed, verifier-owned active job with manifest identity, validation status, readiness, command authority, attempt key, fingerprint, stale reason, result, failure signature, and rerun result. Node, Rust, and Python setup blockers are visible without implicit dependency installation. | Closed by Phase26 proof: setup lifecycle/setup validation tests plus Node/Rust/Python focused fixtures. |
| C15 | Project probe/profile/scaffold profile | `project_probe.rs`, `project_profile.rs`, `project_profile_projection.rs`, `scaffold_profile.rs`, `scaffold_pipeline.rs` | Detects project shape and scaffold expectations before repair. | Profiles / artifact graph | Implemented | Partial | Profiles render a common output schema for project kind, root hints, manifests, entrypoints, integration artifacts, setup/scaffold artifacts, verifier commands, protected paths, behavior obligations, completion evidence, failure mappings, adapter families, capabilities, and recovery candidate hints. Scaffold facts remain artifact-contract evidence, not a hidden scaffold workflow. | Closed by Phase26 proof: profile output tests plus scaffold/profile focused fixture. |
| C16 | Profile failure to recovery job | `project_profile_projection.rs`, `scaffold_profile.rs`, profile modules | Routes profile verification failure to a recovery job/action/target. | Recovery orchestration | Implemented | Partial | Profile failures now map to typed recovery facts for route integration, manifest/config conflict, setup/dependency, source implementation, scaffold/materialization, and explicit stop. Profiles emit candidate facts only; the common dispatch gate still selects the final job/action. | Closed by Phase26 proof: recovery-orchestration profile mapping tests plus focused profile-failure fixture. |
| C17 | Semantic failure report | `semantic_failure.rs`, `semantic_repair_planning.rs`, `verifier_diagnostic_payload.rs`, `verifier_assessment_parser.rs` | Structures verifier failure into kind, clusters, observed/expected, affected cases, conflicts, and target candidates. | Evidence / recovery contract | Implemented | Adopt | Semantic failure reports preserve diagnostic code, kind, source of truth, conflict object inputs, observed/expected pairs, affected cases, candidate artifacts, preferred repair role, weak verifier reason, selected cluster, admitted cluster targets, and unknown-diagnostic count as data-only evidence. | Closed by Phase26 proof: semantic-failure tests plus semantic conflict focused fixture. Conflict resolution itself remains owned by C33/Phase28. |
| C18 | Semantic repair plan | `repair_job.rs`, `semantic_repair_planning.rs`, `task_contract_recovery_planning.rs` | Chooses one cluster, authority, repair role, hypothesis, and expected improvement. | Recovery task contract | Implemented | Adopt | Semantic repair context now renders selected cluster, repair role, hypothesis, expected improvement, expected evidence delta, success check, exhausted target/role/cluster facts, no-progress handoff, and repair state status into repair task/brief/eval fields without expanding retry budgets. | Closed by Phase26 proof: recovery-task/repair-state tests plus semantic repair focused fixture. |
| C19 | Repair brief | `repair_brief.rs`, `repair_framework_findings.rs` | Gives repair task root cause, target, allowed change kind, preservation constraints, and confidence. | Recovery task contract | Implemented | Adopt | Repair brief rendering consumes selected dispatch/action facts and exposes root cause, target, allowed change kind, allowed tool category, disallowed actions, preservation constraints, target confidence, success check, rejection reason, and expected improvement before bounded repair prompt rendering. | Closed by Phase26 proof: repair-brief/recovery-task tests plus repair brief focused fixture. |
| C20 | Repair action space | `repair_action.rs`, `repair_action_space.rs`, `repair_authority.rs`, `repair_plan.rs`, `repair_plan_admission.rs` | Validates whether a selected action is admissible for selected target/authority. | Recovery orchestration | Implemented | Adopt | Action envelopes now admit or reject selected setup, manifest, route, source, docs, evidence-binding, verifier-contract, tool-protocol, scaffold, and safe-stop action families before prompt rendering, including projected tool category, source-of-truth authority, target role, no-change contracts, and rejection evidence. | Closed by Phase26 proof: repair-action/action-envelope tests plus action-envelope focused matrix. |
| C21 | Repair target decision/admission | `repair_target_decision.rs`, `repair_target_admission.rs`, `recovery_targets.rs`, `set_artifact_recovery_target.rs`, `verifier_repair_targeting.rs` | Chooses and admits targets for the active failure. | Recovery orchestration / ArtifactGraph / ArtifactLedger / TargetAdmission | Implemented | Adopt | Target admission admits/rejects targets after active-job dispatch using role, ownership, workspace scope, source of truth, freshness, current excerpt status, exhausted target/role/cluster state, and deterministic source priority. Generated/cache, out-of-scope, stale, missing-excerpt, role-mismatch, and disallowed file targets are rejected with structured evidence. | Closed by Phase27 proof: target-admission tests, focused C21 target matrix `eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917`, and broad sign-off. |
| C22 | Repair target prioritization | `semantic_repair_planning.rs`, `repair_job.rs`, `verifier_failure_artifacts.rs` | Orders targets by failure kind, authority, role, and progress history. | Recovery orchestration / TargetAdmission | Implemented | Adopt | Target priority components include failure/source authority, role, ownership, freshness, focused-edit signal, and progress/exhaustion facts. Same-priority ambiguity stops with structured evidence instead of path-order fallback. | Closed by Phase27 proof: target-priority tests, focused C22 tie-stop fixture, and broad sign-off. |
| C23 | Repair job state machine | `repair_job.rs`, `repair_job_dispatch.rs`, `repair_lifecycle.rs`, `repair_progress.rs`, `verifier_repair_pass_flow.rs` | Tracks verifier repair attempts, rerun outcome, repeated signatures, target exhaustion, and safe stop. | Repair loop / recovery task | Implemented | Adopt | `RepairJobState` records attempt ledgers, before/after signatures, verifier outcomes, exhausted target/role/cluster facts, no-progress strategy, verifier rerun result, and structured safe-stop payloads. | Closed by Phase27 proof: repair-job tests, focused C23 lifecycle/rerun fixture, and broad sign-off. |
| C24 | Repair attempt ledger | `repair_attempt_outcome.rs`, `repair_progress.rs`, `repair_job.rs` | Records attempt outcome by target and failure cluster. | Recovery task / repair loop | Implemented | Adopt | Repair attempts expose `passed`, `noop`, `malformed`, `unsafe`, `duplicate`, `no_progress`, `improved_still_failing`, `worsened`, and `explicit_stop` outcomes with target, role, cluster, changed files, and before/after signatures. | Closed by Phase27 proof: repair-job tests, eval report expected-field tests, focused C24 attempt-ledger fixture, and broad sign-off. |
| C25 | No-progress recovery | `no_progress_recovery.rs`, `repair_progress.rs` | Detects same target/role producing no progress and switches strategy or stops. | Repair loop / target admission | Implemented | Adopt | Duplicate/no-progress/worsened outcomes exhaust target/role/cluster facts and select bounded strategies such as switch target, switch role, evidence binding, contract-conflict deferral, scaffold rebuild, or explicit stop without increasing retry budgets. Contract-conflict resolution remains C33/Phase28. | Closed by Phase27 proof: no-progress tests, focused C25 deferral fixture, and broad sign-off. |
| C26 | Verifier diagnostic assessment | `verifier_diagnostic_flow.rs`, `verifier_diagnostic_attempt.rs`, `verifier_diagnostic_prompt.rs`, `verifier_assessment_parser.rs`, `verifier_weak_reason.rs`, `verifier_weak_repair_target.rs` | Parses verifier failure and classifies likely cause/action. | Verifier / evidence | Implemented | Adopt | Verifier diagnostics carry command, diagnostic code, failure kind/signature, source excerpt, candidates, observed/expected pairs, affected cases, preferred repair role, weak source-grep/self-referential/generated-test reasons, command-not-found, port-in-use, admitted cluster targets, and unknown-diagnostic count. | Closed by Phase27 proof: verifier-diagnostic tests, focused C26 diagnostic fixture, and broad sign-off. |
| C27 | Verifier orchestration | `verifier_orchestration.rs`, `verifier_driver.rs`, `verifier.rs`, `verifier_skill.rs`, `verifier_observation.rs`, `emit_verifier_events.rs`, `verifier_evidence_scope.rs` | Runs verifier, classifies pass/fail, decides whether repair continues or stops. | Step runner verifier / repair loop | Implemented | Adopt | Original verifier authority is retained through repair attempts. Rerun outcomes, repeated-failure/no-progress safe stops, binding scope, and failure-attempt limits are visible in repair/eval fields rather than hidden retries or weaker substitute commands. | Closed by Phase27 proof: repair-job/verifier focused tests, C27 verifier-rerun fixture, and broad sign-off. |
| C28 | Verifier command policy | `verifier_command_policy.rs`, `node_test_evidence_quality.rs`, `generated_test_guard.rs`, `test_expectation_audit.rs` | Rejects brittle, self-referential, or unsupported generated tests/verifiers. | Plan lint / verifier selection | Implemented | Adopt | Verifier selection and integrity guards reject weak source grep, generated/self-referential verifier artifacts, unsupported generated-test assertions, expectation drift, and test weakening before completion evidence is claimed. | Closed by Phase27 proof: verifier-selection/integrity tests, focused C28 verifier-policy fixture, and broad sign-off. |
| C29 | Artifact completion job | `artifact_completion_job.rs`, `artifact_completion_record.rs`, `artifact_recovery_flow.rs` | Creates or repairs missing required deliverables. | ArtifactCompletionJob / recovery orchestration | Implemented | Adopt | Artifact completion jobs bind missing deliverables to artifact ledger, ownership, mode, role, target-only policy, completion evidence, freshness, and missing/failed/stale evidence distinction. | Closed by Phase27 proof: artifact-completion/evidence-authority tests, focused C29 completion-job fixture, and broad sign-off. |
| C30 | Focused edit recovery | `focused_edit_recovery.rs`, `read_target_helpers.rs`, `tool_history.rs`, `post_edit_excerpt.rs` | Directs repair to a previously read/editable target. | TargetAdmission / ArtifactLedger eval fields | Implemented | Adopt | Focused edit admission requires current excerpt/read evidence and rejects stale, changed-only, out-of-scope, and exhausted targets before prompt rendering. There is no hidden focused-edit engine. | Closed by Phase27 proof: target-admission tests, focused C30 stale-target fixture, and broad sign-off. |
| C31 | Forced small edit / deterministic fallback | `forced_small_edit.rs`, `deterministic_fallback_plan.rs`, `mechanical_compile_repair.rs` | Performs or steers narrowly scoped edits after repeated non-progress. | MechanicalRepairAdapter / RecoveryTaskContract | Implemented | Adopt | Mechanical repair adapters are bounded hints/proposals admitted only when owner, action, target, target role, verifier/source-of-truth authority, and allowed change kind are present. They do not directly mutate files and must hand off to patch validation. | Closed by Phase27 proof: mechanical-repair tests, focused C31 mechanical fallback fixture, and broad sign-off. |
| C32 | Repair patch executor/validation | `repair_patch_executor.rs`, `repair_patch_validation.rs`, `patch_provider.rs`, `patch_proposal.rs`, `repair_test_weakening_filter.rs` | Applies bounded patches, rejects unsafe/noop/duplicate/test-weakening patches, can rollback worsened patches. | PatchValidation Contract / repair loop / attempt ledger | Implemented | Adopt | Patch validation rejects unsafe, malformed, noop, duplicate, test-weakening, protected, generated/cache, dependency artifact, and self-referential verifier mutations before progress is claimed. Rollback admission is explicit and verifier-proven. | Closed by Phase27 proof: integrity/patch tests, focused C32 patch-validation fixture, and broad sign-off. |
| C33 | Contract conflict job | `contract_conflict_job.rs`, `spec_authority.rs`, `api_contract_expectation.rs` | Handles implementation-vs-test-vs-docs/API conflicts explicitly. | ContractConflictBoundary / RecoveryOrchestration | Implemented | Adopt | Contract conflicts now produce bounded fields for status, sides, authority, repair target side, selected action, missing evidence, safe-stop reason, and source of truth. Repairable conflicts route to existing source/test/verifier recovery actions; ambiguous or insufficient authority stops explicitly. | Closed by Phase28 proof: contract-conflict and recovery-orchestration tests, focused fixture root `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`, recheck, and broad sign-off. |
| C34 | Language-specific mechanical repair | `rust_binding_repair.rs`, `cargo_dependency_repair.rs`, `repair_python_import_evidence.rs`, `repair_python_test_analysis.rs`, `repair_assertion_analysis.rs`, `python_markers.rs` | Applies narrow compile/import/assertion/dependency fixes for known languages. | MechanicalRepairAdapter / verifier diagnostic payload | Partial | Adopt | Phase 12 adds bounded adapter outputs for Rust compile/test diagnostics, Python import/assertion/FastAPI diagnostics, TypeScript/Next.js type and route diagnostics, and dependency-missing manifest hints. Adapter output is evidence/hint data, not direct mutation or setup execution. | Broader language-specific repair families and live eval proof remain before full parity. |
| C35 | Tool policy and effective policy | `tool_policy.rs`, `tool_policy_decisions.rs`, `effective_tool_policy_flow.rs`, `tool_call_prepare.rs`, `tool_prep.rs`, `tool_call_execution.rs` | Projects current contract/job into allowed tool categories. | Minimal loop guards, step policy, recovery task | Partial | Adopt | Step tool policy and recovery tool-policy projection exist. Tool protocol correction now projects action-specific allowed tools for same-tool schema/JSON correction, stale-edit inspection, and repository-evidence correction. | Broaden owner/action-aware tool category projection for setup jobs, evidence binding, and later repair-job families. |
| C36 | Tool failure recovery | `tool_failure_recovery.rs`, `reply_retry.rs`, `reply_retry_types.rs`, `protocol.rs` | Handles malformed tool calls, prose-only failures, and bounded retry notes. | Provider parser, minimal loop guards, recovery task | Partial | Adopt | Tool protocol failures now normalize into a common payload, select a first-class correction action, render Recovery Task Contract fields, enforce narrow allowed tools, and safe-stop when correction is unsafe or exhausted. | Add broader live E2E coverage for provider parse and stale-edit branches before marking fully implemented. |
| C37 | Bash/setup command classification | `commands.rs`, `bash_policy_e2e_tests.rs`, `tester_invocation.rs`, `tester.rs`, `auto_test.rs`, `node_runner_manifest.rs` | Classifies local commands and test runners separately from ordinary mutation. | Bash tool, verifier, setup runtime | Partial | Partial | Build/test/setup checks exist in tool/verifier policy. | Add evidence binding and setup command authority before permitting more setup recovery. |
| C38 | Workspace candidates/walk | `workspace_candidates.rs`, `workspace_walk.rs`, `path_helpers.rs`, `workspace_access.rs` | Collects meaningful in-scope workspace files for contracts and ownership. | Eval/workspace scans, ArtifactGraph inputs | Partial | Adopt | Required artifacts and observed paths feed the graph. | Add scope-aware workspace walk, ignored-dir SSOT in recovery, and existing candidate discovery. |
| C39 | Job report / progress events | `job_report.rs`, `active_job_emit.rs`, `progress_text.rs`, `progress_tests.rs`, `footer.rs`, `spinner.rs`, `quality_gate.rs` | Emits structured active job, repair action, progress, and quality-gate outcomes. | Runtime events / eval reports | Partial | Partial | Evidence envelope and orchestration fields exist. | Add job-level report schema, active owner, repair action plan status, and attempt outcome events. |
| C40 | Scaffold pipeline | `scaffold_pipeline.rs`, `scaffold_profile.rs`, `scaffold_profile_e2e_tests.rs`, `scaffold_coding_guard_e2e_tests.rs` | Materializes project/profile scaffold and tracks scaffold ownership. | Profiles / plan artifacts | Partial | Partial | Some profile-required artifacts and scaffold materialization evidence exist. | Add scaffold profile as setup/artifact contract, not an independent workflow engine. |
| C41 | Data/docs/research/ops evidence | `structured_data_observation.rs`, `task_contract_data_output_context.rs`, `data_capability_e2e_tests.rs`, `research_acceptance_e2e_tests.rs`, `ops_capability_e2e_tests.rs`, `authoring_style.rs` | Handles non-coding deliverables and their evidence. | Profiles / eval | Partial | Partial | Docs/data profiles have basic required-path/content behavior. | Add generic evidence binding/completion evidence before profile-specific expansion. |
| C42 | Answer-only and work-mode gating | `answer_only_mode.rs`, `work_mode_confirm.rs`, `answer_only` tests, `confirmation_flow.rs`, `work_mode_confirm.rs` | Keeps non-mutating answer-only tasks from being forced into file edits. | Minimal loop final-answer guard / step policy | Partial | Partial | Final-answer and no-tool guards exist, with some broadening constraints. | Keep as policy gate; avoid broadening into normal coding repair. |
| C43 | Interruption, lifecycle, turn state | `interrupt.rs`, `lifecycle.rs`, `loop_state.rs`, `turn_state.rs`, `prepare_actor_loop_state.rs`, `run_turn.rs`, `actor_loop_flow.rs`, `summary.rs` | Maintains loop lifecycle and terminal outcomes. | CLI/repl/minimal loop/session | Partial | Partial | Minimal loop/session state exists. | Do not port full actor-loop complexity; only add state required by explicit recovery contracts. |
| C44 | Provider/model request plumbing | `model_request.rs`, `build_request_messages.rs`, `model_request_phase.rs`, `streaming_reply.rs` | Builds model messages and phases. | Providers / minimal loop prompt | Partial | Partial | Provider transports and minimal prompts exist. | Keep transport/prompt clean; do not put recovery policy into provider-specific branches. |
| C45 | Provider transport parser | provider modules, XML/native parser equivalents | Parses native or fallback tool calls without owning behavior policy. | Providers | Implemented | Adopt | Provider transport is separate from planning/recovery; XML fallback remains compatibility fallback. | Continue provider-native parsing but keep behavior policy outside transports. |
| C46 | Working memory/reminders | `working_memory_messages.rs`, `reminder.rs`, `reminder_pipeline.rs`, `precaution_relevance.rs` | Adds memory/reminder guidance to model turns. | None | Excluded | Excluded | No equivalent. | Keep excluded unless a separate design decision admits memory/advisory systems. |
| C47 | Case record and anti-pattern corpora | `case_record_flow.rs`, `case_record_extract.rs`, `anti_pattern_flow.rs`, recovery masking modules | Records cases and retrieves anti-pattern guidance. | None | Excluded | Excluded | No equivalent. | Keep excluded for MVP; not needed for explicit contract recovery. |
| C48 | PAM/Photon advisory | `pam_advisory.rs`, `photon_feedback_derive.rs`, `photon_user_feedback.rs`, related tests | Adds sidecar/advisory feedback. | None | Excluded | Excluded | No equivalent. | Keep excluded; would reintroduce advisory stack. |
| C49 | Quality classification/confirmation | `quality.rs`, `quality_confirm.rs`, `feedback_kind_confirm.rs`, `task_classification.rs` | Classifies quality/feedback/task intent through secondary confirmation. | Eval/profile/final guard | Missing | Missing | Some eval quality checks exist outside runtime. | Default to exclusion unless deterministic quality classification is required by recovery/eval evidence; do not adopt semantic advisory confirmation by default. |
| C50 | Slash/plan/command UI helpers | `slash_commands.rs`, `plan_sections.rs`, `plan_mode_helpers.rs`, `commands.rs`, `tool_display.rs`, `message_push.rs`, `footer.rs` | User-interface and rendering helpers. | CLI/repl/slash command | Partial | Missing | CommandAgent has independent CLI/slash implementation. | Default to exclusion unless CommandAgent UX/eval evidence shows a recovery-parity gap; do not import Anvil slash commands into the REPL by default. |
| C51 | Legacy engine selector | historical legacy switches | Switches between multiple engines/controllers. | None | Excluded | Excluded | No equivalent. | Keep excluded. CommandAgent has one execution engine. |
| C52 | Hidden or unbounded repair loop | historical broad recovery loops | Continues internally until success. | None | Excluded | Excluded | Repair remains bounded and user-visible. | Keep excluded. |
| C53 | Provider/model-specific behavioral policy | historical provider/model branches | Changes repair/planning behavior for one provider or model. | None | Excluded | Excluded | Shared behavior lives outside provider transports. | Keep excluded. |
| C54 | Model-issued dependency installation | model-driven setup execution | Lets the model decide to run dependency setup implicitly. | None | Excluded | Excluded | Setup requires explicit policy/evidence and remains verifier-owned. | Keep excluded; setup can be a visible active job. |

## Inventory Coverage Summary

This table currently groups the source `loop_run` surface into 54 functional
rows. It is grouped by responsibility rather than listing every helper or test
file one by one.

Current implementation status:

| Current status | Count |
| --- | ---: |
| Implemented | 33 |
| Partial | 12 |
| Missing | 2 |
| Excluded | 7 |

Adoption decision:

| Adoption decision | Count |
| --- | ---: |
| Adopt | 33 |
| Partial | 12 |
| Missing | 2 |
| Excluded | 7 |

Interpretation:

- `Adopt` + `Partial` is the accepted migration surface. There are 45 rows
  where CommandAgent should own the responsibility, either as a new mechanism
  or by completing an existing projection.
- Of those 45 accepted rows, 12 still require row-level implementation proof.
  Provider transport parsing plus Phase22 C01-C03, Phase23 C04-C06, and
  Phase24 C07-C10, Phase25 C11-C12, Phase26 C13-C20, and Phase27 C21-C32 are currently marked
  `Implemented`.
- `Missing` adoption rows are not accepted migration work yet. They form the
  unresolved priority-decision surface and must become `Adopt`, scoped
  `Partial`, or `Excluded` before final closure.
- `Excluded` rows are not gaps for the current architecture.

The table is not a byte-for-byte file inventory. Test modules, e2e fixtures,
formatting helpers, UI progress helpers, and small path/string helpers are
covered by their owning responsibility row unless they encode distinct runtime
control.

## What Is Currently Too Thin

The current implementation is useful, but the following mappings are too thin
to resolve the recurring eval failures reliably:

1. Active-job arbitration is now implemented for the C11-C12 dispatch
   boundary. Remaining recovery-depth work is tracked by the later
   recovery-task, target-admission, verifier, and conflict rows rather than by
   C11-C12.
2. Scope-aware workspace admission is implemented for the C05 boundary.
   Remaining workspace-candidate discovery and broader walk parity belong to
   C38 rather than the C05 ownership gate.
3. Artifact ownership is implemented for the C06 admission/completion boundary,
   and Phase24 closes the C07 producer-visible artifact ledger surface.
   Remaining target prioritization and repair-lifecycle use of those facts
   belongs to C21 and later rows.
4. Tool records, verifier observations, setup/scaffold deltas, and
   completion-authority inputs are now represented in the artifact ledger.
   Post-tool recovery actions still belong to the later repair-lifecycle rows.
5. Semantic failure data now has deterministic diagnostic clusters for verifier
   failures, but conflict handling and cluster-level target ranking are still
   partial.
6. `allowed_change_kind` is guidance, not an admissible repair action plan.
7. Setup validation now covers NPM, Cargo, and Python manifest checks in the
   setup/verifier evidence path, but only Node has bounded setup execution.
8. Completion evidence is a runtime authority for file-layout and verifier
   evidence, but richer docs/data/report/profile-wide producers are still
   partial.
9. Evidence binding is a first-class failure class for current producers, but
   route/import/manifest/schema bindings need broader producer coverage.
10. Deliverable freshness is observable as `stale_evidence`, but obligation
    projection into freshness requirements is still partial.
11. No-progress recovery cannot ban a failed target/role or switch strategy.
12. Tool policy is now projected from recovery owner/action in the active-job
    dispatch boundary. Remaining work is broader E2E coverage and later-phase
    lifecycle transitions, not another owner-selection path.

## What Changed In The Latest Slice

- Verifier repair no longer treats a pytest-owned test artifact as the default
  source repair target when a plan-owned source artifact is available.
- Next.js build failures with route-level diagnostics can target the selected
  route from the plan's required artifacts even when the verify step has no
  expected path.
- Tool protocol failures that repeat after one correction carry the exhausted
  correction fact in the early contract evidence/attempt ledger so repair
  packets remain useful under truncation.
- Tool protocol failures now carry a normalized source/action payload, selected
  correction action, allowed tools, disallowed actions, and eval fields for
  correction spent/exhausted status.
- Dependency setup now distinguishes invalid/missing `package.json` from
  dependency installation. Invalid setup artifacts are routed to manifest
  repair before setup is attempted.
- Added typed `WorkspaceScope` and `ArtifactOwnership` boundaries. Target
  admission now rejects out-of-scope and candidate-only targets before repair
  policy renders a bounded task.
- Added exact changed-path recording for `Write` and `Edit` tool calls.
  Repair packets now receive changed file paths instead of coarse `Write` or
  `Edit` markers.
- Added a bounded artifact-ledger summary type that classifies tool target
  paths by artifact role and ownership without storing raw tool payloads.
- Added typed completion evidence, evidence binding, deliverable obligation,
  recovery owner, repair action plan, semantic failure report, repair job
  state, attempt outcome, patch validation, and eval report projection fields.
- Added `evidence_binding_repair` as a first-class recovery job/action and
  carried recovery owner/action-plan data through repair packets and evidence
  envelopes.
- Broadened setup artifact validation foundations from npm manifests to Rust
  `Cargo.toml` and Python `pyproject.toml` / `requirements.txt`.
- Added a bounded verifier diagnostic payload boundary. Verifier failures now
  can carry deterministic diagnostic code, observed/expected pairs, affected
  cases, preferred repair role, weak verifier reason, admitted cluster targets,
  and confidence through ContractEvidence, RecoveryTaskContract, semantic
  clusters, and eval reports.
- Added a shared `FailureObservation` taxonomy and runtime projection. Failed
  `ContractEvidence` records and `EvidenceEnvelope`s now expose a compact
  terminal-state observation, and eval reports share the same taxonomy fixture
  for terminal state, producer, contract layer, source of truth, diagnostic,
  and actionability fields. Unknown/raw failures remain visible instead of
  being silently collapsed into source repair.
- Added a typed setup lifecycle boundary. Setup and manifest evidence can now
  report setup job kind, target, manifest kind/path, validation status,
  readiness, command authority, setup result, failure signature, stale reason,
  and verifier rerun result without reading raw setup logs.
- Added common profile output rendering across profiles. Next.js, Rust,
  Python, docs, and data profiles now expose comparable root hints,
  setup/scaffold/integration artifacts, verifier commands, obligations, and
  recovery candidate hints while leaving final dispatch in the common recovery
  gate.
- Added profile/language adapter parity fields. Profiles now expose project
  kind, manifest artifacts, entrypoints, integration artifacts, completion
  evidence requirements, failure mappings, adapter families, and a capability
  status matrix through the same output schema. Eval reports these fields under
  profile parity so missing support is visible as a contract coverage gap
  rather than an implicit Next.js-only branch.

## Recommended Port Order

The next implementation should not add more prose to repair prompts first. It
should add missing control data in this order:

1. Connect artifact-ledger verifier observations and post-tool reconciliation
   to recovery/eval reports.
2. Add completion evidence and evidence binding as first-class contract
   outcomes.
3. Add recovery owner / active job arbiter gate.
4. Add repair action plan with admissible/rejected status and allowed tool
   category.
5. Add semantic failure report and cluster-level repair target prioritization.
6. Add repair job state, attempt ledger, and no-progress recovery policy.
7. Broaden setup artifact validation across Rust, Python, and Node manifests.
8. Add owner/action-aware tool policy projection.

This order keeps the behavior attributable: each later mechanism depends on
actual artifact/scope/action facts rather than asking the model to infer a
repair strategy from prose.

## Phase20 Final Closure Appendix - 2026-06-23

Phase20 reconciled this coverage table against Phase1-Phase19 implementation
reports and the final broad sign-off roots.

The broad sign-off command passed:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
status: pass
```

However, Phase20 did not change any `Partial` or `Missing` row to
`Implemented` because row-level parity proof is still absent for many accepted
responsibilities. The final Phase20 decision is therefore:

```text
migration_not_complete
```

Supporting Phase20 artifacts:

- Phase20 workspace `coverage_closure.md`
- Phase20 workspace `ledger_reconciliation.md`
- Phase20 workspace `continuation_ledger.md`
- `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`

Final coverage interpretation:

| Candidate | Count |
| --- | ---: |
| Implemented | 1 |
| Excluded | 7 |
| Unresolved accepted migration surface | 44 |
| Unresolved priority-decision surface | 2 |

The next continuation phase should split the Phase20 continuation ledger into
row-level implementation tasks before changing this table's final statuses.

## Phase21 Core Contract And Ownership Appendix - 2026-06-23

Phase21 selected Phase20 continuation blocker `P20-COV-001` and expanded C01
through C12 into row-level closure records.

Phase21 did not promote any coverage row to `Implemented`. The selected rows
still require implementation proof before status changes:

| Phase21 disposition | Count |
| --- | ---: |
| `closed_proven` | 0 |
| `excluded_with_rationale` | 0 |
| `split_forward` | 12 |
| `open` | 0 |

The selected rows now have explicit downstream blockers:

| rows | downstream phase | responsibility |
| --- | --- | --- |
| C01-C03 | Phase22 | task contract, request admission, behavior obligation projection |
| C04-C06 | Phase23 | artifact role, workspace scope, ownership |
| C07-C10 | Phase24 | artifact ledger, completion evidence, evidence binding, deliverable audit |
| C11-C12 | Phase25 | active-job arbitration and recovery dispatch lifecycle |

Supporting Phase21 artifacts:

- Phase21 workspace row closure matrix
- Phase21 workspace blocking ledger
- Phase21 workspace reconciliation map
- `docs/eval/loadmap2-phase21-core-contract-ownership-20260623.md`

## Phase23 Artifact Scope Ownership Appendix - 2026-06-23

Phase23 closed C04 through C06 with runtime-effective, eval-visible proof for
artifact role taxonomy, workspace scope admission, and artifact ownership.

Status changes:

| row | previous | current | proof |
| --- | --- | --- | --- |
| C04 | Partial | Implemented | Shared `ArtifactKind` -> `ArtifactRole` projection, raw/derived data roles, target/completion consumers, eval fallback alignment, focused fixture proof. |
| C05 | Partial | Implemented | Greenfield, single-project, explicit root, ambiguous parent, and excluded dependency/cache/build output scope tests plus focused scope assertion. |
| C06 | Partial | Implemented | Ownership reason/source/scope tests, target admission raw/generated/cache/out-of-scope rejection, completion authority owned/in-scope deliverable gate, exhausted-target proof. |

Proof commands:

```bash
cargo fmt --check
cargo test profile_artifact
cargo test artifact_graph
cargo test workspace_scope
cargo test workspace_snapshot
cargo test artifact_ownership
cargo test target_admission
cargo test artifact_completion
cargo test evidence_authority
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/planning --out eval/runs/loadmap2-phase23-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 --cases-dir eval/cases/focused/control-recovery/planning --recheck
cargo test
cargo build --release
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023
focused assertions: passed_recheck
broad sign-off: pass
```

The migration decision remains:

```text
migration_not_complete
```

## Phase27 Target Verifier Patch Appendix - 2026-06-23

Phase27 closed C21 through C32 with row-level proof for target admission,
target prioritization, repair lifecycle, attempt ledgers, no-progress
strategy, verifier diagnostics/orchestration/policy, artifact completion,
focused edit admission, mechanical fallback admission, and patch validation /
rollback admission.

Status changes:

| row | previous | current | proof |
| --- | --- | --- | --- |
| C21 | Partial | Implemented | Target admission admits/rejects route/source/test/docs/setup/evidence-binding targets with owner, role, scope, freshness, excerpt, and rejection evidence. |
| C22 | Partial | Implemented | Target priority components are deterministic and same-priority ambiguity stops explicitly. |
| C23 | Partial | Implemented | Repair lifecycle records verifier rerun outcomes and safe-stop evidence. |
| C24 | Partial | Implemented | Attempt ledger records outcome, target, role, cluster, changed paths, and signatures. |
| C25 | Partial | Implemented | No-progress strategy selects bounded switch/stop/defer paths; contract-conflict deferral is closed by Phase28/C33 conflict authority proof. |
| C26 | Partial | Implemented | Verifier diagnostics expose language/common codes, failure kind, candidates, observed/expected, affected cases, weak reason, and unknown count. |
| C27 | Partial | Implemented | Verifier rerun outcome, repeated failure safe stop, binding scope, and attempt limits are visible without weakening the verifier. |
| C28 | Partial | Implemented | Weak/generated/self-referential/unsupported verifier/test assertions are rejected before progress is claimed. |
| C29 | Partial | Implemented | Artifact completion is bound to ledger, ownership, freshness, and missing/failed/stale evidence distinction. |
| C30 | Partial | Implemented | Focused edit requires current excerpt/read evidence and rejects stale targets. |
| C31 | Partial | Implemented | Mechanical fallback is admitted only with owner/action/target/role/source-of-truth/allowed-change authority and still requires patch validation. |
| C32 | Partial | Implemented | Patch validation rejects unsafe/noop/duplicate/test-weakening/protected/generated/cache mutations and records rollback admission. |
| C33 | Missing | Implemented | Contract conflict authority decisions separate authoritative side from repair target side and safe-stop ambiguous/insufficient authority. |

Proof commands:

```bash
cargo test target_admission
cargo test repair_job
cargo test verifier_diagnostic
cargo test verifier_selection
cargo test integrity_guard
cargo test artifact_completion
cargo test evidence_authority
cargo test mechanical_repair
cargo test repair_action_plan
cargo test recovery_orchestration
cargo test repair_loop
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/target-verifier-patch --out eval/runs/loadmap2-phase27-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917 --cases-dir eval/cases/focused/control-recovery/target-verifier-patch --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917
focused assertions: passed_recheck: 12
broad sign-off: pass
```

The migration decision remains:

```text
migration_not_complete
```

Reason: C01-C12 are no longer vague continuation work, but they remain
split-forward blockers until the downstream proof gates pass.

## Phase24 Ledger Evidence Binding Appendix - 2026-06-23

Phase24 closed C07 through C10 with producer-visible, eval-visible proof for
artifact ledger producers, completion evidence producers, evidence binding
producers, and deliverable obligation freshness.

Status changes:

| row | previous | current | proof |
| --- | --- | --- | --- |
| C07 | Partial | Implemented | Ledger source family fields for graph/tool/verifier/setup/scaffold/workspace/completion-authority inputs plus required/read/changed/created path classes. |
| C08 | Partial | Implemented | Completion evidence kind/status/source fields for verifier, file layout, missing, failed, and stale evidence without hidden evidence execution. |
| C09 | Partial | Implemented | Evidence binding kind/status/source fields and failed binding lists projected into completion authority and eval. |
| C10 | Partial | Implemented | Deliverable obligation kind/path/obligation fields and stale read-only freshness checks. |

Proof commands:

```bash
cargo test artifact_ledger
cargo test completion_evidence
cargo test evidence_producer
cargo test evidence_authority
cargo test evidence_binding
cargo test deliverable_obligation
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/completion --out eval/runs/loadmap2-phase24-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617 --cases-dir eval/cases/focused/control-recovery/completion --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617
focused assertions: passed: 6
recheck assertions: passed_recheck: 6
broad sign-off: pass
```

The migration decision remains:

```text
migration_not_complete
```

Reason: Phase24 closes C07-C10, but later accepted rows C11 onward still need
row-level proof before final migration completion can be declared.

## Phase25 Active Job Dispatch Appendix - 2026-06-23

Phase25 closed C11 through C12 with row-level proof for active-job lifecycle
projection, deterministic arbitration, single recovery owner/action dispatch,
and Recovery Task Contract prompt-input propagation.

Status changes:

| row | previous | current | proof |
| --- | --- | --- | --- |
| C11 | Partial | Implemented | Candidate and selected/stop lifecycle evidence for selected, no-owner, ambiguous-tie, explicit-stop, and conflict-stop dispatch states. |
| C12 | Partial | Implemented | Recovery dispatch evidence and recovery task rendering consume one selected owner/action or explicit stop before bounded repair prompt rendering. |

Proof commands:

```bash
cargo test active_job
cargo test recovery_orchestration
cargo test recovery_task
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/dispatch --out eval/runs/loadmap2-phase25-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 --cases-dir eval/cases/focused/control-recovery/dispatch --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110
focused assertions: passed: 10
recheck assertions: passed_recheck: 10
broad sign-off: pass
```

The migration decision remains:

```text
migration_not_complete
```

Reason: Phase25 closes C11-C12, but later accepted rows C13 onward still need
row-level proof before final migration completion can be declared.

## Phase26 Recovery Task And Action Envelope Appendix - 2026-06-23

Phase26 closed C13 through C20 with row-level proof for recovery task packets,
setup lifecycle evidence, common profile/scaffold facts, typed profile failure
mapping, semantic failure/repair context, repair brief rendering, and repair
action envelope admission/rejection.

Status changes:

| row | previous | current | proof |
| --- | --- | --- | --- |
| C13 | Partial | Implemented | Recovery Task Contract renders safe-stop payload, owner/job/action/target/cluster/attempt context, required/disallowed actions, and rerun authority for evidence/completion/setup/profile/semantic/envelope failures. |
| C14 | Partial | Implemented | Setup lifecycle records manifest identity, readiness, command authority, attempt key/fingerprint, stale reason, result, failure signature, and non-Node setup blockers without implicit setup execution. |
| C15 | Partial | Implemented | Profiles render common project/profile/scaffold facts, capability status, completion evidence, failure mappings, and adapter families. |
| C16 | Partial | Implemented | Profile failures project typed route, manifest, setup, source, scaffold, and explicit-stop recovery facts consumed by common dispatch. |
| C17 | Partial | Implemented | Semantic reports preserve conflict inputs, observed/expected pairs, affected cases, candidate artifacts, preferred role, cluster, and unknown diagnostic visibility. |
| C18 | Partial | Implemented | Semantic repair plan fields include selected cluster, hypothesis, expected improvement, exhausted target/role/cluster, no-progress handoff, and repair-state status. |
| C19 | Partial | Implemented | Repair briefs expose root cause, target, constraints, allowed/disallowed actions, confidence, preservation, and success check from selected dispatch/action facts. |
| C20 | Partial | Implemented | Action envelopes admit or reject action families before prompt rendering and report lifecycle/status evidence. |

Proof commands:

```bash
cargo test recovery_task
cargo test recovery_orchestration
cargo test setup_lifecycle
cargo test setup_artifact_validation
cargo test semantic_failure
cargo test repair_brief
cargo test repair_action_plan
cargo test profiles
cargo test profile_artifact
cargo test repair_job
cargo test recovery_policy
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/recovery-task --out eval/runs/loadmap2-phase26-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340 --cases-dir eval/cases/focused/control-recovery/recovery-task --recheck
python3 scripts/eval_signoff.py --require-recheck --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 --root focused-fixture=eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340 --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
focused fixture root: eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340
focused assertions: passed_recheck: 11
broad sign-off: pass
```

The migration decision remains:

```text
migration_not_complete
```

Reason: Phase26 closes C13-C20, but later accepted rows C21 onward still need
row-level proof before final migration completion can be declared.
