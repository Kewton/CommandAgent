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

1. The recovery ledger is the source of truth for Phase17+ blocker execution,
   subordinate to the coverage table for final row adoption and final row
   state.
2. Every blocker must have one owning layer.
3. Every blocker must have a proof command or focused eval case.
4. A blocker cannot be closed by documentation alone.
5. A blocker cannot be closed by CI alone.
6. A broad eval failure can be accepted only when it is owned, actionable, and
   explicitly classified as an environment, provider, or model-throughput
   limit.
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
10. A blocker marked as model-throughput, provider, or environment limitation must
    still have owner/action/evidence. Missing ownership or missing evidence is
    never a model-throughput limitation.

## Authority Order

For Phase17 and later, read the documents in this order when a conflict is
found:

1. `docs/eval/legacy-control-stack-coverage-20260621.md` owns final
   coverage-row adoption and final row state.
2. This `recovery_plan.md` owns continuation phase gates, recovery rules, and
   blocker disposition semantics.
3. Phase-local `row_closure_matrix.md`, `blocking_ledger.md`, and
   `reconciliation.md` own the execution details for rows assigned to that
   phase, but they cannot override the coverage table or this recovery plan.
4. `current_issue_phase_map.md` is a derived navigation map for known issues
   and planned phases.
5. Older roadmap phase sections are historical when they conflict with this
   recovery plan.

Phase20 is therefore a historical checkpoint. It produced the
`migration_not_complete` decision and the continuation ledger; it is not the
current migration-complete authority. Phase32 is the current final closure
phase.

The Anvil source baseline is the coverage-table baseline:

- repository: `/Users/maenokota/share/work/github_kewton/Anvil-develop`
- HEAD: `b3ca3d330546a10bf90d8dd46bd3e102f1710573`
- dirty state: dirty at inventory clarification time; fixed in
  `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`

Refresh the coverage table first if a later Anvil checkout is used for parity
claims.

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

Phase20 proved that final migration completion could not be declared yet. The
Phase21 continuation then split `P20-COV-001` into row-level blockers.

The revised continuation flow is:

```text
Phase 21: Core contract and ownership row-level admission
Phase 22: C01-C03 task contract, request admission, behavior obligations
          (completed / closed_proven)
Phase 23: C04-C06 artifact role, workspace scope, ownership
          (completed / closed_proven)
Phase 24: C07-C10 artifact ledger, completion evidence, evidence binding,
          deliverable obligation audit
          (completed / closed_proven)
Phase 25: C11-C12 active-job arbitration and recovery dispatch gate
          (completed / closed_proven)
Phase 26: C13-C20 recovery task, setup/profile, semantic repair/action envelope
          (completed / closed_proven)
Phase 27: C21-C32 target admission, repair lifecycle, verifier, completion,
          and patch validation
          (completed / closed_proven)
Phase 28: C33 contract conflict job
          (completed / closed_proven)
Phase 29: C34-C44 language/profile/tool/workspace/runtime support
          (completed / closed_proven)
Phase 30: C49-C50 priority decision rows
          (completed / closed_excluded)
Phase 31: P20-LEDGER-001 fresh large timeout proof
          (completed / closed_proven)
Phase 32: Final coverage closure and migration-complete decision
```

Phase32 is now the only phase allowed to declare migration complete. Earlier
phases can close only their assigned blockers.

## Phase Exit Gates

