# Phase 4 Target Admission, Semantic Plan, And Repair Brief

Date: 2026-06-21

## Scope

Phase 4 adds a deterministic gate between active-job dispatch and repair prompt
rendering.

The runtime now carries:

- proposed repair targets
- admitted repair targets
- rejected repair targets with reasons
- selected target and role
- selected deterministic failure cluster
- repair brief lines
- repair brief status
- action envelope status

## Design Boundary

This is contract data, not another execution engine.

Target admission consumes existing deterministic facts from ArtifactGraph,
ArtifactLedger, WorkspaceScope, ArtifactOwnership, active job, and repair
action. Semantic repair planning selects a bounded cluster and hypothesis from
existing contract evidence. RepairBrief renders the admitted target, action
envelope, disallowed actions, success check, and rerun authority for the
existing Recovery Task Contract.

If no target/action is admitted, the repair brief becomes explicit stop. The
runtime must not broaden the prompt, increase retry count, select a future
phase, or add provider/model-specific behavior.

## Focused Eval Expectations

Focused eval should cover:

- route integration wrong-target rejection
- setup/manifest target admission
- missing artifact scaffold target
- source repair generated/cache rejection
- tool protocol correction with no file target
- docs/test role-specific repair

Eval reports should surface the target funnel and repair brief status without
requiring raw log inspection.
