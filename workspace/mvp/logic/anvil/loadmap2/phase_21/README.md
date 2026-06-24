# Phase 21: Core Contract And Ownership Closure

Date: 2026-06-23 JST

## Purpose

Phase21 is the first continuation phase after the Phase20
`migration_not_complete` decision.

Phase20 proved that broad sign-off can pass while the coverage table still has
unresolved accepted responsibilities. Phase21 therefore does not start another
wide migration pass. It selects one distinct responsibility class from the
Phase20 continuation ledger and closes it with row-level proof.

Selected blocker group:

```text
P20-COV-001: core task/contract/ownership surface
```

Selected coverage rows:

| coverage id | source mechanism | owner surface |
| --- | --- | --- |
| C01 | Task contract core | task contract / step runner |
| C02 | Task contract inference and admission | plan input / profile intent |
| C03 | Objective and behavior contract projection | task contract / obligations |
| C04 | Artifact role taxonomy | artifact graph / profile artifacts |
| C05 | Task workspace scope | workspace snapshot / scope admission |
| C06 | Artifact ownership | artifact graph / target admission |
| C07 | Artifact ledger | minimal loop records / repair evidence |
| C08 | Completion evidence | verifier / completion authority |
| C09 | Evidence binding | verifier/profile/setup binding |
| C10 | Deliverable obligation audit | plan lint / profile / eval |
| C11 | Active job arbiter | recovery orchestration |
| C12 | Recovery owner / dispatch gate | recovery orchestration |

## Why This Phase Exists

The recovery plan allows a Phase21 only when a new distinct responsibility
class cannot fit into Phase18 or Phase19 without ambiguity. P20-COV-001 meets
that condition because it is not a focused assertion repair, not a large-row
ownership repair, and not a final decision report. It is the foundational
contract surface that later recovery behavior depends on.

Phase21 must prove the contract foundation before later phases try to close
repair-target, verifier, setup, profile, or tool-policy gaps.

## Phase21 Exit Gate

Phase21 is complete only when all selected rows C01-C12 have one of these
states:

- `closed_proven`: implementation and proof command pass;
- `excluded_with_rationale`: intentionally not part of CommandAgent's design;
- `split_forward`: row is too broad and is split into narrower ledger rows
  with owner, proof command, and downstream phase.

For Phase21 success:

- no selected row may remain vague `Partial`;
- no selected row may lack an owner layer;
- no selected row may lack proof criteria;
- every implementation claim must be backed by tests or focused eval;
- final broad sign-off must still pass after changes;
- coverage table updates must name proof reports.

If any C01-C12 row cannot be closed, excluded, or split with proof gates,
Phase21 remains incomplete.

## Non-goals

- Do not close C13-C54 in this phase.
- Do not reopen Phase18/Phase19 findings unless a new failure maps back to
  their ledger rows.
- Do not change provider/model transport behavior.
- Do not add hidden retry or hidden repair loops.
- Do not convert coverage rows to `Implemented` from prose alone.
- Do not add an independent legacy-style workflow engine.
- Do not weaken broad sign-off, coverage, or branding gates.

## Design Alignment

Phase21 follows the repository design principles:

- deterministic evidence over semantic guessing;
- explicit failure reports over hidden continuation;
- planning, execution, verification, and repair remain separate contracts;
- common contracts before profile-specific fixes;
- profiles provide facts, not workflow engines;
- eval scripts and docs are product code;
- CI success is necessary but not sufficient for migration parity.

The accepted architecture shape is:

```text
TaskContract facts
  -> ArtifactGraph / WorkspaceScope / Ownership
  -> EvidenceBinding / CompletionEvidence
  -> ActiveJob candidate
  -> Dispatch decision or explicit stop
  -> proof report
```

Each arrow must be observable in structured fields, tests, or eval reports.

Phase21 must also preserve the recovery-plan reconciliation chain:

