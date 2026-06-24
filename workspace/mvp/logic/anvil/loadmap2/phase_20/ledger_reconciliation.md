# Phase 20 Ledger Reconciliation

Date: 2026-06-23 JST

## Summary

Phase20 reconciled the Phase17 recovery ledger against Phase18 focused proof,
Phase19 large proof, and the final broad sign-off command.

Final broad sign-off passes, and no Phase17 ledger row remains `open`.
However, `P17-L001` remains `blocked_external`; under the recovery plan, that
prevents pure `migration_complete` unless it is converted to
`closed_proven`. It can only be accepted as an explicit limitation or blocker
in a non-pure final decision.

## Proof Command

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

## Ledger Rows

| id | phase | final status | proof source | proof command/report | final decision impact |
| --- | --- | --- | --- | --- | --- |
| P17-F001 | Phase18 | closed_proven | `docs/eval/loadmap2-phase18-focused-recovery-20260623.md` | focused local LLM root plus full focused sign-off | Compatible with `migration_complete`. |
| P17-F002 | Phase18 | closed_proven | `docs/eval/loadmap2-phase18-focused-recovery-20260623.md` | focused local LLM root plus full focused sign-off | Compatible with `migration_complete`. |
| P17-F003 | Phase18 | closed_proven | `docs/eval/loadmap2-phase18-focused-recovery-20260623.md` | focused local LLM root plus full focused sign-off | Compatible with `migration_complete`. |
| P17-F004 | Phase18 | closed_proven | `docs/eval/loadmap2-phase18-focused-recovery-20260623.md` | focused local LLM root plus full focused sign-off | Compatible with `migration_complete`. |
| P17-L001 | Phase19 | blocked_external | `docs/eval/loadmap2-phase19-large-recovery-20260623.md` | large time-boxed root and broad sign-off; timeout rows have provider/eval ownership and not-applicable evidence semantics | Incompatible with pure `migration_complete`; acceptable only as explicit limitation or `migration_not_complete` blocker. |
| P17-L002 | Phase19 | closed_proven | `docs/eval/loadmap2-phase19-large-recovery-20260623.md` | large recheck plus broad sign-off | Compatible with `migration_complete`. |
| P17-L003 | Phase19 | closed_proven | `docs/eval/loadmap2-phase19-large-recovery-20260623.md` | large recheck plus broad sign-off | Compatible with `migration_complete`. |
| P17-L004 | Phase19 | closed_proven | `docs/eval/loadmap2-phase19-large-recovery-20260623.md` | large recheck plus broad sign-off | Compatible with `migration_complete`. |

## Checks

| Check | Result |
| --- | --- |
| No `open` Phase17 row remains | pass |
| No row is closed by CI-only evidence | pass |
| Every focused row maps to Phase18 proof | pass |
| Every large row maps to Phase19 proof | pass |
| `blocked_external` has owner/action/evidence | pass |
| `blocked_external` is compatible with pure `migration_complete` | fail |

## Phase20 Treatment Of P17-L001

`P17-L001` is accepted as an owned external/provider-eval limitation for the
Phase19 broad sign-off proof. It is not accepted as a pure completion row.

Because the coverage table also has adopted unresolved rows, the Phase20 final
decision is `migration_not_complete`, not
`migration_complete_with_explicit_exclusions`.
