# Loadmap2 Phase24 Plan

Date: 2026-06-23 JST

## Objective

Phase24 closes the Phase21 split-forward rows C07-C10:

| coverage id | responsibility |
| --- | --- |
| C07 | Artifact ledger producers for tool records, verifier mentions, setup/scaffold deltas, workspace observations, and pass-side authority facts. |
| C08 | Completion evidence producers for verifier, file layout, docs, data, report, and profile-wide completion facts. |
| C09 | Evidence binding producers for manifest identity, docs section, schema output, source citation, route/import, and test script bindings. |
| C10 | Deliverable obligation audit, freshness rules, and read-only freshness checks projected into plan/profile/eval evidence. |

The goal is to turn the existing ledger/evidence/binding/obligation types into
producer-complete, eval-visible contract data. Phase24 should make completion
authority depend on observed producer facts rather than broad terminal-state
defaults or prose-only success claims.

## Inputs

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_22/`
- `workspace/mvp/logic/anvil/loadmap2/phase_23/`
- existing modules under `src/agent/step_runner/`:
  - `artifact_ledger.rs`
  - `completion_evidence.rs`
  - `evidence_producer.rs`
  - `evidence_authority.rs`
  - `evidence_binding.rs`
  - `deliverable_obligation.rs`
  - `task_contract.rs`
  - `artifact_completion.rs`
  - `recovery_contract.rs`
  - `runtime/repair_loop.rs`
- eval/report modules:
  - `scripts/eval_agent_slice.sh`
  - `scripts/eval_report.py`
  - `scripts/eval_failure_observation.py`
  - `tests/test_eval_report.py`
  - focused cases under `eval/cases/focused/control-recovery/`

## Non-goals

- Do not implement active-job arbitration or dispatch lifecycle. That is
  Phase25.
- Do not implement setup/profile semantic repair, repair brief, or repair
  action envelope expansion. That is Phase26.
- Do not implement broader target prioritization, verifier repair lifecycle,
  patch validation, or no-progress transitions. That is Phase27.
- Do not add hidden evidence runners, hidden repair loops, or retry expansion.
- Do not make profiles workflow engines. Profile facts may produce obligations
  and evidence hints only.
- Do not add provider/model-specific evidence policy.
- Do not mark a row `Implemented` from docs, CI, or broad sign-off alone.

## Design Alignment

Phase24 follows the current CommandAgent contract stack:

```text
task/profile/plan facts
  -> deliverable obligations
  -> artifact ledger observations
  -> completion evidence producers
  -> evidence binding producers
  -> completion authority / recovery evidence / eval report
  -> bounded repair or explicit stop
