# Phase32 Row Closure Matrix

Date: 2026-06-23 JST

Status: completed / migration_complete_with_explicit_exclusions

## Closure Rows

| row | owner layer | current state before Phase32 | planned disposition | proof artifact or command | closure condition |
| --- | --- | --- | --- | --- | --- |
| FC-01 / coverage final-state audit | Coverage authority | Coverage table reports `Implemented=45`, `Partial=0`, `Missing=0`, `Excluded=9`. | `closed_proven`. | Coverage table audit and final report counts. | Counts are recorded and no adopted unresolved row remains. |
| FC-02 / ledger closure audit | Recovery plan / phase-local ledgers | KI-001 through KI-010 were closed; KI-011 was open. | `closed_proven`. | Phase-local reports and KI map. | Phase22-Phase31 ledgers close their assigned rows. |
| FC-03 / exclusion rationale audit | Coverage authority / architecture docs | C46-C54 and C49-C50 are excluded. | `closed_proven`. | Coverage table, Phase30 report, final report. | Exclusions are explicit and not adopted gaps. |
| FC-04 / final broad sign-off | Eval/sign-off | Last Phase31 broad sign-off passed. | `closed_proven`. | `python3 scripts/eval_signoff.py --require-recheck ...`. | Final Phase32 sign-off returned `status: pass`. |
| FC-05 / final migration report | Documentation / eval reporting | No final Phase32 report existed. | `closed_proven`. | `docs/eval/anvil-migration-complete.md`. | Report states `migration_complete_with_explicit_exclusions` and evidence without overstating completion. |
| KI-011 | Recovery plan final closure | Open. | `closed_proven`. | Roadmap updates plus final report. | Current issue map and recovery plan mark Phase32 closed. |

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
- `KI-011` closes only after the FC rows close.
