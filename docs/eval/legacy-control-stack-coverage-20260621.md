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

| Source mechanism | Representative source modules | Source responsibility | CommandAgent owner | Current status | Adoption decision | Current CommandAgent mapping | Missing from parity / next action |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Task contract core | `task_contract.rs`, `task_contract_core.rs`, `task_contract_taxonomy.rs`, `task_contract_display.rs` | Holds task purpose, kind, artifacts, constraints, and expected completion evidence. | Step runner plan schema, profiles, artifact graph, TaskContract projection | Partial | Adopt | `TaskContract` now projects task kind, required artifacts, behavior obligations, and artifact role facts into plan prompts, active step facts, plan-lint evidence, and eval report fields. | Add richer constraints, expected completion evidence, lifecycle state, and cross-command task contract persistence. |
| Task contract inference and admission | `task_contract_request_inference.rs`, `contract_request_signals.rs`, `task_contract_admission.rs`, `task_kind_confirm.rs`, `classify_confirm_flow.rs` | Infers whether the request is coding/docs/data/ops/research and admits contract authority. | Plan input, profiles, plan lint | Partial | Partial | Profiles and slash commands provide coarse task/profile intent. | Add deterministic task-kind/request signals where plan/profile ambiguity causes wrong workflow. |
| Objective and behavior contract projection | `objective_contract_projection.rs`, `behavior_contract_projection_e2e_tests.rs`, `required_behavior.rs`, `behavior_delta_obligation.rs`, `contract_bound_generation.rs`, `contract_generation_expectations.rs` | Projects user-visible required behavior into obligations and completion checks. | Plan prompt, plan lint, profile verification, TaskContract projection | Partial | Adopt | Required artifacts, deliverable kinds, and profile obligations now become typed behavior obligations such as dependency setup, manifest contract, dev-server port, route integration, docs literal, data schema, and test artifact. | Add richer completion checks and behavior-delta obligations beyond deterministic path/profile facts. |
| Artifact role taxonomy | `task_contract_artifact_contract.rs`, `task_contract_artifact_predicates.rs`, `task_contract_artifact_intent.rs`, `artifact_target_alignment.rs` | Classifies artifacts as setup, implementation, test, docs, data, route/integration targets. | ArtifactGraph / profiles / TaskContract projection | Partial | Adopt | Path-based `ArtifactRole`, profile-specific classifiers, setup-step ownership lint, and TaskContract artifact-role projection exist. | Continue unifying role SSOT across profile verification, verifier repair, and recovery admission. |
| Task workspace scope | `task_workspace_scope.rs`, `workspace_access.rs`, `workspace_candidates.rs`, `workspace_walk.rs`, `workspace_paths.rs` | Decides which subtree this task may claim ownership over. | Safety/path confinement, recovery contract label | Partial | Adopt | `WorkspaceSnapshot` now performs a bounded path walk, skips dependency/cache/build output paths, records manifests/lockfiles, and combines snapshot paths with `ArtifactGraph` for greenfield/single-project/explicit/ambiguous scope evidence. | Add persistent task-scope admission and richer profile-selected root handling where observed failures require it. |
| Artifact ownership | `artifact_ownership.rs`, `owned_test_projection.rs`, `artifact_state_projection.rs` | Distinguishes owned artifacts from candidate-only or out-of-scope files. | ArtifactGraph / recovery contract | Partial | Adopt | `ArtifactOwnershipDecision` now carries top-level ownership, reason, source of truth, workspace scope summary, candidate origin, repair admissibility, and subreason labels for read-only, verifier, scaffold, setup, generated, and dependency/cache cases. | Connect ownership decisions to more completion-evidence producers and repeated-target exclusion. |
| Artifact ledger | `artifact_ledger.rs`, `artifact_ledger_state.rs`, `repo_edit_observation.rs`, `post_tool_reconciliation.rs` | Records per-turn artifact observations, edits, scaffold deltas, and verifier observations. | Minimal loop result / step runner evidence | Partial | Adopt | `Read`, `Write`, and `Edit` tool records now retain normalized target paths; repair state reconciles tool records, bounded workspace snapshot observations, verifier mentions, scaffold/setup deltas, ownership reasons, and ledger eval fields into `ContractEvidence` / `RecoveryTaskContract`. | Add focused eval cases for all ledger signals and stronger pass-side completion authority in later phases. |
| Completion evidence | `completion_evidence.rs`, `success.rs`, `completion_probe_gate.rs`, `objective_evidence.rs`, `evidence_observation.rs` | Converts actual tool/build/doc/data observations into completion authority. | Step verifier, final-answer guard, eval | Partial | Adopt | Typed `completion_evidence` can carry verifier exit, command observation, repo edit, docs/data/report pass/fail records through ContractEvidence, RecoveryTaskContract, and EvidenceEnvelope orchestration. Verifier failures now emit failed verifier completion evidence. | Add pass-side repo edit/docs/data/report producers and use completion evidence in final/eval summaries. |
| Evidence binding | `evidence_binding.rs`, `evidence_runner.rs`, `evidence_binding` adapters | Checks whether a deliverable can bind to its evidence runner before execution. | Verifier/profile/setup | Partial | Adopt | `EvidenceBindingPlan` can render missing/failed/unbound binding facts, map them to `evidence_binding_repair`, and carry them through recovery packets/envelopes. | Add concrete producers for manifest identity, docs section, schema output, source citation, and route/import binding checks. |
| Deliverable obligation audit | `deliverable_obligation_audit.rs`, `task_contract_deliverable_projection.rs`, `task_contract_deliverable_lifecycle.rs`, `deliverable_freshness.rs` | Audits required deliverables, freshness, and non-coding artifact obligations. | Plan lint / eval / profile | Partial | Adopt | `DeliverableObligation` records kind, path, required evidence, and freshness rules for ContractEvidence/RecoveryTaskContract. | Connect obligation projection to plan/profile/eval producers and add read-only freshness checks. |
| Active job arbiter | `active_job_arbiter.rs`, `active_job_emit.rs`, `actor_loop_phase_decision.rs`, `loop_phase.rs`, `model_request_phase.rs` | Selects the current recovery owner/job and loop control action before model action. | Recovery orchestration | Partial | Adopt | `active_job_priority` sorts/labels recovery tasks and helps select repair envelope. | Add `LoopControlAction`, `RecoveryOwner`, dispatch gate, lifecycle state, and deterministic next action. |
| Recovery owner / dispatch gate | `active_job_arbiter.rs`, `repair_job_dispatch.rs`, `artifact_recovery_flow.rs` | Prevents multiple recovery systems from acting at once. | Recovery orchestration | Partial | Adopt | `recovery_owner` is projected from the selected active job and rendered into ContractEvidence, RecoveryTaskContract, and EvidenceEnvelope orchestration. | Add a stricter dispatch gate for competing owners and explicit tie-safe stop. |
| Recovery messages and packets | `recovery_messages.rs`, `repair_packet.rs`, `failure_packet.rs`, `safe_stop_payload.rs`, `safe_stop_emit.rs` | Renders structured failure/repair/safe-stop information. | Recovery task / repair packet / final error | Partial | Partial | Recovery task renders structured contract fields and bounded repair packets. | Add safe-stop payloads tied to owner/job/action and evidence binding/completion failure classes. |
| Setup bootstrap | `node_runner_manifest.rs`, `package_manifest_summary.rs`, `cargo_manifest_summary.rs`, `setup_artifact_validation.rs`, `node_request_helpers.rs`, `python_request_helpers.rs` | Treats dependency/toolchain setup and setup manifests as separate active jobs. | Setup runtime / recovery orchestration | Partial | Partial | Dependency-missing evidence maps to `setup_bootstrap`; NPM manifest shape is checked. | Add setup job lifecycle, setup readiness, command authority, Cargo/Python validation, candidate-content validation, and setup result ledger. |
| Project probe/profile/scaffold profile | `project_probe.rs`, `project_profile.rs`, `project_profile_projection.rs`, `scaffold_profile.rs`, `scaffold_pipeline.rs` | Detects project shape and scaffold expectations before repair. | Profiles / artifact graph | Partial | Partial | Next.js profile carries richer project facts; other profiles are thinner. | Add common project/profile output schema and scaffold artifact ownership, without making profiles workflow engines. |
| Profile failure to recovery job | `project_profile_projection.rs`, `scaffold_profile.rs`, profile modules | Routes profile verification failure to a recovery job/action/target. | Recovery orchestration | Partial | Partial | Profile evidence can become job/action/target fields. | Add typed failure-specific mapping across profiles: route vs manifest vs source vs setup vs scaffold. |
| Semantic failure report | `semantic_failure.rs`, `semantic_repair_planning.rs`, `verifier_diagnostic_payload.rs`, `verifier_assessment_parser.rs` | Structures verifier failure into kind, clusters, observed/expected, affected cases, conflicts, and target candidates. | Evidence / recovery contract | Partial | Adopt | `semantic_failure_kind`, candidate artifacts, observed/expected pairs, failure signatures, deterministic cluster id, diagnostic code, preferred repair role, admitted cluster targets, and confidence bounds are now carried through repair/eval evidence. | Complete conflict objects, richer proposed/admitted cluster target ranking, and cross-profile source-of-truth adapters. |
| Semantic repair plan | `repair_job.rs`, `semantic_repair_planning.rs`, `task_contract_recovery_planning.rs` | Chooses one cluster, authority, repair role, hypothesis, and expected improvement. | Recovery task contract | Missing | Adopt | Only strings such as `source_of_truth` and `expected_evidence_delta` are rendered. | Add cluster-level plan slot, exhausted clusters, role strategy, and expected-improvement tracking. |
| Repair brief | `repair_brief.rs`, `repair_framework_findings.rs` | Gives repair task root cause, target, allowed change kind, preservation constraints, and confidence. | Recovery task contract | Partial | Adopt | `allowed_change_kind`, `source_of_truth`, disallowed actions, and expected delta are rendered. | Add structured root cause, concrete fix intent, `must_preserve`, target confidence, low-confidence rejection. |
| Repair action space | `repair_action.rs`, `repair_action_space.rs`, `repair_authority.rs`, `repair_plan.rs`, `repair_plan_admission.rs` | Validates whether a selected action is admissible for selected target/authority. | Recovery orchestration | Partial | Adopt | `RecoveryActionKind`, `ToolPolicyProjection`, `allowed_change_kind`, and `RepairActionPlan` status/tool category/rejection/source-of-truth records exist. | Add lifecycle transitions and stricter target-role/path match rejection before repair prompt rendering. |
| Repair target decision/admission | `repair_target_decision.rs`, `repair_target_admission.rs`, `recovery_targets.rs`, `set_artifact_recovery_target.rs`, `verifier_repair_targeting.rs` | Chooses and admits targets for the active failure. | Recovery orchestration / ArtifactGraph | Partial | Adopt | Setup jobs admit setup artifacts, source repair rejects tests, verifier contract targets `step:*`, generated/cache paths are rejected. | Add scope-aware ownership, source-of-truth authority, cluster target admission, edited/scaffold/verifier signals, and repeated-target exclusion. |
| Repair target prioritization | `semantic_repair_planning.rs`, `repair_job.rs`, `verifier_failure_artifacts.rs` | Orders targets by failure kind, authority, role, and progress history. | Recovery orchestration | Partial | Adopt | Target role priority prefers entrypoint/integration/source over setup/test/docs for verifier failures. | Add failure-kind-specific ranking, authority-based implementation-vs-test choice, and exhausted-target handling. |
| Repair job state machine | `repair_job.rs`, `repair_job_dispatch.rs`, `repair_lifecycle.rs`, `repair_progress.rs`, `verifier_repair_pass_flow.rs` | Tracks verifier repair attempts, rerun outcome, repeated signatures, target exhaustion, and safe stop. | Repair loop / recovery task | Partial | Adopt | `RepairJobState` can render active job, current target, exhausted target/role, and attempt records; verifier failures now attach a repair-job-state line. | Add persistent repair job state, repeated signature thresholds, rerun outcome classification, and safe-stop report. |
| Repair attempt ledger | `repair_attempt_outcome.rs`, `repair_progress.rs`, `repair_job.rs` | Records attempt outcome by target and failure cluster. | Recovery task / repair loop | Partial | Adopt | Prior attempts are string summaries; `RepairAttemptOutcomeKind` and deterministic no-progress classification helpers exist. | Connect exact attempt outcomes to repair loop events and exhausted target lists. |
| No-progress recovery | `no_progress_recovery.rs`, `repair_progress.rs` | Detects same target/role producing no progress and switches strategy or stops. | None | Missing | Adopt | No equivalent no-progress policy exists. | Add bounded cluster-scoped target/role bans and strategy switch: role switch, evidence binding, contract conflict, scaffold rebuild, or explicit stop. |
| Verifier diagnostic assessment | `verifier_diagnostic_flow.rs`, `verifier_diagnostic_attempt.rs`, `verifier_diagnostic_prompt.rs`, `verifier_assessment_parser.rs`, `verifier_weak_reason.rs`, `verifier_weak_repair_target.rs` | Parses verifier failure and classifies likely cause/action. | Verifier / evidence | Partial | Adopt | Command, diagnostic, failure kind/signature, source excerpt, candidates, diagnostic payload schema, initial Python/FastAPI/Rust/Next.js/common diagnostic codes, observed/expected pairs, affected cases, preferred repair role, weak source-grep/self-referential verifier reasons, and admitted cluster targets are carried. | Add deeper language-specific assessment, richer weak target filters, and verifier attempt flow parity. |
| Verifier orchestration | `verifier_orchestration.rs`, `verifier_driver.rs`, `verifier.rs`, `verifier_skill.rs`, `verifier_observation.rs`, `emit_verifier_events.rs`, `verifier_evidence_scope.rs` | Runs verifier, classifies pass/fail, decides whether repair continues or stops. | Step runner verifier / repair loop | Partial | Adopt | Original verifier rerun is referenced by repair packet; build/test commands run as explicit verify steps. | Add verifier repair flow, failure-attempt limits by job, rerun outcome events, binding scope, and safe-stop report. |
| Verifier command policy | `verifier_command_policy.rs`, `node_test_evidence_quality.rs`, `generated_test_guard.rs`, `test_expectation_audit.rs` | Rejects brittle, self-referential, or unsupported generated tests/verifiers. | Plan lint / verifier selection | Partial | Adopt | Source-grep and some verifier checks exist; generated-test quality is not a first-class policy. | Add generated-test preflight, self-referential verifier detection, unsupported contract assertion filtering, and expectation audit. |
| Artifact completion job | `artifact_completion_job.rs`, `artifact_completion_record.rs`, `artifact_recovery_flow.rs` | Creates or repairs missing required deliverables. | ArtifactCompletionJob / recovery orchestration | Partial | Adopt | Missing required artifacts can become artifact completion evidence. | Bind completion to artifact ledger, ownership, deliverable freshness, and missing-evidence distinction. |
| Focused edit recovery | `focused_edit_recovery.rs`, `read_target_helpers.rs`, `tool_history.rs`, `post_edit_excerpt.rs` | Directs repair to a previously read/editable target. | None | Missing | Missing | No focused-edit recovery state. | Consider after artifact ledger; add only as target-admission evidence, not provider-specific behavior. |
| Forced small edit / deterministic fallback | `forced_small_edit.rs`, `deterministic_fallback_plan.rs`, `mechanical_compile_repair.rs` | Performs or steers narrowly scoped edits after repeated non-progress. | None | Missing | Missing | No deterministic fallback edit mechanism. | Defer until repair action/target contracts are strong enough; avoid hidden mutation. |
| Repair patch executor/validation | `repair_patch_executor.rs`, `repair_patch_validation.rs`, `patch_provider.rs`, `patch_proposal.rs`, `repair_test_weakening_filter.rs` | Applies bounded patches, rejects unsafe/noop/duplicate/test-weakening patches, can rollback worsened patches. | Tools / minimal loop | Partial | Adopt | `PatchValidation` and a deterministic test-weakening detector exist as contract data; model edits still use normal tools. | Connect patch validation to repair attempt evaluation and allow rollback only when verifier-proven worsened. |
| Contract conflict job | `contract_conflict_job.rs`, `spec_authority.rs`, `api_contract_expectation.rs` | Handles implementation-vs-test-vs-docs/API conflicts explicitly. | None | Missing | Adopt | Some source/test distinction exists through target priority. | Add conflict object, source-of-truth decision, spec authority, and safe stop on ambiguous authority. |
| Language-specific mechanical repair | `rust_binding_repair.rs`, `cargo_dependency_repair.rs`, `repair_python_import_evidence.rs`, `repair_python_test_analysis.rs`, `repair_assertion_analysis.rs`, `python_markers.rs` | Applies narrow compile/import/assertion/dependency fixes for known languages. | Profiles / verifier evidence | Missing | Missing | Python import candidates and generic verifier candidates exist. | Do not add before common repair action/target contracts are stable; then add as bounded adapters. |
| Tool policy and effective policy | `tool_policy.rs`, `tool_policy_decisions.rs`, `effective_tool_policy_flow.rs`, `tool_call_prepare.rs`, `tool_prep.rs`, `tool_call_execution.rs`, `tool_execution.rs` | Projects current contract/job into allowed tool categories. | Minimal loop guards, step policy, recovery task | Partial | Partial | Step tool policy and recovery tool-policy projection exist. | Add owner/action-aware tool category projection for repair jobs, setup jobs, and evidence binding. |
| Tool failure recovery | `tool_failure_recovery.rs`, `reply_retry.rs`, `reply_retry_types.rs`, `protocol.rs` | Handles malformed tool calls, prose-only failures, and bounded retry notes. | Provider parser, minimal loop guards, recovery task | Partial | Partial | Tool protocol evidence and correction exhaustion exist. | Add protocol failure as a first-class recovery owner/action, not just evidence text. |
| Bash/setup command classification | `commands.rs`, `bash_policy_e2e_tests.rs`, `tester_invocation.rs`, `tester.rs`, `auto_test.rs`, `node_runner_manifest.rs` | Classifies local commands and test runners separately from ordinary mutation. | Bash tool, verifier, setup runtime | Partial | Partial | Build/test/setup checks exist in tool/verifier policy. | Add evidence binding and setup command authority before permitting more setup recovery. |
| Workspace candidates/walk | `workspace_candidates.rs`, `workspace_walk.rs`, `path_helpers.rs`, `workspace_access.rs` | Collects meaningful in-scope workspace files for contracts and ownership. | Eval/workspace scans, ArtifactGraph inputs | Partial | Adopt | Required artifacts and observed paths feed the graph. | Add scope-aware workspace walk, ignored-dir SSOT in recovery, and existing candidate discovery. |
| Job report / progress events | `job_report.rs`, `active_job_emit.rs`, `progress_text.rs`, `progress_tests.rs`, `footer.rs`, `spinner.rs`, `quality_gate.rs` | Emits structured active job, repair action, progress, and quality-gate outcomes. | Runtime events / eval reports | Partial | Partial | Evidence envelope and orchestration fields exist. | Add job-level report schema, active owner, repair action plan status, and attempt outcome events. |
| Scaffold pipeline | `scaffold_pipeline.rs`, `scaffold_profile.rs`, `scaffold_profile_e2e_tests.rs`, `scaffold_coding_guard_e2e_tests.rs` | Materializes project/profile scaffold and tracks scaffold ownership. | Profiles / plan artifacts | Partial | Partial | Some profile-required artifacts and scaffold materialization evidence exist. | Add scaffold profile as setup/artifact contract, not an independent workflow engine. |
| Data/docs/research/ops evidence | `structured_data_observation.rs`, `task_contract_data_output_context.rs`, `data_capability_e2e_tests.rs`, `research_acceptance_e2e_tests.rs`, `ops_capability_e2e_tests.rs`, `authoring_style.rs` | Handles non-coding deliverables and their evidence. | Profiles / eval | Partial | Partial | Docs/data profiles have basic required-path/content behavior. | Add generic evidence binding/completion evidence before profile-specific expansion. |
| Answer-only and work-mode gating | `answer_only_mode.rs`, `work_mode_confirm.rs`, `answer_only` tests, `confirmation_flow.rs`, `work_mode_confirm.rs` | Keeps non-mutating answer-only tasks from being forced into file edits. | Minimal loop final-answer guard / step policy | Partial | Partial | Final-answer and no-tool guards exist, with some broadening constraints. | Keep as policy gate; avoid broadening into normal coding repair. |
| Interruption, lifecycle, turn state | `interrupt.rs`, `lifecycle.rs`, `loop_state.rs`, `turn_state.rs`, `prepare_actor_loop_state.rs`, `run_turn.rs`, `actor_loop_flow.rs`, `summary.rs` | Maintains loop lifecycle and terminal outcomes. | CLI/repl/minimal loop/session | Partial | Partial | Minimal loop/session state exists. | Do not port full actor-loop complexity; only add state required by explicit recovery contracts. |
| Provider/model request plumbing | `model_request.rs`, `build_request_messages.rs`, `model_request_phase.rs`, `streaming_reply.rs` | Builds model messages and phases. | Providers / minimal loop prompt | Partial | Partial | Provider transports and minimal prompts exist. | Keep transport/prompt clean; do not put recovery policy into provider-specific branches. |
| Provider transport parser | provider modules, XML/native parser equivalents | Parses native or fallback tool calls without owning behavior policy. | Providers | Implemented | Adopt | Provider transport is separate from planning/recovery; XML fallback remains compatibility fallback. | Continue provider-native parsing but keep behavior policy outside transports. |
| Working memory/reminders | `working_memory_messages.rs`, `reminder.rs`, `reminder_pipeline.rs`, `precaution_relevance.rs` | Adds memory/reminder guidance to model turns. | None | Excluded | Excluded | No equivalent. | Keep excluded unless a separate design decision admits memory/advisory systems. |
| Case record and anti-pattern corpora | `case_record_flow.rs`, `case_record_extract.rs`, `anti_pattern_flow.rs`, recovery masking modules | Records cases and retrieves anti-pattern guidance. | None | Excluded | Excluded | No equivalent. | Keep excluded for MVP; not needed for explicit contract recovery. |
| PAM/Photon advisory | `pam_advisory.rs`, `photon_feedback_derive.rs`, `photon_user_feedback.rs`, related tests | Adds sidecar/advisory feedback. | None | Excluded | Excluded | No equivalent. | Keep excluded; would reintroduce advisory stack. |
| Quality classification/confirmation | `quality.rs`, `quality_confirm.rs`, `feedback_kind_confirm.rs`, `task_classification.rs` | Classifies quality/feedback/task intent through secondary confirmation. | Eval/profile/final guard | Missing | Missing | Some eval quality checks exist outside runtime. | Keep as lower priority; adopt only deterministic quality gates, not semantic advisory. |
| Slash/plan/command UI helpers | `slash_commands.rs`, `plan_sections.rs`, `plan_mode_helpers.rs`, `commands.rs`, `tool_display.rs`, `message_push.rs`, `footer.rs` | User-interface and rendering helpers. | CLI/repl/slash command | Partial | Missing | CommandAgent has independent CLI/slash implementation. | Not a recovery-parity target unless UX/eval evidence shows a gap. |
| Legacy engine selector | historical legacy switches | Switches between multiple engines/controllers. | None | Excluded | Excluded | No equivalent. | Keep excluded. CommandAgent has one execution engine. |
| Hidden or unbounded repair loop | historical broad recovery loops | Continues internally until success. | None | Excluded | Excluded | Repair remains bounded and user-visible. | Keep excluded. |
| Provider/model-specific behavioral policy | historical provider/model branches | Changes repair/planning behavior for one provider or model. | None | Excluded | Excluded | Shared behavior lives outside provider transports. | Keep excluded. |
| Model-issued dependency installation | model-driven setup execution | Lets the model decide to run dependency setup implicitly. | None | Excluded | Excluded | Setup requires explicit policy/evidence and remains verifier-owned. | Keep excluded; setup can be a visible active job. |

