# Loadmap2 Phase23 Plan

Date: 2026-06-23 JST

## Objective

Phase23 closes the Phase21 split-forward rows C04-C06:

| coverage id | responsibility |
| --- | --- |
| C04 | Artifact role taxonomy consumed by profile verification, verifier repair, and recovery admission. |
| C05 | Scope-aware workspace admission for greenfield, single-project, explicit root, ambiguous parent, and excluded paths. |
| C06 | Ownership decisions consumed by target admission, completion evidence, and repeated-target exclusion. |

The goal is to make artifact role, workspace scope, and ownership a shared
contract boundary across existing CommandAgent modules. This phase must reduce
drift between profile artifact classification, workspace snapshot/scope,
target admission, completion evidence, and recovery admission.

## Inputs

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_22/`
- existing modules under `src/agent/step_runner/`:
  - `profile_artifact.rs`
  - `artifact_graph.rs`
  - `workspace_scope.rs`
  - `workspace_snapshot.rs`
  - `artifact_ownership.rs`
  - `target_admission.rs`
  - `artifact_completion.rs`
  - `recovery_contract.rs`
  - `runtime/repair_loop.rs`

## Non-goals

- Do not implement artifact ledger producer closure for C07. That is Phase24.
- Do not implement completion evidence producer closure for C08. That is
  Phase24, except where Phase23 must prove ownership is consumable by existing
  completion boundaries.
- Do not implement evidence binding producer closure for C09. That is Phase24.
- Do not implement active-job arbitration or dispatch gate behavior. That is
  Phase25.
- Do not add provider/model-specific behavior.
- Do not add hidden workspace discovery, hidden repair loops, or retry
  expansion.
- Do not use broad sign-off alone as row proof.

## Design Alignment

This plan follows the current CommandAgent architecture:

```text
deterministic path / profile / workspace facts
  -> artifact role classification
  -> workspace scope admission
  -> artifact ownership decision
  -> target/completion/recovery consumers
  -> bounded repair or explicit stop
```

Layer ownership:

| layer | Phase23 responsibility |
| --- | --- |
| `profile_artifact` / `artifact_graph` | own role taxonomy and deterministic path classification. |
| `workspace_snapshot` / `workspace_scope` | own bounded workspace facts and task-claimable scope. |
| `artifact_ownership` | own whether a path is owned, candidate-only, generated, dependency/cache, read-only, verifier-only, or out-of-scope. |
| `target_admission` / `artifact_completion` | consume role/scope/ownership decisions without reclassifying paths independently. |
| `recovery_contract` / `repair_loop` | expose role/scope/ownership evidence in repair admission and safe-stop facts. |
| `eval_report` / focused cases | prove that role, scope, and ownership are visible and stable in eval output. |

## Architecture Shape

Prefer a shared classification boundary over adding new workflow behavior.

Expected implementation shape:

1. Identify the current role/scope/ownership producers.
2. Ensure consumers use those producers instead of parallel string heuristics.
3. Add typed or rendered evidence only where the deterministic producer already
   exists.
4. Reject or classify out-of-scope/generated/dependency/cache targets before
   repair target selection.
5. Prove each row with unit tests first, focused fixture second, broad
   sign-off last.

This keeps complexity bounded: Phase23 strengthens shared contract consumers;
it does not create a new workspace manager or profile-specific workflow engine.

## Horizontal Rollout

Phase23 must not be Next.js-only. Coverage should include:

- Next.js app route, manifest, generated declaration, dependency/cache, and
  build output paths.
- Rust source, `target/`, and generated/build paths.
- Python source, cache, and virtual environment style paths where existing
  classifiers support them.
- Docs and data artifacts so non-code deliverables do not get forced into
  source-repair ownership.
- Greenfield, single-project, explicit-root, ambiguous-parent, and
  excluded-path workspace scope cases.

Horizontal rollout should extend shared role/scope/ownership facts, not add
per-profile repair workflows.

## Documentation Updates

Runtime changes in Phase23 must update:

- `docs/architecture.md` if the shared role/scope/ownership boundary changes.
- `docs/evaluation.md` if eval fields, expected fields, or sign-off
  interpretation changes.
- `docs/profiles.md` if profile artifact classification expectations change.
- `docs/ultra-plan-run.md` if plan/profile contracts expose new role/scope
  requirements to the planner.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after proof
  exists for C04-C06.
- a new `docs/eval/loadmap2-phase23-artifact-scope-ownership-*.md` report at
  implementation closure.

## Required Proof

Minimum proof before a row can be `closed_proven`:

| row | minimum proof |
| --- | --- |
| C04 | Unit tests proving shared artifact role taxonomy is consumed by profile verification, verifier repair/admission, and recovery admission without divergent path heuristics. |
| C05 | Unit tests proving workspace scope admission for greenfield, single-project, explicit root, ambiguous parent, ignored dependency/cache/build output, and excluded paths. |
| C06 | Unit tests proving ownership decisions feed target admission, completion evidence eligibility, and repeated-target exclusion without treating read-only/verifier/cache/generated paths as owned implementation targets. |

Phase-level proof:

- `cargo fmt --check`
- targeted `cargo test` filters for profile artifact, artifact graph,
  workspace scope, workspace snapshot, artifact ownership, target admission,
  and artifact completion
- focused eval proof for role/scope/ownership admission and reporting
- broad sign-off rerun after behavior changes

## Exit Gate

Phase23 can close only when:

- C04, C05, and C06 are each `closed_proven`, or a narrower same-surface split
  is created with failed proof evidence, owner, downstream phase, and closure
  condition.
- `source_alignment_matrix.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, and `reconciliation.md` are updated with final
  results.
- focused proof and broad sign-off results are recorded when behavior changes.
- coverage table status changes are made only for rows with proof.
- focused proof root is recorded, either from an existing focused case that
  proves the Phase23 fields or from a new Phase23 focused fixture.

## Closure Result

Phase23 is closed as `closed_proven`.

| row | final status | proof |
| --- | --- | --- |
| C04 | Implemented | `cargo test profile_artifact`, `cargo test artifact_graph`, `cargo test target_admission`, `cargo test artifact_completion`, focused fixture root `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023` |
| C05 | Implemented | `cargo test workspace_scope`, `cargo test workspace_snapshot`, `cargo test artifact_ownership`, `cargo test target_admission`, focused fixture root `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023` |
| C06 | Implemented | `cargo test artifact_ownership`, `cargo test target_admission`, `cargo test artifact_completion`, `cargo test evidence_authority`, focused fixture root `eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023` |

Regression proof:

```text
cargo fmt --check: pass
cargo test: pass
cargo build --release: pass
python3 tests/test_eval_report.py: pass
focused assertions: passed_recheck
broad sign-off: pass
```

No Phase23 row was split forward.

## Plan Review

Review findings applied:

- Kept Phase23 scoped to common role/scope/ownership facts and their consumers,
  not ledger/completion producer closure or active-job arbitration.
- Required cross-profile coverage so role taxonomy does not become a Next.js
  special case.
- Required consumer proof for target admission, completion eligibility, and
  recovery admission so the phase cannot close with foundation types only.
- Required focused proof because role/scope/ownership fields affect recovery
  and eval interpretation. The plan allows reusing an existing focused case
  only if it proves the Phase23 fields explicitly.
- Preserved bounded behavior: no retry count increase, hidden continuation,
  provider/model-specific branch, or profile-owned workflow engine is part of
  the plan.
