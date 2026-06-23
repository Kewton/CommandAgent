# Phase26 Row Closure Matrix

Date: 2026-06-23 JST

| coverage id | current status | adoption | owner layer | missing contract | target modules | required proof | closure condition | disposition |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| C13 | Implemented | Partial | recovery task / repair packet / safe stop | closed: safe-stop and repair packet fields now cover evidence, completion, setup, profile, semantic, and action-envelope failures. | `recovery_task.rs`, `repair.rs`, `repair_job.rs`, eval scripts | `cargo test recovery_task`, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Packets expose owner/job/action/target/cluster/attempt/required/disallowed/rerun facts and never hide failure as success. | closed_proven |
| C14 | Implemented | Partial | setup lifecycle / setup runtime | closed: setup validation, result ledger, stale setup, command authority, and non-Node setup facts are rendered as setup lifecycle evidence. | `setup_lifecycle.rs`, `setup_artifact_validation.rs`, `runtime/setup.rs`, `recovery_orchestration.rs`, eval scripts | setup lifecycle/setup validation tests, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Setup is visible, policy-gated, evidence-bound, and covers Node/Rust/Python setup blockers without implicit execution. | closed_proven |
| C15 | Implemented | Partial | profiles / scaffold facts / artifact graph | closed: profiles expose common project/profile/scaffold facts, completion evidence, failure mappings, and capability status. | `profiles.rs`, `profile_artifact.rs`, `artifact_graph.rs`, `artifact_completion.rs`, eval scripts | profile output tests, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Profiles expose common project/scaffold facts and completion evidence without becoming workflow engines. | closed_proven |
| C16 | Implemented | Partial | profile failure mapping / recovery policy | closed: profile failures map to typed route, manifest, setup, source, scaffold, and explicit-stop recovery facts. | `profiles.rs`, `recovery_policy.rs`, `recovery_orchestration.rs`, eval scripts | profile mapping tests, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Route, manifest, setup, source, scaffold, and explicit-stop profile failures produce typed facts consumed by dispatch. | closed_proven |
| C17 | Implemented | Adopt | semantic failure report / verifier diagnostic | closed: semantic reports preserve conflict inputs, cluster/ranking facts, observed/expected pairs, affected cases, and unknown diagnostics. | `semantic_failure.rs`, `verifier_diagnostic.rs`, `recovery_contract.rs`, eval scripts | semantic-failure tests, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Semantic reports expose conflict inputs and ranking facts without resolving conflicts or executing repair. | closed_proven |
| C18 | Implemented | Adopt | semantic repair plan / recovery task | closed: selected cluster, role, hypothesis, expected improvement, expected delta, success check, and exhaustion handoff are rendered. | `semantic_failure.rs`, `recovery_task.rs`, `repair_job.rs`, `repair_brief.rs`, eval scripts | recovery-task/repair-state tests, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Repair task/brief render selected cluster, role, hypothesis, expected delta, success check, and exhausted state facts. | closed_proven |
| C19 | Implemented | Adopt | repair brief / recovery task | closed: repair brief exposes root cause, target, constraints, allowed/disallowed actions, confidence, preservation, and success check. | `repair_brief.rs`, `recovery_task.rs`, eval scripts | `cargo test repair_brief`, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Repair brief consumes dispatch/action facts and renders bounded repair instructions without recomputing owner/action. | closed_proven |
| C20 | Implemented | Adopt | repair action plan / action envelope | closed: action envelopes admit or reject action families before prompt rendering and record lifecycle/status evidence. | `repair_action_plan.rs`, `recovery_orchestration.rs`, `recovery_policy.rs`, `recovery_task.rs`, eval scripts | repair-action-plan/action-envelope tests, focused root `eval/runs/loadmap2-phase26-focused-fixtures/20260623T140340`, broad sign-off | Action envelope admits or rejects selected action families before prompt rendering and records lifecycle/status evidence. | closed_proven |

## Closure Rules

- `closed_proven` requires row-specific unit or fixture proof plus focused
  proof where listed.
- `split_forward` is allowed only for a narrower same-surface blocker with
  failed proof evidence, owner, downstream phase, and closure condition.
- Broad sign-off is regression evidence, not row proof.
- C13-C20 cannot be closed by docs alone, CI alone, field existence alone, or
  post-hoc eval derivation from reason text.

## Review Result

Review findings applied:

- Kept each coverage row independent so setup/profile/semantic/action-envelope
  proof cannot be merged into a vague recovery-task claim.
- Required selected repair and explicit-stop proof.
- Made Phase27 and Phase28 boundaries explicit in closure conditions.
