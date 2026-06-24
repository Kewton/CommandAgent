# Phase 20: Final Coverage Closure And Migration Decision

Date: 2026-06-23 JST

## Purpose

Phase20 is the final migration decision phase for the Anvil control-stack
migration. It is not another implementation slice by default. Its job is to
decide, with evidence, whether the migration is:

- `migration_complete`;
- `migration_complete_with_explicit_exclusions`; or
- `migration_not_complete`.

Phase20 is the only phase allowed to declare migration complete.

## Inputs

- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `docs/eval/loadmap2-phase18-focused-recovery-20260623.md`
- `docs/eval/loadmap2-phase19-large-recovery-20260623.md`
- current broad sign-off roots:
  - smoke:
    `eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759`
  - focused:
    `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638`
  - focused fixture:
    `eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659`
  - large:
    `eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149`

## Phase20 Scope

Phase20 owns final closure across three evidence surfaces:

| Surface | Phase20 responsibility |
| --- | --- |
| Coverage table | Every adopted responsibility must be `Implemented`, or explicitly excluded with design rationale. No adopted row may remain `Partial` or `Missing` in a final-complete decision. |
| Recovery ledger | Pure `migration_complete` requires every Phase17 row to be `closed_proven`. A `blocked_external` row can only support `migration_complete_with_explicit_exclusions` when it is accepted by design with owner/action/evidence. No `open` row may remain. |
| Broad sign-off | Final broad sign-off must exit zero. Any failure must map to a ledger row before a final decision. |

Phase20 may produce `migration_not_complete`. That is a valid outcome when
evidence says the migration is not complete. It must not be softened into a
successful migration declaration.

## Non-goals

- Do not weaken coverage or sign-off gates.
- Do not convert `Partial` or `Missing` into `Implemented` without proof.
- Do not hide remaining work in a generic follow-up.
- Do not add hidden retry loops or new runtime behavior only to pass final
  sign-off.
- Do not treat CI success as migration success.
- Do not force a fresh live large rerun if the existing proof roots already
  cover the final decision surface; require a fresh run only when a decision
  depends on behavior not present in existing roots.

## Current Risk

The current coverage table still lists many `Partial` and `Missing` rows. Some
of those may be stale because phases 1-19 added implementation and evidence
without fully updating the coverage table. Some may still be real gaps.

Phase20 must distinguish these cases:

```text
stale coverage row
  -> update to Implemented with proof

intentional non-migration
  -> update to Excluded with rationale

real gap
  -> keep migration_not_complete and create a blocking ledger item
```

No row should be changed by assertion alone.

The current Phase17 ledger includes `P17-L001` as `blocked_external`.
Phase20 must not treat that as pure completion unless it is either proven and
changed to `closed_proven`, or explicitly accepted as an exclusion/limitation
in the final decision.

## Design Alignment

The plan follows the repository design principles:

- deterministic evidence over semantic guessing;
- explicit failure reports over hidden continuation;
- eval scripts and docs are product code;
- planning, execution, verification, and repair remain separate contracts;
- common contracts are preferred before profile-specific fixes;
- no provider/model-specific behavioral policy outside provider transport.

Phase20 also follows the recovery-plan correction:

```text
phase complete
  = implementation complete
  + assigned blockers pass their proof gate
  + remaining blockers are reassigned with explicit rationale
```

For Phase20, the stricter form is:

```text
migration complete
  = adopted coverage closed
  + ledger closed
  + final broad sign-off pass
  + final report written
```

## Architecture Shape

Phase20 should keep the decision layer separate from runtime:

| Component | Role | Must not do |
| --- | --- | --- |
| Coverage closure audit | Reads coverage table and phase reports, reconciles row status and proof. | Execute runtime or silently mark rows implemented. |
| Sign-off proof | Runs existing eval/report/sign-off commands against named roots. | Retry until green or change success criteria. |
| Final decision report | Declares complete / complete with exclusions / not complete. | Hide unresolved adopted rows. |
| Optional helper script | Deterministically counts unresolved coverage states and ledger rows. | Become a workflow engine or repair executor. |