| Phase | Exit gate |
| --- | --- |
| Phase 17 | Every current sign-off finding is mapped to a ledger row, coverage responsibility, downstream phase, and proof command. |
| Phase 18 | All focused ledger rows assigned to Phase18 are `closed_proven`; focused sign-off has no failed expected assertions or raw diagnostics. |
| Phase 19 | All large ledger rows assigned to Phase19 are `closed_proven` or `blocked_external` with owner/action/evidence; broad sign-off has no unowned large failure. |
| Phase 20 | Historical checkpoint: produced the `migration_not_complete` decision and continuation ledger. Superseded by Phase32 for final closure. |
| Phase 21 | C01-C12 are each `closed_proven`, `excluded_with_rationale`, or `split_forward` with owner, proof command, downstream phase, and closure condition. |
| Phase 22 | C01-C03 are `closed_proven` or split into narrower same-surface blockers with failed proof evidence. |
| Phase 23 | C04-C06 are `closed_proven` or split into narrower same-surface blockers with failed proof evidence. |
| Phase 24 | C07-C10 are `closed_proven` or split into narrower same-surface blockers with failed proof evidence. |
| Phase 25 | C11-C12 are `closed_proven` or split into narrower same-surface blockers with failed proof evidence. |
| Phase 26 | C13-C20 are row-level reconciled and proven for recovery task/setup/profile/semantic repair/action-envelope behavior. |
| Phase 27 | C21-C32 are row-level reconciled and proven for target, verifier, repair lifecycle, completion, and patch behavior. Status: `closed_proven` by Phase27 focused fixture root `eval/runs/loadmap2-phase27-focused-fixtures/20260623T144917`, targeted tests, and broad sign-off. |
| Phase 28 | C33 contract conflict job is `closed_proven`: authority decision, repair-target-side projection, focused C33 fixture root `eval/runs/loadmap2-phase28-contract-conflict-fixtures/20260623T152521`, and broad sign-off pass. |
| Phase 29 | C34-C44 are `closed_proven`: Phase29 runtime-support fields, command classification, workspace candidate policy, job/scaffold/noncoding/lifecycle/provider boundary projection, focused fixture root `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335`, targeted tests, and broad sign-off pass. |
| Phase 30 | C49-C50 are `closed_excluded`: Phase30 records design rationale excluding Anvil semantic quality confirmation and slash/plan UI helper compatibility, updates coverage, and verifies with docs/report and slash parser checks. |
| Phase 31 | P20-LEDGER-001 is `closed_proven` by fresh no-timeout large root `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`, large recheck, and broad sign-off pass. |
| Phase 32 | Completed: coverage table has no unresolved adopted `Partial` or `Missing`; all ledgers are closed or excluded; final broad sign-off exits zero; final report declares `migration_complete_with_explicit_exclusions`. |

`blocked_external` is not allowed for missing owner/action/evidence. It is only
allowed for a provider, model-throughput, network, or environment constraint
after ownership and evidence are already present.

`blocked_external` is a ledger-level proof disposition, not a coverage-row
implementation state. By Phase32 it must be converted into one of these
outcomes:

- `Implemented` coverage with accepted external proof limitation, when the
  responsibility is implemented but the final proof is blocked by provider,
  model-throughput, network, or environment conditions.
- `Excluded` coverage with design rationale, when the responsibility itself
  will not be migrated.

It must not remain as an excuse for missing owner, missing action, missing
target, or missing evidence.

The final report must name each accepted external proof limitation and state:

- implemented owner and enforced contract;
- unavailable proof and why it is external;
- last attempted proof command or eval root;
- why the limitation does not hide an Anvil migration gap.

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

The default action is to keep the current phase open. Creating a new phase
after Phase32 is allowed only when a new distinct responsibility class is
discovered and the current phase cannot own it without ambiguity. The new
phase must be added to this roadmap with ledger/reconciliation/proof gates
before work starts.

`split_forward` closes only the current phase's responsibility to account for
the discovered failure. It does not close the coverage row or migration. A
split is valid only when it names the narrower blocker, owner layer, downstream
phase, failed proof, and closure condition. At Phase32, any `split_forward`
means the final decision is `migration_not_complete` until the extended phase
is completed.

## Known Issue To Phase Map

`current_issue_phase_map.md` is the single known-issue-to-phase table for
Phase22 and later. This recovery plan owns the rules and exit gates; the issue
map owns the navigational list of KI rows and assigned phases. Do not duplicate
the KI table here.

## Phase22+ Implementation Contract

Every Phase22+ implementation phase must create its own local execution
package before changing runtime behavior.

