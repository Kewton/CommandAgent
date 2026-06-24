# Phase25 Source Alignment Matrix

Date: 2026-06-23 JST

| coverage id | Anvil source files | adopted behavior | intentionally omitted behavior | CommandAgent target modules | proof method |
| --- | --- | --- | --- | --- | --- |
| C11 | `active_job_arbiter.rs`, `active_job_emit.rs`, `actor_loop_phase_decision.rs`, `loop_phase.rs`, `model_request_phase.rs` | Select one active recovery owner/job/action from deterministic candidates before model repair; expose lifecycle state, candidate list, dispatch reason, tie-break reason, no-owner, ambiguous tie, and conflict-stop evidence. | Full actor-loop phase engine, hidden phase switching, model-request phase scheduler, sidecar arbiter, or unbounded continuation. | `active_job.rs`, `recovery_orchestration.rs`, `recovery_contract.rs`, `evidence.rs`, eval scripts | Active-job/recovery-orchestration tests; focused dispatch fixture; broad sign-off regression. |
| C12 | `active_job_arbiter.rs`, `repair_job_dispatch.rs`, `artifact_recovery_flow.rs` | Prevent multiple recovery systems from acting at once by requiring a single selected owner/action or explicit stop before repair prompt rendering. Candidate producers feed one dispatch gate. | Independent recovery subsystems acting concurrently, profile-specific workflow dispatch, implicit setup execution, or provider/model-specific dispatch policy. | `recovery_orchestration.rs`, `recovery_policy.rs`, `recovery_task.rs`, `repair_brief.rs`, `repair_action_plan.rs`, `runtime/repair_loop.rs`, eval scripts | Recovery-orchestration/recovery-task tests; focused owner/action proof; broad sign-off regression. |

## Review Result

Review findings applied:

- Mapped every Phase25 row to explicit Anvil source files and CommandAgent
  target modules.
- Marked the omitted Anvil actor-loop behavior so Phase25 imports the useful
  arbitration contract without importing hidden orchestration.
- Required proof that dispatch is consumed before prompt rendering, not merely
  reported after failure.

## Implementation Result

Phase25 implemented the adopted C11-C12 responsibilities in the CommandAgent
contract stack:

- C11 active-job arbitration evidence now includes lifecycle projection and
  explicit stop-state evidence.
- C12 dispatch output is consumed by recovery task rendering before bounded
  repair prompt construction.
- The intentionally omitted Anvil actor-loop scheduler and hidden phase
  switching remain out of scope.

Proof root: `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`.
Broad sign-off: `status: pass`.