If a helper script is added, it should be a read-only checker over Markdown/TSV
artifacts. It should not update files automatically.

## Horizontal Rollout

Phase20 must assess all migrated workstreams, not only the most recent large
eval blockers:

- A: task contract and behavior obligations
- B: failure observation
- C: artifact graph, scope, ownership, ledger
- D: completion evidence and evidence binding
- E: active job, owner, dispatch
- F: setup/scaffold/profile/dev-server jobs
- G: target admission and prioritization
- H: semantic failure report and repair plan
- I: repair brief, action envelope, tool policy
- J: repair job state and no-progress handling
- K: tool failure recovery
- L: verifier orchestration and command policy
- M: profile/language adapters
- N: eval reporting and broad sign-off

The final report must state whether each workstream is fully implemented,
explicitly excluded, or still incomplete.

## Documentation Updates

Phase20 should update or create:

- `docs/eval/loadmap2-phase20-final-migration-decision-<date>.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `docs/known-limitations.md` if final accepted exclusions or limitations
  change.
- `docs/architecture.md` only if Phase20 adds a new read-only closure checker
  or changes the documented evaluation boundary.
- `workspace/mvp/logic/anvil/loadmap2/phase_20/*` with final proof status.

## Proof Strategy

1. Reconcile coverage table against Phase1-Phase19 reports.
2. Reconcile Phase17 ledger status against Phase18 and Phase19 reports.
3. Regenerate reports/recheck summaries for the chosen roots when needed.
4. Run final broad sign-off.
5. Write final decision report.
6. If the decision is not complete, create a blocking continuation ledger with
   row-level owner/proof commands instead of declaring completion.

## Final Decision Rules

| Decision | Required evidence |
| --- | --- |
| `migration_complete` | All adopted coverage rows are `Implemented`; every Phase17 ledger row is `closed_proven`; final broad sign-off exits zero; final report is written. |
| `migration_complete_with_explicit_exclusions` | All non-implemented coverage rows and non-proven ledger blockers are explicitly `Excluded` or accepted limitations with design rationale, owner/action/evidence, and final report coverage; no unowned sign-off finding remains. |
| `migration_not_complete` | Any adopted row remains `Partial`/`Missing`; any non-excluded ledger row remains `open` or `blocked_external`; any sign-off finding is unmapped; any blocker lacks owner/action/evidence; final broad sign-off exits non-zero. |

## Exit Gate

Phase20 is complete only when:

- final decision report exists;
- final broad sign-off result is recorded;
- coverage table has no unresolved adopted `Partial` or `Missing` rows if the
  decision is `migration_complete` or
  `migration_complete_with_explicit_exclusions`;
- ledger rows are all `closed_proven` for `migration_complete`, or all
  non-proven rows are explicitly accepted as exclusions/limitations for
  `migration_complete_with_explicit_exclusions`;
- every remaining non-implemented responsibility is either explicitly excluded
  or listed as a blocker in a continuation ledger;
- the final decision is stated without ambiguity.

## Plan Review Result Reflected

The initial risk was to make Phase20 a broad implementation phase and repeat
the Phase1-16 pattern of adding mechanisms without proving closure. This plan
keeps Phase20 as a decision and reconciliation phase first. It permits code
only if the audit finds a narrow deterministic checker gap; runtime behavior
changes must become separate blocker rows with proof commands before work
starts.

## Execution Result

Phase20 was executed on 2026-06-23.

Outputs:

- `coverage_closure.md`
- `ledger_reconciliation.md`
- `continuation_ledger.md`
- `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`
- Phase20 appendix in
  `docs/eval/legacy-control-stack-coverage-20260621.md`

Final broad sign-off result:

```text
status: pass
```

Final migration decision:

```text
migration_not_complete
```

Reason: broad sign-off is green, but the adopted coverage surface still has
44 unresolved `Partial`/`Missing` rows and `P17-L001` remains
`blocked_external`, which is incompatible with pure `migration_complete`.