```

Layer ownership:

| layer | Phase24 responsibility |
| --- | --- |
| `artifact_ledger` | Own observed artifact facts from graph, tool records, verifier mentions, workspace observations, setup deltas, and scaffold deltas. |
| `completion_evidence` / `evidence_producer` | Own conversion from already-observed facts into typed completion evidence. |
| `evidence_binding` | Own deterministic binding facts between deliverables and proof paths. |
| `deliverable_obligation` / `task_contract` | Own required deliverable kind, evidence requirement, and freshness requirement projection. |
| `evidence_authority` | Own terminal completion classification from ledger, evidence, binding, and freshness facts. |
| `eval_report` / focused cases | Prove that producer facts are visible and stable in eval output. |

This phase strengthens producer closure. It does not select repair jobs, choose
new targets, run setup commands implicitly, or reinterpret provider behavior.

## Architecture Shape

Prefer one common producer boundary over per-profile ad hoc checks.

Expected implementation shape:

1. Inventory current C07-C10 producers and report fields.
2. Add or complete deterministic producer functions for missing observed facts.
3. Ensure completion authority consumes producer output instead of fallback
   terminal-state defaults where concrete facts are available.
4. Add focused fixtures that assert producer-specific fields, not only final
   success/failure.
5. Update the coverage table only when each row has unit proof, focused proof,
   and regression sign-off.

This keeps behavior stable: producer facts are bounded, visible, and
provider-independent. They do not create a new runtime loop.

## Horizontal Rollout

Phase24 must cover common artifact families, not only Next.js:

- Next.js setup manifest, route/import binding, build verifier, selected route,
  and generated/cache exclusion.
- Rust `Cargo.toml`, source/test deliverables, `cargo test` verifier evidence,
  and `target/` output exclusion.
- Python import binding, test artifact binding, source/report deliverables, and
  cache/venv exclusion.
- Docs deliverables with required section binding and freshness checks.
- Data deliverables with schema/column binding and derived-output completion
  evidence.
- Report artifacts with completeness evidence and freshness status.

Horizontal rollout should extend shared producers and focused fixtures. It
should not introduce profile-specific workflow dispatch.

## Documentation Updates

Runtime changes in Phase24 must update:

- `docs/architecture.md` if ledger/evidence/binding authority boundaries
  change.
- `docs/evaluation.md` if eval fields, expected fields, or sign-off
  interpretation changes.
- `docs/profiles.md` if profile facts produce new evidence or binding hints.
- `docs/ultra-plan-run.md` if planner-facing deliverable/evidence obligations
  change.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after proof
  exists for C07-C10.
- a new `docs/eval/loadmap2-phase24-ledger-evidence-binding-*.md` report at
  implementation closure.

## Required Proof

Minimum proof before a row can be `closed_proven`:

| row | minimum proof |
| --- | --- |
| C07 | Unit tests proving ledger entries are produced for graph/tool/read/write/edit, verifier mention, setup delta, scaffold delta, workspace observation, and pass-side authority inputs. |
| C08 | Unit tests proving completion evidence producers cover verifier pass/fail, file layout, docs section, structured data/schema, report completeness, and profile-wide evidence without running hidden tools. |
| C09 | Unit tests proving evidence binding producers cover manifest identity, docs section, schema column, source citation, route/import, test script, executable handle, and file layout bindings. |
| C10 | Unit tests proving deliverable obligations and freshness rules are projected into task/profile/eval evidence, including read-only stale evidence checks. |

Phase-level proof:

- `cargo fmt --check`
- targeted `cargo test` filters for artifact ledger, completion evidence,
  evidence producer, evidence authority, evidence binding, deliverable
  obligation, task contract, plan lint, and artifact completion
- `python3 tests/test_eval_report.py`
- focused eval proof for ledger/evidence/binding/obligation producer fields
- broad sign-off rerun after behavior changes

## Exit Gate

Phase24 can close only when:

- C07, C08, C09, and C10 are each `closed_proven`, or a narrower same-surface
  split is created with failed proof evidence, owner, downstream phase, and
  closure condition.
- `source_alignment_matrix.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, and `reconciliation.md` are updated with final
  results.
- focused proof and broad sign-off results are recorded when behavior changes.
- coverage table status changes are made only for rows with proof.
- focused proof root is recorded from fixtures that explicitly prove C07-C10
  producer fields.

## Implementation Result

Phase24 is complete.

- C07-C10 are `closed_proven` in `row_closure_matrix.md`.
- All Phase24 blockers are `closed_proven` in `blocking_ledger.md`.
- Focused proof root:
  `eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617`
- Focused assertions: `passed: 6`
- Recheck assertions: `passed_recheck: 6`
- Broad sign-off: `status: pass`
- Coverage table updated C07-C10 from `Partial` to `Implemented`.
- No hidden evidence runner, hidden repair loop, retry expansion,
  provider/model branch, or profile workflow engine was added.

## Plan Review

Review findings applied:

- Kept Phase24 scoped to producer closure and completion authority, not
  active-job arbitration or repair lifecycle behavior.
- Required row-specific producer proof so existing type definitions cannot be
  mistaken for implemented parity.
- Required cross-profile coverage for source, setup, test, docs, data, report,
  route/import, and verifier evidence.
- Required focused assertions for producer-visible fields because broad
  sign-off can pass while ledger/evidence/binding fields remain defaulted.
- Preserved bounded behavior: no retry count increase, hidden continuation,
  provider/model branch, profile workflow engine, or implicit dependency setup
  is part of this plan.
