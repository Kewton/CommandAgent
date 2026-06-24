# Phase 17 Plan: Recovery Rebaseline And Blocking Ledger

## Objective

Phase 17 converts the Phase 16 broad sign-off failure into an owned blocking
ledger. It fixes the process gap before more runtime migration work continues.

The goal is not to repair all runtime behavior in Phase 17. The goal is to make
the remaining work finite, assigned, and proof-backed so later phases cannot be
closed while migration blockers remain unresolved.

## Problem Statement

Phase 16 produced useful evidence but did not pass broad sign-off. The process
mistake was treating the sign-off checker implementation as complete even
though the sign-off result was red.

Phase 17 changes the operating rule:

```text
implementation done
  != phase complete

phase complete
  = assigned blockers pass proof gates
```

## Inputs

- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `docs/eval/loadmap2-phase16-broad-signoff-20260622.md`
- `scripts/eval_signoff.py`
- Phase 16 eval roots:
  - `eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659`
  - `eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759`
  - `eval/runs/loadmap2-phase16-focused-local-llm/20260622T173940`
  - `eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149`

## Deliverables

- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`
- Updates to `docs/eval/legacy-control-stack-coverage-20260621.md` if the
  current coverage table does not identify the owner or proof gate for a
  blocker.
- A short eval note under `docs/eval/` if Phase 17 changes interpretation of
  Phase 16 findings.

## Horizontal Rollout

Phase 17 is not profile-specific. The reconciliation contract applies across
all current sign-off families:

- focused control-recovery;
- focused deterministic fixtures;
- smoke local LLM;
- large Next.js;
- large Python/FastAPI;
- large Rust.

Horizontal rollout means every family uses the same accounting model:

```text
sign-off finding
  -> ledger row
  -> coverage responsibility
  -> downstream phase
  -> proof command
```

Profile-specific behavior can appear only as a coverage responsibility or proof
command detail. It must not create a separate completion process.

## Documentation Updates

Phase 17 must update documentation only where the process contract changes:

- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
  - failure handling when exit gates are not met;
  - migration-complete decision states.
- `workspace/mvp/logic/anvil/loadmap2/README.md`
  - Phase17-20 ownership and exit gates.
- `docs/eval/legacy-control-stack-coverage-20260621.md`
  - only if reconciliation exposes a missing or stale coverage
    responsibility.
- `docs/eval/`
  - only if Phase17 changes interpretation of Phase16 evidence.

Do not update architecture or ADR docs in Phase 17 unless the reconciliation
shows that the current documented architecture cannot represent a blocker.

## Architecture And Extensibility

The architectural improvement in Phase 17 is a stable accounting boundary, not
a runtime mechanism.

The model is intentionally append-only:

- new sign-off findings can be appended to reconciliation;
- new blocker rows can be added or split;
- downstream phases can be reassigned with rationale;
- proof commands can change only when the closure condition changes.

This keeps future Phase18/19/20 changes auditable without adding hidden
orchestration to the minimal loop.

## Blocking Ledger Schema

Each blocker row must contain:

- `id`
- `source_root` or source family when multiple roots share the same blocker
- `case_id`
- `observed_failure`
- `expected_behavior`
- `owning_layer`
- `failed_contract`
- `suspected_module`
- `responsible_phase`
- `proof_command`
- `closure_condition`
- `status`

Allowed statuses:

- `open`
- `in_progress`
- `blocked_external`
- `closed_proven`
- `deferred_with_rationale`

`closed_proven` requires a passing proof command or an explicitly recorded
accepted limitation.

## Reconciliation Requirement

The blocking ledger is not sufficient by itself. Phase 17 must also reconcile
the current sign-off output against the ledger.

Required reconciliation columns:

- `finding_id`
- `family`
- `case_id`
- `signoff_code`
- `ledger_row`
- `coverage_responsibility`
- `downstream_phase`
- `proof_command`
- `reconciliation_status`

Allowed reconciliation statuses:

- `mapped`
- `split_required`
- `coverage_gap`
- `stale_finding`
- `accepted_external_after_ownership`

No Phase18 or Phase19 implementation should start while any finding is
`split_required` or `coverage_gap`.

## Revised Phase Ownership

Phase 17 owns process recovery:

- build the ledger;
- assign every Phase 16 blocker;
- ensure Phase18/19 have finite scopes;
- prevent Phase20 declaration until broad sign-off passes.

Phase 18 owns focused sign-off recovery.

Phase 19 owns large ownership/evidence recovery.

Phase 20 owns final coverage closure and migration-complete declaration.

## Acceptance Criteria

- Every Phase 16 sign-off finding is present in the blocking ledger.
- Every Phase 16 sign-off finding is present in
  `signoff_reconciliation.md`.
- Every ledger row has one owning layer and one responsible phase.
- Every ledger row has a concrete proof command or accepted-limitation
  criterion.
- Every reconciliation row maps to a coverage responsibility or explicitly
  records a coverage-table gap to fix before runtime work.
- No row is closed by CI-only evidence.
- No row is closed by documentation-only evidence.
- Phase18 and Phase19 scopes can be derived from the ledger without rereading
  raw eval logs.

## If Acceptance Is Not Met

Phase17 remains open. Apply the recovery plan failure table:

- missing sign-off mapping: add or split ledger rows;
- missing coverage responsibility: update coverage table or record explicit
  exclusion;
- ambiguous owner: split the row or define deterministic owner arbitration;
- missing proof command: add a focused/report fixture or broad rerun command;
- new sign-off finding: add it to reconciliation before runtime work starts.

Do not proceed to Phase18 or Phase19 while Phase17 has `split_required` or
`coverage_gap` reconciliation rows.

## Design Alignment

- This phase does not change runtime behavior.
- It does not add hidden retries or broader orchestration.
- It tightens the process contract around existing eval evidence.
- It keeps CI, focused proof, broad proof, and migration declaration separate.

## Stability And Complexity Controls

- Keep Phase17 docs/eval-only unless reconciliation exposes a coverage-table
  schema gap.
- Do not add new runtime branches in Phase17.
- Do not broaden `scripts/eval_signoff.py` to execute cases.
- Do not create profile-specific sign-off logic.
- Prefer splitting an ambiguous row over adding a broad catch-all owner.

## Review Result Reflected

Review concern:

```text
The first recovery plan still relied too much on manual summary. It did not
prove that every sign-off finding was accounted for.
```

Reflected changes:

- added `signoff_reconciliation.md`;
- required a one-row accounting entry for every current sign-off finding;
- required mapping to coverage responsibility, downstream phase, and proof
  command;
- made `split_required` and `coverage_gap` blockers for Phase18/19;
- kept Phase20 as the only migration-complete declaration phase.
