# Phase 17 Implementation Tasks

## 1. Read Evidence

- [ ] Read `docs/eval/loadmap2-phase16-broad-signoff-20260622.md`.
- [ ] Run or inspect `scripts/eval_signoff.py` output for the Phase 16 roots.
- [ ] Inspect the failing rows in:
  - focused local LLM `summary.tsv`
  - focused local LLM `recheck_summary.tsv`
  - large time-boxed `summary.tsv`
  - large time-boxed `recheck_summary.tsv`

## 2. Create Blocking Ledger

- [ ] Add `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`.
- [ ] Add one row per sign-off finding or tightly related finding group.
- [ ] Include:
  - owning layer;
  - failed contract;
  - expected behavior;
  - proof command;
  - responsible phase;
  - closure condition.

## 3. Create Sign-off Reconciliation

- [ ] Add
  `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`.
- [ ] Add every current `scripts/eval_signoff.py` finding as a reconciliation
  row.
- [ ] Map each finding to exactly one ledger row, unless the finding is split
  into narrower ledger rows.
- [ ] Record the coverage-table responsibility for each finding.
- [ ] Record the downstream phase and proof command.
- [ ] Mark rows as `mapped`, `split_required`, `coverage_gap`,
  `stale_finding`, or `accepted_external_after_ownership`.

## 4. Validate Ledger Completeness

- [ ] Confirm every `scripts/eval_signoff.py` finding appears in the ledger.
- [ ] Confirm every `scripts/eval_signoff.py` finding appears in the
  reconciliation table.
- [ ] Confirm no ledger row has multiple owners unless it is split.
- [ ] Confirm every row is assigned to Phase18, Phase19, or Phase20.
- [ ] Confirm every `split_required` row is split before Phase17 closes.
- [ ] Confirm every `coverage_gap` row updates or references the coverage table
  before Phase17 closes.
- [ ] Confirm no row is closed in Phase17 unless it is a stale expectation or
  report-only interpretation issue proven by a focused command.

## 5. Update Roadmap If Needed

- [ ] Keep Phase17 as recovery rebaseline.
- [ ] Keep Phase18 as focused sign-off recovery.
- [ ] Keep Phase19 as large ownership/evidence recovery.
- [ ] Keep Phase20 as final closure only.
- [ ] Update any stale text that still implies Phase17 can immediately declare
  migration complete.

## 6. Apply Horizontal Rollout Accounting

- [ ] Confirm focused rows use the same reconciliation columns as large rows.
- [ ] Confirm Next.js, Python/FastAPI, and Rust large rows use the same
  ownership/evidence vocabulary.
- [ ] Confirm profile-specific details appear only in coverage responsibility
  or proof command fields.
- [ ] Confirm smoke and fixture roots remain part of the sign-off command even
  if they do not currently emit blockers.

## 7. Update Documentation

- [ ] Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md` if exit-gate
  failure handling changes.
- [ ] Update `workspace/mvp/logic/anvil/loadmap2/README.md` if Phase17-20
  ownership changes.
- [ ] Add a `docs/eval/` note only if Phase17 changes interpretation of
  Phase16 evidence.
- [ ] Do not update architecture or ADR docs unless reconciliation proves the
  current architecture cannot represent a blocker.

## 8. Update Coverage Table If Needed

- [ ] If a blocker maps to a coverage responsibility that remains `Partial`,
  add or update the stage/proof note.
- [ ] If a blocker is not represented in the coverage table, add a split row or
  note under the closest responsibility.
- [ ] Do not proceed to runtime work while a blocker has no coverage-table
  responsibility or documented intentional exclusion.

## 9. Derive Phase18 And Phase19 Worklists

- [ ] Create a finite Phase18 worklist from reconciliation rows assigned to
  Phase18.
- [ ] Create a finite Phase19 worklist from reconciliation rows assigned to
  Phase19.
- [ ] Ensure every worklist item has one proof command and one closure
  condition.
- [ ] Ensure no item is described only as "rerun eval".

## 10. Review The Plan

- [ ] Verify the plan does not introduce runtime behavior.
- [ ] Verify the plan does not add hidden retries or broad orchestration.
- [ ] Verify all blockers have deterministic proof gates.
- [ ] Verify Phase20 remains the only place where migration complete can be
  declared.
- [ ] Reflect any review finding in README, implementation tasks, and concrete
  work plan before Phase17 execution starts.

## 11. Verification

- [ ] `python3 scripts/eval_signoff.py --require-recheck ...` still reproduces
  the Phase 16 blockers.
- [ ] Ledger row count and sign-off finding count reconcile.
- [ ] Reconciliation finding count equals the current sign-off finding count.
- [ ] Every reconciliation row has a non-empty ledger row, coverage
  responsibility, downstream phase, and proof command.
- [ ] Documentation links point to existing files.

## 12. If Verification Fails

- [ ] If sign-off output has an unmapped finding, add a reconciliation row and
  either map it to an existing ledger row or create a new ledger row.
- [ ] If a ledger row has multiple owners, split it before Phase17 closes.
- [ ] If a row has no coverage responsibility, update
  `docs/eval/legacy-control-stack-coverage-20260621.md` or record an explicit
  exclusion.
- [ ] If a row has no proof command, add a focused case, report fixture, or
  broad rerun command before assigning it to Phase18/19.
- [ ] If the same row fails twice after targeted fixes in later phases, attach
  a design review note before another implementation attempt.

## Completion Criteria

Phase 17 is complete only when:

- the blocking ledger covers every Phase 16 sign-off finding;
- the reconciliation table covers every Phase 16 sign-off finding;
- every blocker has owner, phase, proof command, and closure condition;
- every blocker maps to a coverage responsibility or documented coverage gap;
- Phase18/19 scopes are finite and directly traceable to ledger rows;
- horizontal rollout accounting is consistent across focused, smoke, fixture,
  large Next.js, large Python/FastAPI, and large Rust families;
- required documentation updates are complete;
- no migration-complete declaration is made.

## Review Result Reflected

- Added horizontal rollout accounting so Phase17 does not solve only the
  currently visible Next.js/focused rows.
- Added documentation update boundaries so docs are updated when contracts
  change but architecture docs are not churned unnecessarily.
- Added an explicit plan-review task before execution.
- Kept runtime work out of Phase17 to avoid instability and hidden
  orchestration.
