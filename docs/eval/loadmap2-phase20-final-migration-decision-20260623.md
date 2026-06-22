# Loadmap2 Phase20 Final Migration Decision

Date: 2026-06-23 JST

## Scope And Inputs

Phase20 reconciled the Anvil control-stack migration state after Phase19.

Inputs:

- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `docs/eval/loadmap2-phase18-focused-recovery-20260623.md`
- `docs/eval/loadmap2-phase19-large-recovery-20260623.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_20/coverage_closure.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_20/ledger_reconciliation.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_20/continuation_ledger.md`

Baseline:

- commit: `5b562cf`
- branch: `develop`
- tracked dirty flag before Phase20 docs: clean

## Coverage Final Counts

Current coverage table counts:

| Status | Count |
| --- | ---: |
| Implemented | 1 |
| Partial | 44 |
| Missing | 2 |
| Excluded | 7 |

Adoption decision counts:

| Adoption decision | Count |
| --- | ---: |
| Adopt | 33 |
| Partial | 12 |
| Missing | 2 |
| Excluded | 7 |

Final Phase20 closure candidates:

| Candidate | Count |
| --- | ---: |
| Implemented | 1 |
| Excluded | 7 |
| Unresolved accepted migration surface | 44 |
| Unresolved priority-decision surface | 2 |

## Ledger Reconciliation Summary

Phase17 ledger rows:

| Status | Rows |
| --- | --- |
| closed_proven | P17-F001, P17-F002, P17-F003, P17-F004, P17-L002, P17-L003, P17-L004 |
| blocked_external | P17-L001 |
| open | none |

`P17-L001` has owner/action/evidence and is acceptable as an external
provider/eval limitation for Phase19 sign-off. It is not compatible with pure
`migration_complete` because the recovery plan requires every ledger row to be
`closed_proven` for that decision.

## Final Broad Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
# Eval Sign-off

status: pass
```

## Final Decision

`migration_not_complete`

Reason:

- broad sign-off passes, but sign-off only proves that current failures are
  owned and evidence-bound;
- the coverage table still has 44 accepted migration-surface rows in
  `Partial` state or equivalent unresolved state;
- one accepted row, `Contract conflict job`, remains `Missing`;
- `P17-L001` remains `blocked_external` and is not pure completion evidence;
- two rows remain unresolved priority-decision surfaces.

The decision is intentionally not softened into
`migration_complete_with_explicit_exclusions` because the unresolved accepted
surface is too broad to treat as intentional exclusion.

## Explicit Exclusions

Existing exclusions remain unchanged:

- Working memory/reminders
- Case record and anti-pattern corpora
- PAM/Photon advisory
- Legacy engine selector
- Hidden or unbounded repair loop
- Provider/model-specific behavioral policy
- Model-issued dependency installation

No new Phase20 exclusion was added.

## Remaining Blockers

Continuation blockers are recorded in:

```text
workspace/mvp/logic/anvil/loadmap2/phase_20/continuation_ledger.md
```

The blockers are grouped as:

- P20-COV-001: core task/contract/ownership surface
- P20-COV-002: recovery task and repair action surface
- P20-COV-003: target/repair/verifier/completion surface
- P20-COV-004: missing contract conflict job
- P20-COV-005: language/profile/tool/workspace/runtime-support surface
- P20-COV-006: unresolved priority-decision rows
- P20-LEDGER-001: `P17-L001` blocked external timeout proof

## Next Action

Do not declare migration completion from Phase20. Start a continuation phase
only after selecting one blocker group and splitting it into row-level work
with owner layer, deterministic proof command, and closure condition.

The main process correction from Phase20 is that a green broad sign-off is
necessary but not sufficient for migration completion. Coverage parity and
ledger closure must also pass.
