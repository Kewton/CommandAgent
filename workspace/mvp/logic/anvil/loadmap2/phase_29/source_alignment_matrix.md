# Phase29 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C34 | `rust_binding_repair.rs`, `cargo_dependency_repair.rs`, `repair_python_import_evidence.rs`, `repair_python_test_analysis.rs`, `repair_assertion_analysis.rs`, `python_markers.rs` | Deterministic language-family repair evidence and admitted mechanical proposals for Rust, Python, TypeScript, Next.js, import/assertion/dependency diagnostics. | Direct mutation by adapter, hidden compile repair loop, provider/model-specific language policy, broad semantic guessing. | `mechanical_repair.rs`, `verifier_diagnostic.rs`, `semantic_failure.rs`, `repair_action_plan.rs`, `setup_artifact_validation.rs`, profile output facts | language adapter unit tests; focused language matrix if prompt/eval fields change; broad sign-off. |
| C35 | `tool_policy.rs`, `tool_policy_decisions.rs`, `effective_tool_policy_flow.rs`, `tool_call_prepare.rs`, `tool_prep.rs`, `tool_call_execution.rs` | Owner/action/job-aware effective tool policy, allowed tool category, disallowed actions, and policy reason. | Hidden tool-plan generation, provider-specific policy, retrying until a permitted tool is found. | `recovery_policy.rs`, `recovery_orchestration.rs`, `recovery_task.rs`, minimal-loop guards, eval report fields | tool policy tests; focused effective-policy fixture if model-facing packet changes. |
| C36 | `tool_failure_recovery.rs`, `reply_retry.rs`, `reply_retry_types.rs`, `protocol.rs` | Bounded correction facts for provider parse, schema, prose-only, stale edit, and invalid tool inputs. | Unbounded reply retry, provider/model behavior branches, converting tool failures into generic source repair. | provider parser outputs, `xml_fallback.rs`, minimal-loop guards, `recovery_task.rs`, `recovery_orchestration.rs`, eval report fields | tool failure tests; focused tool-failure fixture; report tests. |
| C37 | `commands.rs`, `bash_policy_e2e_tests.rs`, `tester_invocation.rs`, `tester.rs`, `auto_test.rs`, `node_runner_manifest.rs` | Command classification for verifier/setup/inspection/mutation/network/dependency and setup command authority. | Implicit dependency installation, model-issued setup execution, shell command weakening to pass tests. | `tools/bash.rs`, `verify.rs`, `runtime/setup.rs`, `setup_lifecycle.rs`, `setup_artifact_validation.rs`, `recovery_policy.rs` | bash/setup policy tests; Node/Cargo/Python setup command fixtures. |
| C38 | `workspace_candidates.rs`, `workspace_walk.rs`, `path_helpers.rs`, `workspace_access.rs` | Scope-aware workspace walk, candidate discovery, ignored-dir single source of truth, candidate exclusion reasons. | Treating candidates as owned deliverables, hidden broad scans after prompt rendering, path-order target selection. | `workspace_snapshot.rs`, `workspace_scope.rs`, `artifact_graph.rs`, `artifact_ownership.rs`, `target_admission.rs`, eval scans | workspace walk/scope tests; candidate discovery fixture. |
| C39 | `job_report.rs`, `active_job_emit.rs`, `progress_text.rs`, `progress_tests.rs`, `footer.rs`, `spinner.rs`, `quality_gate.rs` | Structured job report fields for owner, action plan status, lifecycle, attempt outcome, stop reason, and progress state. | UI-only spinner/footer parity, quality confirmation, hidden progress-driven continuation. | `active_job.rs`, `repair_job.rs`, `runtime/repair_loop.rs`, `evidence.rs`, eval report scripts | runtime/eval report tests; broad sign-off. |
| C40 | `scaffold_pipeline.rs`, `scaffold_profile.rs`, `scaffold_profile_e2e_tests.rs`, `scaffold_coding_guard_e2e_tests.rs` | Scaffold represented as setup/artifact obligations, materialization evidence, ownership, and safe stop. | Independent scaffold workflow engine, hidden project generator, profile-owned dispatcher. | `profiles.rs`, `profile_artifact.rs`, `artifact_completion.rs`, `setup_lifecycle.rs`, `recovery_orchestration.rs` | scaffold contract tests; focused scaffold fixture if recovery prompt changes. |
| C41 | `structured_data_observation.rs`, `task_contract_data_output_context.rs`, `data_capability_e2e_tests.rs`, `research_acceptance_e2e_tests.rs`, `ops_capability_e2e_tests.rs`, `authoring_style.rs` | Generic docs/data/research/ops evidence producers through completion evidence, evidence binding, deliverable obligation, and eval fields. | Profile-specific workflow engines, style/advisory sidecars, final-answer-only proof for required artifacts. | `completion_evidence.rs`, `evidence_binding.rs`, `deliverable_obligation.rs`, `evidence_producer.rs`, docs/data profile facts | non-coding evidence tests; focused non-coding matrix when model-facing behavior changes. |
| C42 | `answer_only_mode.rs`, `work_mode_confirm.rs`, answer-only tests, `confirmation_flow.rs` | Deterministic answer-only/work-mode gate that prevents mutation only for admitted non-work requests. | Broad no-tool guard for normal coding tasks, confirmation loops, semantic advisory classification. | `task_contract.rs`, minimal-loop guards, `recovery_policy.rs`, step policy/final-answer validation | final-answer/step-policy tests; focused answer-only fixture if needed. |
| C43 | `interrupt.rs`, `lifecycle.rs`, `loop_state.rs`, `turn_state.rs`, `prepare_actor_loop_state.rs`, `run_turn.rs`, `actor_loop_flow.rs`, `summary.rs` | Recovery-relevant lifecycle states such as interrupted, stopped, completed, exhausted, awaiting user, and explicit stop. | Full actor loop, hidden continuation, memory-based turn state, autonomous lifecycle scheduler. | `minimal_loop/result.rs`, `runtime/repair_loop.rs`, `repair_job.rs`, `session/*`, eval report fields | CLI/session/runtime tests; report tests when fields are added. |
| C44 | `model_request.rs`, `build_request_messages.rs`, `model_request_phase.rs`, `streaming_reply.rs` | Provider request plumbing remains transport-only; tool declarations, native/fallback parsing, usage attachment, prompt boundaries are policy-free. | Planning/recovery/profile policy in providers, provider/model-specific behavior branches, sidecar routing. | `providers/*`, `providers/planner.rs`, `providers/xml_fallback.rs`, `providers/usage.rs`, minimal-loop prompt boundary tests | provider unit tests; offline request-shape tests; docs/providers update. |

## Review Result

Review findings applied:

- Mapped every C34-C44 row to concrete Anvil source families and existing
  CommandAgent target modules.
- Marked omitted behavior explicitly to avoid importing hidden orchestration,
  advisory confirmation, or provider/model policy.
- Split support surfaces by owner layer so implementation cannot close Phase29
  with one generic runtime-support change.
- Added proof methods that distinguish unit/report proof from focused
  model-facing proof.

## Implementation Result

All C34-C44 rows are implemented at the accepted CommandAgent boundary:

- adopted behavior is represented as deterministic contract/report data;
- intentionally omitted Anvil behavior remains omitted and documented;
- focused fixture proof root:
  `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335`;
- broad sign-off with the Phase29 root as supplemental evidence returned
  `status: pass`.

No row required importing Anvil's hidden actor loop, advisory sidecars,
provider/model-specific behavior, or unbounded retry.