```text
Phase20 continuation row
  -> Phase21 blocking ledger row
  -> coverage responsibility C01-C12
  -> implementation task
  -> proof command
  -> final sign-off rerun
```

If any arrow is missing, implementation should not start for that row.

## Architecture Guidance

Phase21 should improve architecture by adding or tightening shared contracts,
not by adding case-specific prompt text.

Preferred implementation shape:

| Need | Preferred boundary |
| --- | --- |
| Task kind / intent facts | `TaskContract` or plan/profile input facts |
| Required behavior | behavior obligation projection |
| Artifact role | common artifact classifier with profile adapters |
| Workspace claim | workspace snapshot/scope admission |
| Ownership | artifact ownership decision and target admission |
| Tool/edit observation | artifact ledger |
| Pass/fail authority | completion evidence and evidence binding |
| Current job selection | active job candidate and dispatch gate |

If a row requires a new data shape, add it as a typed contract/evidence field
and project it into eval reports. Do not bury it only in a repair prompt.

## Horizontal Rollout

Phase21's contract foundation should be usable by later phases:

- C13-C20 recovery task / setup / semantic repair should consume the same
  owner/action/fact fields.
- C21-C32 target/verifier/patch work should consume the same ownership and
  completion evidence.
- C34-C44 profile/tool/runtime support should consume the same profile-neutral
  facts before adding profile-specific adapters.
- P20-LEDGER-001 should remain provider/eval-bound and should not be hidden in
  this phase.

## Required Outputs

Phase21 should produce:

- `workspace/mvp/logic/anvil/loadmap2/phase_21/row_closure_matrix.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_21/blocking_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_21/reconciliation.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_21/implementation_report.md`
- `docs/eval/loadmap2-phase21-core-contract-ownership-<date>.md`
- updates to `docs/eval/legacy-control-stack-coverage-20260621.md` only for
  rows that have proof;
- tests and focused eval fixtures for implemented behavior;
- final broad sign-off result.

## Verification Strategy

Minimum local verification after implementation:

```bash
cargo fmt --check
cargo test
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
bash scripts/eval_smoke.sh
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
scripts/check_branding.sh
```

Add focused eval only where a row's proof needs model-facing behavior. Unit
tests are sufficient for deterministic projection or parser behavior, but not
for claims about end-to-end recovery behavior.

## Documentation Updates

Update documentation when Phase21 changes behavior:

- `docs/architecture.md` for new shared contract boundaries.
- `docs/philosophy.md` only if the contract/recovery philosophy changes.
- `docs/evaluation.md` for new proof or report expectations.
- `docs/profiles.md` if profile-neutral facts or role taxonomy change.
- `docs/eval/legacy-control-stack-coverage-20260621.md` for row status
  updates with proof references.
- `docs/eval/loadmap2-phase21-core-contract-ownership-<date>.md` for the final
  phase report.

## Review Result Reflected

The initial risk was to define Phase21 as another broad "continue migration"
phase. The reviewed plan narrows Phase21 to P20-COV-001 and requires C01-C12
row-level closure. It also allows `split_forward` only when the split includes
owner, proof command, downstream phase, and coverage mapping, so unresolved
work cannot disappear into generic follow-up text. The review also added
explicit Phase21 ledger and reconciliation outputs so the phase satisfies the
recovery-plan admission rule before implementation starts.

## Execution Result

Phase21 was executed as an admission/reconciliation phase.

Created outputs:

- `row_closure_matrix.md`
- `blocking_ledger.md`
- `reconciliation.md`
- `implementation_report.md`
- `docs/eval/loadmap2-phase21-core-contract-ownership-20260623.md`

Final row disposition:

| disposition | count |
| --- | ---: |
| `closed_proven` | 0 |
| `excluded_with_rationale` | 0 |
| `split_forward` | 12 |
| `open` | 0 |

Phase21 intentionally did not mark C01-C12 as `Implemented`. The phase closes
the vague grouped blocker by assigning row-level downstream work with owner,
proof command, downstream phase, and closure condition.
