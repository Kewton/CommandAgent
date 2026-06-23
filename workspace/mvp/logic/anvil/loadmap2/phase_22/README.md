# Loadmap2 Phase22 Plan

Date: 2026-06-23 JST

## Objective

Phase22 closes the Phase21 split-forward rows C01-C03:

| coverage id | responsibility |
| --- | --- |
| C01 | Task contract core evidence, lifecycle, constraints, and persistence boundary. |
| C02 | Deterministic task-kind/request signal admission for ambiguous task/profile input. |
| C03 | Behavior-delta obligations projected into lint/evidence/completion checks. |

The goal is not to add another planning layer. The goal is to make existing
CommandAgent planning and recovery contracts strong enough that task intent,
admission status, constraints, lifecycle state, and behavior obligations are
observable, deterministic, and enforced by the responsible layers.

## Inputs

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`
- `docs/eval/loadmap2-phase1-task-contract-20260622.md`
- `docs/eval/loadmap2-phase21-core-contract-ownership-20260623.md`

## Non-goals

- Do not introduce a second execution engine or hidden workflow controller.
- Do not add provider/model-specific planning behavior.
- Do not make profiles into workflow engines.
- Do not close C04-C12. Those remain Phase23-Phase25 scope.
- Do not use broad sign-off alone as row proof.
- Do not use `split_forward` for known Phase22 work simply because
  implementation is incomplete.

## Design Alignment

This plan follows the CommandAgent boundary:

```text
deterministic request/plan facts
  -> TaskContract
  -> plan lint / prompt / eval report / recovery evidence
  -> bounded correction or explicit stop
```

Layer ownership:

| layer | Phase22 responsibility |
| --- | --- |
| `task_contract` | own task kind, admission status, constraints, lifecycle state, expected completion evidence, and behavior obligation projection data. |
| `plan_input` / `plan_yaml` | parse and preserve public plan fields needed for task contract construction. |
| `plan_lint` | enforce missing task contract projection and behavior obligation owners. |
| `profiles` | provide domain obligations, but not recovery workflow decisions. |
| `eval_report` / focused cases | prove that task contract fields are visible and stable under focused fixtures. |

## Architecture Shape

Prefer extending the existing `TaskContract` data boundary rather than adding
a new arbiter.

Expected implementation shape:

1. Add typed fields only where deterministic inputs already exist.
2. Render those fields through existing prompt/evidence/eval paths.
3. Enforce only deterministic omissions in `plan_lint`.
4. Keep recovery action selection in the existing recovery orchestration.
5. Prove with unit tests first, focused eval second, broad sign-off last.

This keeps complexity bounded: Phase22 adds contract data and guards, not a
new runtime loop.

## Horizontal Rollout

Phase22 must not be Next.js-only. Coverage must include at least:

- generic new-file task;
- Next.js app task with manifest/route/build/dev-port obligations;
- docs literal task;
- data/schema-style task if existing profile fixtures support it.

The rollout should use common `TaskContract` and `BehaviorObligation` fields
so later Phase23-Phase25 work can consume the same contract data.

## Documentation Updates

Runtime changes in Phase22 must update:

- `docs/architecture.md` if task contract fields or ownership boundaries
  change.
- `docs/ultra-plan-run.md` if public plan input expectations change.
- `docs/evaluation.md` if focused eval fields or sign-off interpretation
  change.
- `docs/eval/legacy-control-stack-coverage-20260621.md` only after proof
  exists for C01-C03.
- a new `docs/eval/loadmap2-phase22-task-contract-admission-*.md` report at
  implementation closure.

## Required Proof

Minimum proof before a row can be `closed_proven`:

| row | minimum proof |
| --- | --- |
| C01 | Unit tests for lifecycle, constraints, expected completion evidence, and cross-command/session persistence or an explicit bounded persistence decision. |
| C02 | Unit tests for deterministic request signals and plan admission status, plus focused task-admission fixture. |
| C03 | Unit tests for behavior-delta obligation projection into lint/evidence/completion checks, plus focused behavior-obligation fixture. |

Phase-level proof:

- `cargo fmt --check`
- targeted `cargo test` filters for task contract, plan lint, and eval report
- focused eval for task contract admission and behavior obligation projection
- broad sign-off rerun after behavior changes

## Exit Gate

Phase22 can close only when:

- C01, C02, and C03 are each `closed_proven`, or a narrower same-surface split
  is created with failed proof evidence, owner, downstream phase, and closure
  condition.
- `source_alignment_matrix.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, and `reconciliation.md` are updated with final
  results.
- focused proof and broad sign-off results are recorded.
- coverage table status changes are made only for rows with proof.

## Plan Review

Review findings applied:

- Added `source_alignment_matrix.md` as a first-class Phase22 input to avoid
  reinterpreting Anvil source files from summary prose.
- Kept Phase22 scoped to contract data and deterministic guards, not active
  job arbitration or recovery dispatch.
- Required focused proof because C02/C03 are model-facing planning behavior.
- Added horizontal rollout requirements so task contract improvements do not
  become Next.js-only.
- Preserved bounded behavior: no retry count increase, hidden repair loop, or
  provider-specific branch is part of the plan.

## Implementation Result

Phase22 is complete. C01-C03 are `closed_proven` and the coverage table now
marks them `Implemented`.

Key proof:

- `cargo fmt --check`: passed
- `cargo test task_contract`: passed
- `cargo test plan_lint`: passed
- `python3 tests/test_eval_report.py`: passed
- `python3 tests/test_eval_signoff.py`: passed
- `cargo test`: passed
- `cargo build --release`: passed
- focused fixture root:
  `eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658`
- broad sign-off: `status: pass`

No hidden retry, hidden repair loop, provider/model-specific branch, or
profile-owned workflow engine was added.
