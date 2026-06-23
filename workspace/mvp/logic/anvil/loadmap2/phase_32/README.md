# Phase32 Final Migration Closure Plan

Date: 2026-06-23 JST

Status: recovery_open / migration_not_complete_pending_current_eval_reconciliation

## Objective

Phase32 is the only phase allowed to declare the final Anvil migration state.

The goal is to close `KI-011` by reconciling the authoritative coverage table,
Phase17+ recovery rules, Phase22-Phase31 phase-local ledgers, and final broad
sign-off evidence.

The preferred closure is:

```text
migration_complete
```

If any adopted row is not proven, the phase must declare
`migration_not_complete` and create a narrower follow-up phase before using
the word "complete".

## Inputs

Authoritative inputs:

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- Phase22-Phase31 `implementation_report.md`, `row_closure_matrix.md`,
  `blocking_ledger.md`, and `reconciliation.md`
- final broad sign-off command output

Original closure baseline:

- Coverage table has `Implemented=45`, `Partial=0`, `Missing=0`,
  `Excluded=9`.
- KI-001 through KI-010 are closed.
- KI-011 was previously closed by Phase32 final migration decision, but is now
  reopened because current eval reconciliation found a coverage gap.
- Phase31 closed `P17-L001` with fresh large proof root
  `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`.

Recovery baseline:

- current eval roots cover 91 unique cases;
- previous accepted Phase32 signoff roots covered 47 unique cases;
- 44 current cases were not covered by previous Phase32 signoff;
- current broad signoff returns `status: fail`;
- Phase32 cannot declare migration completion until current eval
  reconciliation closes.

## Scope

Phase32 covers final closure only:

- verify coverage rows C01-C54 have final states;
- verify all adopted rows have row-level proof or an allowed explicit
  exception;
- verify excluded rows have design rationale;
- verify Phase17+ continuation ledgers contain no open blocker;
- rerun or revalidate final broad sign-off;
- write final migration report;
- update roadmap/status documents to show the final state.

## Non-goals

- Do not add runtime behavior.
- Do not add provider/model-specific policy.
- Do not weaken coverage or sign-off gates.
- Do not classify missing owner/action/target/evidence as model quality.
- Do not introduce hidden retries or hidden continuation.
- Do not re-open already closed coverage rows unless reconciliation finds a
  concrete contradiction.

## Design Alignment

This phase follows the current CommandAgent design:

- final closure is an evidence and documentation action, not an execution
  engine change;
- the coverage table owns row adoption and final row state;
- recovery plan owns disposition semantics;
- phase-local files own execution detail but cannot override authority;
- broad sign-off is mandatory but not a substitute for row-specific proof;
- every unresolved finding becomes a ledger row instead of prose.

## Horizontal Rollout

Phase32 should update only shared completion documents and reports. It should
not create profile-specific or provider-specific behavior.

Expected documentation targets during implementation:

- `docs/eval/loadmap2-final-migration-decision-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `docs/philosophy.md`, `docs/architecture.md`,
  `docs/adr/0002-contract-recovery.md`, and `docs/evaluation.md` only if the
  final closure wording exposes a stale design statement.

## Exit Gate

Phase32 can close because all items are true:

1. coverage table contains no adopted `Partial` or `Missing`;
2. every adopted row is `Implemented` and either stage 5 or explicitly accepted
   with a documented external proof limitation;
3. every `Excluded` row has design rationale;
4. KI-001 through KI-011 are closed;
5. no Phase17+ blocking ledger has an open row;
6. final broad sign-off exits zero on the current eval case set, not only on
   historical accepted roots;
7. final report names migrated responsibilities, excluded responsibilities,
   proof roots, limitations, and the final migration decision;
8. `implementation_report.md` records row disposition counts and exact proof
   commands.
9. `phase_32/current_eval_manifest.md` proves the current eval manifest and
   signoff roots cover the same case set.

## Failure Handling

If the exit gate fails:

- keep Phase32 open;
- record the failed check in `blocking_ledger.md`;
- map the finding to a coverage row, owner layer, target document/module, proof
  command, and closure condition;
- create a new phase only for a new distinct responsibility class;
- declare `migration_not_complete` in the Phase32 report.

## Superseded Implementation Result

Phase32 previously completed with:

- final decision: `migration_complete_with_explicit_exclusions`;
- final broad sign-off: `status: pass`;
- final report: `docs/eval/loadmap2-final-migration-decision-20260623.md`;
- implementation report: `phase_32/implementation_report.md`.

No runtime, provider, profile, or hidden retry behavior changed.

That result is superseded by the current eval reconciliation performed on
2026-06-23. The correct current state is:

```text
migration_not_complete_pending_current_eval_reconciliation
```

Recovery artifacts:

- `phase_32/current_eval_manifest.md`
- `phase_32/recovery_task_ledger.md`
- updated `phase_32/blocking_ledger.md`

## Plan Review Result

The initial plan was reviewed against:

- authority order in `recovery_plan.md`;
- final closure rules in `README.md`;
- current KI map in `current_issue_phase_map.md`;
- Phase31 large proof handoff;
- CommandAgent design constraints in `AGENTS.md`.

Review changes applied:

- made coverage-table reconciliation the first implementation task;
- made final broad sign-off mandatory but not row-closing by itself;
- added explicit failure handling for any contradiction discovered during final
  closure;
- kept runtime and provider behavior out of scope;
- required a final report rather than relying on CI or sign-off output alone.
