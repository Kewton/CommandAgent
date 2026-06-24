# Phase23 Implementation Tasks

Date: 2026-06-23 JST

## Task Status Legend

- `[ ]` not started
- `[~]` started / blocked on proof
- `[x]` complete

## Preparation

- [x] Confirm the current branch, dirty state, and CommandAgent commit before
      implementation.
- [x] Re-read `AGENTS.md`, `docs/philosophy.md`, `docs/architecture.md`,
      `docs/adr/0002-contract-recovery.md`, and `docs/evaluation.md`.
- [x] Confirm Anvil baseline against `anvil_source_baseline.md`.
- [x] Confirm C04-C06 still map to the same coverage rows before runtime
      changes.
- [x] Inspect current producers and consumers:
      - `profile_artifact.rs`
      - `artifact_graph.rs`
      - `workspace_scope.rs`
      - `workspace_snapshot.rs`
      - `artifact_ownership.rs`
      - `target_admission.rs`
      - `artifact_completion.rs`
      - `recovery_contract.rs`
      - `runtime/repair_loop.rs`

## C04: Artifact Role Taxonomy

- [x] Inventory all current role producers.
      - `ArtifactRole` in `artifact_graph.rs`
      - profile-specific classification in `profile_artifact.rs`
      - eval/report fallback classification in scripts
- [x] Identify consumers that still classify path roles independently.
      - target admission
      - verifier repair/admission
      - recovery admission
      - completion eligibility
      - eval report fields
- [x] Add or refactor a shared role-classification entrypoint if consumers
      still depend on divergent string heuristics.
- [x] Prove role classification for common profiles:
      - Next.js source/route/manifest/generated/cache/build output
      - Rust source/test/target output
      - Python source/test/cache/venv-like path
      - docs and data artifacts
- [x] Add tests that generated, dependency/cache, and build-output roles are
      rejected or marked non-owned before repair target selection.
- [x] Ensure profile verification and verifier repair use the same role facts
      or explicitly record a planned split if a consumer cannot be unified in
      Phase23.

## C05: Workspace Scope Admission

- [x] Inventory current workspace fact producers.
      - `WorkspaceSnapshot`
      - `WorkspaceScope`
      - `ArtifactGraph`
      - path confinement / safety utilities
- [x] Prove scope kind selection for:
      - greenfield workspace
      - single project root
      - explicit project root
      - ambiguous parent
      - ignored dependency/cache/build output paths
      - excluded/generated paths
- [x] Add or complete task-claimable scope admission rules where deterministic
      facts already exist.
- [x] Ensure scope evidence is visible to target admission, artifact
      ownership, recovery contract rendering, and eval reporting.
- [x] Add tests that ambiguous parent scope does not silently admit unrelated
      project files as owned targets.
- [x] Add tests that dependency/cache/build outputs do not expand the task
      scope.

## C06: Artifact Ownership

- [x] Inventory ownership decision fields and source-of-truth labels in
      `artifact_ownership.rs`.
- [x] Connect ownership decisions to target admission.
      - owned implementation target may be admitted
      - generated/cache/build output is rejected
      - read-only observation is not an owned edit target
      - verifier-mentioned path is evidence, not automatic ownership
- [x] Connect ownership decisions to completion eligibility.
      - completion may use owned required artifacts
      - completion must not treat candidate-only/cache/generated paths as
        owned deliverables
- [x] Connect ownership decisions to repeated-target exclusion or no-progress
      evidence where the current repair loop already has deterministic attempt
      facts.
- [x] Add tests proving ownership source, reason, scope summary, and role are
      rendered or reported.
- [x] Add or select focused fixture expectations for ownership/status fields.
      A focused proof root is required for Phase23 closure even if no new
      fixture file is needed.

## Documentation

- [x] Update `docs/architecture.md` if role/scope/ownership boundaries change.
- [x] Update `docs/evaluation.md` if eval fields or report sections change.
- [x] Update `docs/profiles.md` if profile artifact classification behavior
      changes.
- [x] Update `docs/ultra-plan-run.md` if planner-facing contract guidance
      changes.
- [x] Add Phase23 eval report under `docs/eval/`.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
      C04-C06 proof supports status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
      if KI-002 status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md` or
      `README.md` only if exit gates or authority rules change.

## Evaluation

- [x] Run `cargo fmt --check`.
- [x] Run targeted Rust tests for:
      - `profile_artifact`
      - `artifact_graph`
      - `workspace_scope`
      - `workspace_snapshot`
      - `artifact_ownership`
      - `target_admission`
      - `artifact_completion`
- [x] Run `python3 tests/test_eval_report.py` if eval report fields change.
- [x] Run `cargo test`.
- [x] Run focused eval proof:
      - `focused-artifact-role-scope-ownership`, when no existing case proves
        C04-C06 fields
      - otherwise existing focused cases that explicitly assert
        role/scope/ownership fields
- [x] Run broad sign-off after behavior changes.

## Review Checklist

- [x] Every C04-C06 task has an owner layer and proof command.
- [x] No task closes by docs alone or CI alone.
- [x] No provider/model-specific runtime branch is introduced.
- [x] No hidden retry or hidden repair loop is introduced.
- [x] No profile becomes a workflow engine.
- [x] `source_alignment_matrix.md` and `reconciliation.md` agree with the
      coverage table.
- [x] Any split-forward row is narrower, same-surface, evidence-backed, and
      assigned to a downstream phase.

## Review Result

Review findings applied:

- Split role taxonomy, workspace scope, and ownership into separate row-level
  task groups so C04 proof cannot mask C05/C06 gaps.
- Added consumer-oriented tasks for target admission, completion eligibility,
  recovery admission, and eval reporting. This prevents closing Phase23 with
  producer-only foundation types.
- Added cross-profile role coverage to keep the design common rather than
  Next.js-specific.
- Added explicit boundaries against C07-C12 work so Phase23 does not absorb
  artifact ledger, completion/evidence binding, or active-job dispatch scope.
- Strengthened focused proof from conditional to mandatory-at-closure, while
  allowing the implementation to reuse an existing focused case if it proves
  C04-C06 fields explicitly.
