# Phase32 Row Closure Matrix

Date: 2026-06-23 JST

Status: superseded by Phase39 / closed_current_final_decision

Phase39 supersedes this historical closure matrix with:

```text
migration_complete_with_explicit_exclusions
```

See `../phase_39/decision_evidence_matrix.md` and
`../phase_39/final_closure_report.md` for the current final closure evidence.

## Closure Rows

| row | owner layer | current state before Phase32 | planned disposition | proof artifact or command | closure condition |
| --- | --- | --- | --- | --- | --- |
| FC-01 / coverage final-state audit | Coverage authority | Historical coverage counts were recorded, but current proof roots cover more cases than the historical sign-off roots. | `reopened_current_eval_gap`. | `current_eval_manifest.md`; coverage table; current sign-off output. | Current case set is mapped to adopted/excluded rows and no adopted unresolved row remains. |
| FC-02 / ledger closure audit | Recovery plan / phase-local ledgers | KI-001 through KI-010 were historically closed; KI-011 is reopened. | `reopened_current_eval_gap`. | `recovery_task_ledger.md`; phase-local reports; KI map. | Current focused/large blockers have row-level dispositions. |
| FC-03 / exclusion rationale audit | Coverage authority / architecture docs | C46-C54 and C49-C50 remain excluded. | `closed_excluded`. | Coverage table, Phase30 report, final report. | Exclusions are explicit and not adopted gaps. |
| FC-04 / final broad sign-off | Eval/sign-off | Historical sign-off passed, but current sign-off fails. | `open`. | `python3 scripts/eval_signoff.py --require-recheck ...` on current roots. | Current Phase32 sign-off returns `status: pass`. |
| FC-05 / final migration report | Documentation / eval reporting | Final report existed but overstated completion against historical roots. | `reopened_current_eval_gap`. | `docs/eval/loadmap2-final-migration-decision-20260623.md`. | Report states the current decision and does not rely on superseded evidence. |
| KI-011 | Recovery plan final closure | Reopened by current eval case-set gap. | `open`. | Roadmap updates plus recovery ledger. | Current issue map and recovery plan mark Phase32 closed only after current sign-off passes. |

## Allowed Dispositions

| disposition | allowed in Phase32 | condition |
| --- | --- | --- |
| `closed_proven` | Yes. Preferred for final closure rows. | Row-specific audit/proof exists and final sign-off passes. |
| `closed_excluded` | Yes only for rows already excluded by coverage authority. | Exclusion has design rationale and is not hiding adopted behavior. |
| `blocked_external` | Only as an accepted external proof limitation in the final report. | Owner/action/evidence already exist; proof is blocked by provider/model-throughput/network/environment. |
| `split_forward` | Not a completion state. | Allowed only if a new distinct responsibility class is discovered; final decision becomes `migration_not_complete`. |

## Review Notes

- Phase32 rows are closure controls, not new runtime coverage IDs.
- FC rows prevent the final decision from collapsing into a single prose
  statement.
- `KI-011` closes only after the FC rows close against the current eval roots.
