# Phase32 Source Alignment Matrix

Date: 2026-06-23 JST

Status: recovery_open / reviewed

## Matrix

| closure item | Anvil source / prior evidence | Adopted behavior | Omitted behavior | CommandAgent target | Proof method |
| --- | --- | --- | --- | --- | --- |
| Final coverage authority | Anvil coverage inventory in `docs/eval/legacy-control-stack-coverage-20260621.md` keyed to HEAD `b3ca3d330546a10bf90d8dd46bd3e102f1710573`. | Use coverage IDs C01-C54 as the final responsibility inventory. | Do not compare against a moving Anvil checkout without refreshing the baseline. | Coverage table and Phase32 final report. | Coverage count audit and final report. |
| Adopted control stack rows | Phase22-Phase29 row-level implementation reports for C01-C44 plus C45 provider parser row. | Treat implemented rows as migrated only when row-specific proof exists against the current eval case set. | Do not infer migration from type presence, CI success, or historical roots that omit current cases. | Phase22-Phase29 reports, coverage table, current decision report. | Row closure audit and current broad sign-off. |
| Excluded legacy rows | Coverage rows C46-C54 and Phase30 C49-C50 decision. | Preserve explicit exclusions for memory/advisory, UI helper, legacy engine, hidden loop, provider policy, and model-issued dependency install. | Do not port excluded sidecar/advisory/hidden control surfaces. | Coverage table, Phase30 report, final report. | Exclusion rationale audit. |
| Large proof blocker | Phase31 `P17-L001` closure root `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624`; current large root `eval/runs/current-all-local-llm/large/20260623T204816`. | Use Phase31 as historical timeout proof and current large root as current blocker evidence. | Do not reuse the old root as completion proof for cases not covered by it. | Phase31 report, current manifest, recovery ledger. | Current large rows have owner/action/target/evidence or accepted limitation. |
| Migration decision | Anvil has a broad control stack; CommandAgent adopts explicit contract/recovery-control parity, not a byte-for-byte engine. | Declare final migration state only after coverage, ledgers, current case-set coverage, and current sign-off reconcile. | Do not add legacy engine switch, hidden retry loop, or provider/model behavioral branch. | `docs/eval/loadmap2-final-migration-decision-20260623.md`. | Current decision report. |

## Review Notes

- Phase32 does not need a new Anvil source module inventory; the authoritative
  row inventory already exists in the coverage table.
- The source alignment risk is stale or incomplete proof evidence, not missing
  runtime source alone.
- Any later Anvil baseline refresh must be a new coverage-table update before
  changing the Phase32 decision.
