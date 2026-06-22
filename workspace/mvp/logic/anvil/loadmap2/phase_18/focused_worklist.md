# Phase 18 Focused Recovery Worklist

## Scope

This worklist is derived from Phase17 ledger rows assigned to Phase18.

| Work item | Ledger row | Sign-off findings | Case | Primary owner | Proof |
| --- | --- | --- | --- | --- | --- |
| F18-001 | P17-F001 | S001 | `focused-docs-literal-mismatch` | recovery task / step policy | targeted docs rerun, full focused matrix, sign-off with no S001 |
| F18-002 | P17-F002 | S002 | `focused-nextjs-dependency-setup` | setup / recovery task | targeted dependency setup rerun, full focused matrix, sign-off with no S002 |
| F18-003 | P17-F003 | S003, S005 | `focused-nextjs-endpoint-smoke` | planning / eval observation | targeted endpoint rerun, no raw `rc:*`, full focused matrix, sign-off with no S003/S005 |
| F18-004 | P17-F004 | S004 | `focused-nextjs-route-integration` | profile / planning / recovery task | targeted route rerun, full focused matrix, sign-off with no S004 |

## Closure Rules

- A work item closes only after its targeted proof passes and full focused
  sign-off no longer reports its sign-off finding.
- If the targeted proof fails with the same finding twice after fixes, add a
  design review note before another implementation attempt.
- If a new focused finding appears, add it to Phase17 reconciliation or a
  Phase18 addendum before continuing.
- Large findings do not block Phase18 closure unless a Phase18 change creates
  a new focused or smoke regression.

## Status

| Work item | Status | Notes |
| --- | --- | --- |
| F18-001 | closed_proven | Targeted root `eval/runs/loadmap2-phase18-targeted-docs-v4/20260623T000427`; focused assertions passed in normal and recheck reports. |
| F18-002 | closed_proven | Targeted root `eval/runs/loadmap2-phase18-targeted-nextjs-dependency-v5/20260622T234925`; focused assertions passed in normal and recheck reports. |
| F18-003 | closed_proven | Targeted root `eval/runs/loadmap2-phase18-targeted-nextjs-endpoint-v2/20260622T235427`; no raw `rc:*`; focused assertions passed in normal and recheck reports. |
| F18-004 | closed_proven | Targeted root `eval/runs/loadmap2-phase18-targeted-nextjs-route-v2/20260622T235832`; focused assertions passed in normal and recheck reports. |

## Full Focused Proof

| Proof | Root | Result |
| --- | --- | --- |
| full focused summary | `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638` | 27/27 focused assertions passed. |
| full focused recheck | `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638/recheck_summary.tsv` | 27/27 focused assertions passed after recheck normalization. |
| broad sign-off | same focused root with Phase16 smoke/fixture/large roots | Failed only on Phase19 large rows; no focused findings S001-S005 remained. |