Required and conditional phase-local files:

| file | status | required content |
| --- | --- | --- |
| `README.md` | always required | Scope, selected rows, non-goals, owner layers, design alignment, horizontal rollout, exit gate. |
| `implementation_tasks.md` | always required | Row-level task checklist, docs updates, eval work, verification commands, failure handling. |
| `concrete_work_plan.md` | always required | Ordered steps, target files/modules, unit tests, focused eval cases, broad sign-off rerun. |
| `source_alignment_matrix.md` | always required | One row per selected coverage ID mapping Anvil source files, adopted behavior, omitted behavior, CommandAgent target modules, and proof method. |
| `row_closure_matrix.md` | always required | Coverage row, source mechanism, current status, owner, missing contract, target modules, proof, disposition. |
| `blocking_ledger.md` | always required | Blocker id, coverage row, owner layer, incomplete contract, suspected module, downstream task, proof command, closure condition, status. |
| `reconciliation.md` | always required | Source blocker to row to implementation task to proof command to final sign-off chain. |
| `focused_worklist.md` | conditionally required | Focused eval cases, expected assertion changes, recheck roots. Required only when model-facing behavior changes, focused assertions change, or focused proof is part of the exit gate. |
| `implementation_report.md` | required at closure | Final row disposition counts, proof results, remaining blockers, review result. |

Minimum row fields:

- coverage id;
- source mechanism;
- owner layer;
- incomplete or failed contract;
- suspected module family;
- target files or modules;
- Anvil source files;
- adopted behavior;
- intentionally omitted behavior;
- expected runtime decision or enforced contract;
- unit proof command;
- focused eval proof, if model-facing;
- broad sign-off dependency;
- closure condition;
- final disposition.

Review gate before implementation:

- No selected row may remain grouped only by Phase20 blocker id.
- No selected row may lack owner, target, proof command, or closure condition.
- Any row with multiple owners must be split before runtime changes.
- Any row with no deterministic proof must define a fixture/report proof first.
- Existing partial code may be cited as context, but not as completion proof.

Review gate before phase closure:

- Every assigned row is `closed_proven`, `excluded_with_rationale`,
  allowed `blocked_external`, or split to a narrower owned blocker with failed
  proof evidence.
- The phase report names the exact proof commands and results.
- The coverage table is updated only for rows with proof or explicit exclusion.
- Broad sign-off is rerun after behavior changes.
- Remaining blockers are not hidden in prose; they appear in a ledger row.

Broad sign-off is a required regression and ownership check after behavior
changes, and it is mandatory for final Phase32 closure. It is not sufficient
by itself to close a row. Each row still needs the row-specific proof listed in
its phase matrix, such as a unit test, deterministic fixture, focused eval, or
E2E proof.

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

Coverage `Partial` and adoption `Partial` are intentionally different:

- Coverage `Partial` means the implementation/proof is incomplete and must
  become `Implemented` or `Excluded`.
- Adoption `Partial` means a scoped subset is intentionally adopted. The
  adopted subset must be split into rows that become `Implemented`; omitted
  behavior must become `Excluded` with rationale.

Neither form of `Partial` may remain as a final Phase32 state.

## Phase32 Final Result

Phase32 closed the recovery plan with:

```text
migration_complete_with_explicit_exclusions
```

Closure evidence:

- coverage table final state: `Implemented=45`, `Partial=0`, `Missing=0`,
  `Excluded=9`;
- final report: `docs/eval/anvil-migration-complete.md`;
- final broad sign-off:
  `python3 scripts/eval_signoff.py --require-recheck ...`, result
  `status: pass`;
- Phase32 implementation report:
  `workspace/mvp/logic/anvil/loadmap2/phase_32/implementation_report.md`.

No accepted external proof limitation was required for final closure.

## Non-goals

- Do not rerun until green.
- Do not weaken sign-off gates.
- Do not classify missing evidence as model quality.
- Do not add hidden runtime loops.
- Do not hide provider/model-specific behavior in shared runtime policy.