## Inventory Coverage Summary

This table currently groups the source `loop_run` surface into 54 functional
rows. It is grouped by responsibility rather than listing every helper or test
file one by one.

Current implementation status:

| Current status | Count |
| --- | ---: |
| Implemented | 1 |
| Partial | 39 |
| Missing | 7 |
| Excluded | 7 |

Adoption decision:

| Adoption decision | Count |
| --- | ---: |
| Adopt | 25 |
| Partial | 17 |
| Missing | 5 |
| Excluded | 7 |

Interpretation:

- `Adopt` + `Partial` is the accepted migration surface. There are 42 rows
  where CommandAgent should own the responsibility, either as a new mechanism
  or by completing an existing projection.
- Of those 42 rows, 40 still require implementation work. Provider transport
  parsing is implemented, and artifact ledger work now has a first partial
  slice for exact write/edit paths.
- `Missing` is intentionally visible debt, not a rejection. These rows need a
  priority decision after the foundational recovery contracts land.
- `Excluded` rows are not gaps for the current architecture.

The table is not a byte-for-byte file inventory. Test modules, e2e fixtures,
formatting helpers, UI progress helpers, and small path/string helpers are
covered by their owning responsibility row unless they encode distinct runtime
control.

## What Is Currently Too Thin

The current implementation is useful, but the following mappings are too thin
to resolve the recurring eval failures reliably:

1. `active_job_priority` is metadata, not a real arbiter.
2. `workspace_scope` now has a typed detector for greenfield,
   single-project, explicit, and ambiguous-parent scopes, but it is not yet fed
   by a scope-aware workspace walk.
3. `artifact_ownership` now distinguishes owned, candidate-only, and
   out-of-scope targets for target admission, but it still needs more signals
   from scaffold deltas and verifier ownership.
4. Tool records now know exact `Write`/`Edit` paths; verifier observations and
   post-tool reconciliation are still missing from the artifact ledger.
5. Semantic failure data now has deterministic diagnostic clusters for verifier
   failures, but conflict handling and cluster-level target ranking are still
   partial.
6. `allowed_change_kind` is guidance, not an admissible repair action plan.
7. Setup validation only covers NPM `package.json`.
8. Completion evidence is not a single runtime authority.
9. Evidence binding is not a first-class failure class.
10. Deliverable freshness and obligation audits are not centrally enforced.
11. No-progress recovery cannot ban a failed target/role or switch strategy.
12. Tool policy is not yet projected from recovery owner/action in a single
    place.

## What Changed In The Latest Slice

- Verifier repair no longer treats a pytest-owned test artifact as the default
  source repair target when a plan-owned source artifact is available.
- Next.js build failures with route-level diagnostics can target the selected
  route from the plan's required artifacts even when the verify step has no
  expected path.
- Tool protocol failures that repeat after one correction carry the exhausted
  correction fact in the early contract evidence/attempt ledger so repair
  packets remain useful under truncation.
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
