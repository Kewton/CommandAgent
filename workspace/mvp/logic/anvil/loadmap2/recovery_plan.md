# Loadmap2 Recovery Plan After Phase 16

## Purpose

This plan corrects the process failure exposed by Phase 16.

The issue is not only that some Anvil responsibilities are still incomplete.
The deeper issue is that phase completion was treated as implementation
completion, while migration completion requires eval-proven behavior.

From this point forward:

```text
phase complete
  = implementation complete
  + assigned blockers pass their proof gate
  + remaining blockers are reassigned to a later phase with explicit rationale
```

CI success is required but never sufficient for migration completion.

## What Went Wrong

### 1. Residual work was visible but not gate-enforced

The roadmap already defined stage 5 as eval-proven behavior and stated that
`Partial` / `Missing` cannot remain final states. However, the phase workflow
did not consistently prevent moving forward when assigned rows were still below
stage 5.

### 2. Phase 16 was treated as a sign-off checker implementation

Phase 16 added the broad sign-off checker and ran the required roots. The
checker correctly returned `fail`, but that fail result was recorded as a
follow-up rather than making Phase 16 incomplete.

Correct handling should have been:

```text
sign-off fail
  -> phase remains blocked
  -> every failed sign-off row becomes a blocking ledger item
  -> targeted phases clear the blockers
  -> sign-off is rerun
```

### 3. CI and migration proof were mixed in reporting

CI proves build/test health. It does not prove Anvil migration parity.

```text
CI pass != broad sign-off pass
CI pass != migration complete
```

### 4. Component migration outpaced E2E responsibility closure

Phases 1-16 added many components: failure observation, active jobs, target
admission, repair actions, evidence fields, and sign-off reporting. The
remaining gap is the end-to-end connection from failure to owner/action/target
to evidence under broad real model runs.

## Recovery Rules

1. The recovery ledger is the source of truth for Phase 17 onward.
2. Every blocker must have one owning layer.
3. Every blocker must have a proof command or focused eval case.
4. A blocker cannot be closed by documentation alone.
5. A blocker cannot be closed by CI alone.
6. A broad eval failure can be accepted only when it is owned, actionable, and
   explicitly classified as an environment, provider, or model-quality limit.
7. If the sign-off checker reports `fail`, the related phase is blocked unless
   every finding is mapped to a later phase with rationale.
8. A phase cannot close from a summary paragraph. It must close from a
   reconciliation table that maps:
   - sign-off finding;
   - blocking ledger row;
   - coverage-table responsibility;
   - downstream phase task;
   - proof command.
9. Grouping multiple sign-off findings into one blocker is allowed only when
   the same root cause, owner layer, proof command, and closure condition apply.
   Otherwise the row must be split.
10. A blocker marked as model-quality, provider, or environment limitation must
    still have owner/action/evidence. Missing ownership or missing evidence is
    never a model-quality limitation.

## Required Reconciliation Chain

Phase 17 must make this chain explicit:

```text
scripts/eval_signoff.py finding
  -> phase_17/blocking_ledger.md row
  -> docs/eval/legacy-control-stack-coverage-20260621.md responsibility
  -> Phase18 or Phase19 implementation task
  -> proof command
  -> final sign-off rerun
```

If any arrow is missing, the work is not ready for implementation. This is the
main process correction from the Phase 16 failure.

The reconciliation is recorded in:

```text
workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md
```

## Phase 16 Blocking Ledger Seed

These rows seed the Phase 17 recovery ledger.

| Source | Blocker | Initial owner layer | Recovery phase |
| --- | --- | --- | --- |
| focused | `focused-docs-literal-mismatch` expected source repair, observed explicit stop / step policy failure | recovery task / step policy | Phase 18 |
| focused | `focused-nextjs-dependency-setup` expected completed setup, observed explicit stop | setup / recovery task | Phase 18 |
| focused | `focused-nextjs-endpoint-smoke` expected ok, observed plan lint failure and raw `rc:1` | planning / eval observation | Phase 18 |
| focused | `focused-nextjs-route-integration` expected ok, observed manifest repair / plan lint failure | profile / planning / recovery task | Phase 18 |
| large | five large rows timed out as `provider_transport:eval_timeout` | provider transport / eval boundary | Phase 19 |
| large | `large-nextjs-app-modify` profile dependency conflict mapped to source repair in the recorded root | profile / active job arbitration | Phase 19 |
| large | failed large rows missing evidence binding / completion evidence | completion evidence / eval report | Phase 19 |
| large | failed large rows missing target where applicable | target admission / eval report | Phase 19 |

