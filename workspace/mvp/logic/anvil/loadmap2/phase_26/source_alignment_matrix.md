# Phase26 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C13 | `recovery_messages.rs`, `repair_packet.rs`, `failure_packet.rs`, `safe_stop_payload.rs`, `safe_stop_emit.rs` | Render structured recovery, repair, and safe-stop packets with owner/job/action/target/cluster/attempt context. | Hidden continuation after safe stop, success masking, or model-prose-only repair instructions. | `recovery_task.rs`, `repair.rs`, `repair_job.rs`, eval scripts | recovery-task tests; safe-stop focused fixtures; broad sign-off. |
| C14 | `node_runner_manifest.rs`, `package_manifest_summary.rs`, `cargo_manifest_summary.rs`, `setup_artifact_validation.rs`, `node_request_helpers.rs`, `python_request_helpers.rs` | Treat setup as a typed, policy-gated active job with setup manifest validation, readiness, command authority, stale setup, result ledger, and non-Node setup facts. | Implicit dependency installation, arbitrary setup Bash, or profile-specific setup workflow engines. | `setup_lifecycle.rs`, `setup_artifact_validation.rs`, `runtime/setup.rs`, `recovery_orchestration.rs`, eval scripts | setup lifecycle/setup validation tests; setup focused matrix; broad sign-off. |
| C15 | `project_probe.rs`, `project_profile.rs`, `project_profile_projection.rs`, `scaffold_profile.rs`, `scaffold_pipeline.rs` | Expose common project/profile/scaffold facts and bounded scaffold materialization/completion evidence. | Full scaffold pipeline scheduler or profile-owned file mutation. | `profiles.rs`, `profile_artifact.rs`, `artifact_graph.rs`, `artifact_completion.rs`, eval scripts | profile output/scaffold tests; scaffold focused fixture; broad sign-off. |
| C16 | `project_profile_projection.rs`, `scaffold_profile.rs`, profile modules | Map profile verification failures to typed recovery job/action/target facts across route, manifest, setup, source, scaffold, and explicit stop. | Profile final job arbitration or provider/model-specific profile behavior. | `profiles.rs`, `recovery_policy.rs`, `recovery_orchestration.rs`, eval scripts | profile mapping tests; profile-failure focused matrix; broad sign-off. |
| C17 | `semantic_failure.rs`, `semantic_repair_planning.rs`, `verifier_diagnostic_payload.rs`, `verifier_assessment_parser.rs` | Structure verifier failure into semantic report fields, conflict inputs, observed/expected, affected cases, clusters, candidate artifacts, and target ranking inputs. | Full contract conflict resolution, hidden semantic planner, or automatic repair execution. | `semantic_failure.rs`, `verifier_diagnostic.rs`, `recovery_contract.rs`, eval scripts | semantic-failure tests; verifier focused fixture; broad sign-off. |
| C18 | `repair_job.rs`, `semantic_repair_planning.rs`, `task_contract_recovery_planning.rs` | Render selected cluster, authority, repair role, hypothesis, expected improvement, success check, evidence delta, and exhausted cluster/role/target handoff. | Retry expansion, hidden role switching, or target admission decisions owned by semantic planning. | `semantic_failure.rs`, `recovery_task.rs`, `repair_job.rs`, `repair_brief.rs`, eval scripts | recovery-task/repair-state tests; semantic repair focused fixture; broad sign-off. |
| C19 | `repair_brief.rs`, `repair_framework_findings.rs` | Render root cause, target, allowed change kind, preservation constraints, confidence, disallowed actions, and expected evidence delta. | Repair brief selecting a different owner/action than dispatch or hiding weak confidence. | `repair_brief.rs`, `recovery_task.rs`, eval scripts | repair-brief tests; focused repair brief fixture; broad sign-off. |
| C20 | `repair_action.rs`, `repair_action_space.rs`, `repair_authority.rs`, `repair_plan.rs`, `repair_plan_admission.rs` | Validate selected action family, authority, target role, source of truth, tool policy, no-change contracts, and action-envelope lifecycle. | Patch execution, rollback execution, unbounded repair planning, or action mutation after prompt rendering. | `repair_action_plan.rs`, `recovery_orchestration.rs`, `recovery_policy.rs`, `recovery_task.rs`, eval scripts | repair-action-plan/action-envelope tests; focused action-envelope matrix; broad sign-off. |

## Review Result

Review findings applied:

- Mapped every C13-C20 row to explicit Anvil sources and CommandAgent target
  modules.
- Marked omitted Anvil behavior so Phase26 imports useful contracts without
  importing hidden orchestration.
- Kept Phase27 target/verifier/patch behavior and Phase28 contract conflict
  resolution outside Phase26.

## Implementation Result

Phase26 implemented the adopted C13-C20 contract surfaces in CommandAgent and
proved them with unit tests plus focused deterministic fixtures. Omitted Anvil
behavior remains intentionally assigned to later rows:

- target prioritization, verifier orchestration, repair lifecycle, completion
  job, focused edit, and patch validation remain Phase27;
- full contract-conflict resolution remains Phase28;
- cross-profile/runtime-support expansion remains Phase29.
