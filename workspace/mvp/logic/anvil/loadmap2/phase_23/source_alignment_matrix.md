# Phase23 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C04 | `task_contract_artifact_contract.rs`, `task_contract_artifact_predicates.rs`, `task_contract_artifact_intent.rs`, `artifact_target_alignment.rs` | Use one deterministic artifact-role taxonomy for setup, implementation, test, docs, data, route/integration, generated, dependency/cache, and build output paths; ensure profile verification, verifier repair/admission, recovery admission, and eval reporting consume compatible role facts. | Broad semantic artifact intent inference, hidden role confirmation prompts, and profile-specific workflow selection based on role. | `src/agent/step_runner/artifact_graph.rs`, `profile_artifact.rs`, `plan_lint/mod.rs`, `target_admission.rs`, `artifact_completion.rs`, `scripts/eval_report.py`, `scripts/eval_agent_slice.sh` | Unit tests for role classification and consumers; focused role proof; broad sign-off regression. |
| C05 | `task_workspace_scope.rs`, `workspace_access.rs`, `workspace_candidates.rs`, `workspace_walk.rs`, `workspace_paths.rs` | Decide claimable task scope from bounded workspace facts for greenfield, single-project, explicit root, ambiguous parent, and ignored/excluded dependency/cache/build/generated paths; make ownership and target consumers use the same scope fact. | Hidden full-repo crawler, unbounded workspace indexing, advisory workspace memory, or automatic migration of unrelated project roots. | `src/agent/step_runner/workspace_snapshot.rs`, `workspace_scope.rs`, `artifact_graph.rs`, `artifact_ownership.rs`, `target_admission.rs`, `recovery_contract.rs`, `runtime/repair_loop.rs` | Workspace scope/snapshot tests, ownership/target admission tests, focused scope proof. |
| C06 | `artifact_ownership.rs`, `owned_test_projection.rs`, `artifact_state_projection.rs` | Distinguish owned artifacts from candidate-only, read-only, verifier-only, generated, dependency/cache, build output, scaffold/setup, and out-of-scope files; feed decisions into target admission, completion eligibility, repeated-target exclusion, and eval reporting. | Anvil's broader artifact state projector if it implies hidden job control, semantic ownership guessing, or unbounded repair target search. | `src/agent/step_runner/artifact_ownership.rs`, `target_admission.rs`, `artifact_completion.rs`, `evidence_authority.rs`, `recovery_orchestration.rs`, `runtime/repair_loop.rs`, `scripts/eval_report.py` | Ownership/target/completion/evidence-authority tests, focused ownership proof, broad sign-off regression. |

## Review Result

Review findings applied:

- Mapped every Phase23 coverage row to explicit Anvil source files and
  CommandAgent target modules.
- Marked omitted Anvil behavior so Phase23 does not import hidden crawlers,
  semantic ownership guessing, or workflow selection.
- Required consumer proof per row, not only producer existence.

## Closure Result

| coverage id | final decision | proof root |
| --- | --- | --- |
| C04 | Implemented / closed_proven | `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023` |
| C05 | Implemented / closed_proven | `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023` |
| C06 | Implemented / closed_proven | `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023` |