Phase 17 must expand this seed into a concrete ledger with exact fields,
expected behavior, proof command, and closure status.

## Revised Phase Flow

```text
Phase 17: Recovery rebaseline and blocking ledger
Phase 18: Focused sign-off recovery
Phase 19: Large ownership and evidence recovery
Phase 20: Final coverage closure and migration-complete declaration
```

Phase 20 is the only phase allowed to declare migration complete.

## Phase Exit Gates

| Phase | Exit gate |
| --- | --- |
| Phase 17 | Every current sign-off finding is mapped to a ledger row, coverage responsibility, downstream phase, and proof command. |
| Phase 18 | All focused ledger rows assigned to Phase18 are `closed_proven`; focused sign-off has no failed expected assertions or raw diagnostics. |
| Phase 19 | All large ledger rows assigned to Phase19 are `closed_proven` or `blocked_external` with owner/action/evidence; broad sign-off has no unowned large failure. |
| Phase 20 | Coverage table has no adopted `Partial` or `Missing`; final broad sign-off exits zero; final report declares migration complete. |

`blocked_external` is not allowed for missing owner/action/evidence. It is only
allowed for a provider, model-throughput, network, or environment constraint
after ownership and evidence are already present.

## If A Phase Does Not Meet Its Exit Gate

Do not close the phase. Do not move the failure to a vague follow-up. Use the
failure type to choose one of the following actions.

| Failure type | Required action | May move to next phase? |
| --- | --- | --- |
| Sign-off finding has no ledger row | Add or split a blocking ledger row, then rerun reconciliation. | No |
| Ledger row has no coverage responsibility | Update/split the coverage table or record an explicit design exclusion. | No |
| Ledger row has multiple plausible owners | Split the row by owner/layer or add a deterministic owner arbitration rule. | No |
| Ledger row has no proof command | Define a focused case, fixture, report fixture, or broad rerun command before implementation. | No |
| Proof command fails with same finding | Keep the row open and create a narrower implementation task in the same phase. | No |
| Proof command fails with a new finding | Add the new finding to reconciliation, map it to a row, then decide whether it belongs to the current or next phase. | Only if mapped with rationale |
| A blocker is truly provider/model/environment limited | Mark `blocked_external` only after owner/action/evidence are present and the limitation is recorded in the final report. | Yes, but Phase20 cannot call it fully migrated unless it is excluded or accepted by design |
| A coverage responsibility remains `Partial` or `Missing` | Continue migration work or explicitly exclude the responsibility with design rationale. | No for Phase20 |
| Final broad sign-off exits non-zero | Convert every finding into ledger rows and continue Phase18/19-style recovery. | No for Phase20 |

The default action is to keep the current phase open. Creating Phase21 is
allowed only when the new work is a distinct responsibility class that cannot
fit Phase18 or Phase19 without making them ambiguous. A Phase21 must be added
to this roadmap with the same ledger/reconciliation/proof gates before work
starts.

## Completion Failure Escalation

If the same ledger row fails its proof gate twice after targeted fixes, do not
keep patching blindly. Escalate the row into a design review note with:

- observed behavior after each attempt;
- why the previous owner/action/target hypothesis was insufficient;
- whether the issue is in planning, execution, recovery task, profile, setup,
  verifier, eval/reporting, or provider transport;
- whether the row should be split;
- whether an Anvil mechanism was still missed;
- the next proof command.

This review note must be attached to the ledger row before another
implementation attempt. The goal is to avoid repeating the Phase1-16 mistake of
continuing phase work without proving closure.

## Migration Complete Decision

There are only three valid end states:

1. `migration_complete`
   - all adopted coverage rows are `Implemented`;
   - all ledger rows are `closed_proven`;
   - final broad sign-off exits zero;
   - final report is written.
2. `migration_complete_with_explicit_exclusions`
   - all remaining non-proven rows are intentionally `Excluded`;
   - each exclusion has design rationale;
   - no unowned sign-off finding remains;
   - final report clearly says the excluded behavior is not migrated.
3. `migration_not_complete`
   - any adopted row remains `Partial` or `Missing`;
   - any sign-off finding is unmapped;
   - any unowned failure remains;
   - any blocker lacks owner/action/evidence;
   - final broad sign-off exits non-zero.

Do not use "complete" for the third state, even if CI passes.

## Non-goals

- Do not rerun until green.
- Do not weaken sign-off gates.
- Do not classify missing evidence as model quality.
- Do not add hidden runtime loops.
- Do not hide provider/model-specific behavior in shared runtime policy.
