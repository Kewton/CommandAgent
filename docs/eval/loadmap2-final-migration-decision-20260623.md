# Anvil Migration Decision - 2026-06-23

## Decision

```text
migration_not_complete_pending_current_eval_reconciliation
```

This report supersedes the earlier Phase32 completion claim. A fresh local LLM
eval run on 2026-06-23 showed that the previous Phase32 sign-off roots covered
47 unique cases, while the current eval roots cover 91 unique cases. The
previous sign-off omitted 44 current cases, so it is no longer sufficient as a
final migration-complete proof.

CommandAgent has not yet completed the accepted Anvil migration surface
recorded in `docs/eval/legacy-control-stack-coverage-20260621.md` under the
current eval case set.

This is not a byte-for-byte Anvil engine port. The completed surface is the
explicit contract and bounded recovery-control surface that CommandAgent
intentionally adopts:

```text
failure observation
  -> artifact ledger / scope / ownership
  -> completion evidence and evidence binding authority
  -> active job arbitration and dispatch
  -> target admission and prioritization
  -> semantic failure report and repair plan
  -> repair brief, action envelope, and tool policy
  -> bounded repair or setup/verifier action
  -> verifier/profile/evidence rerun
  -> pass or explicit safe stop
```

The remaining Anvil surfaces are explicitly excluded because they conflict
with CommandAgent's minimal-loop architecture or are legacy UI/advisory
compatibility surfaces.

## Source Baseline

| Field | Value |
| --- | --- |
| Anvil repository | `/Users/maenokota/share/work/github_kewton/Anvil-develop` |
| Anvil HEAD | `b3ca3d330546a10bf90d8dd46bd3e102f1710573` |
| Dirty state | Dirty at inventory time; treatment fixed in `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md` |
| CommandAgent closure branch | `develop` |

## Coverage Result

| Final coverage state | Count |
| --- | ---: |
| Implemented | 45 |
| Partial | 0 |
| Missing | 0 |
| Excluded | 9 |

Adoption result:

| Adoption decision | Count |
| --- | ---: |
| Adopted / implemented | 45 |
| Partial | 0 |
| Missing | 0 |
| Excluded | 9 |

There are no adopted `Partial` rows and no adopted `Missing` rows.

## Implemented Rows

| Rows | Closed by | Proof summary |
| --- | --- | --- |
| C01-C03 | Phase22 | Task contract, request admission, and behavior obligations. |
| C04-C06 | Phase23 | Artifact role, workspace scope, and ownership. |
| C07-C10 | Phase24 | Artifact ledger, completion evidence, evidence binding, and deliverable audit. |
| C11-C12 | Phase25 | Active-job arbitration and recovery dispatch. |
| C13-C20 | Phase26 | Recovery task packets, setup/profile mapping, semantic repair, repair brief, and action envelope. |
| C21-C32 | Phase27 | Target admission, target prioritization, repair lifecycle, verifier orchestration, completion job, focused edit, no-progress, and patch validation. |
| C33 | Phase28 | Contract conflict job and authority decision. |
| C34-C44 | Phase29 | Language/profile/tool/workspace/runtime support. |
| C45 | Coverage table | Provider transport parser is already implemented and policy-free. |

## Explicit Exclusions

| Rows | Exclusion rationale |
| --- | --- |
| C46-C48 | Working memory/reminders, case records, anti-pattern corpora, and PAM/Photon advisory systems are outside the MVP and would reintroduce advisory sidecars. |
| C49 | Anvil semantic quality confirmation and secondary model feedback classification are excluded; CommandAgent uses deterministic eval/report taxonomy and recovery evidence instead. |
| C50 | Anvil slash/plan UI helper compatibility is excluded; CommandAgent keeps its native CLI/REPL slash parser and documentation. |
| C51 | Legacy engine selector is excluded; CommandAgent has one execution engine. |
| C52 | Hidden or unbounded repair loop is excluded; repair remains bounded and user-visible. |
| C53 | Provider/model-specific behavioral policy is excluded; shared behavior stays outside provider transports. |
| C54 | Model-issued dependency installation is excluded; setup can be explicit and evidence-bound but not implicitly model-driven. |

## Historical Proof Roots

| Label | Root |
| --- | --- |
| smoke | `eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759` |
| focused | `eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638` |
| focused-fixture | `eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335` |
| large | `eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624` |

Historical sign-off command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --root large=eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624
```

Historical result:

```text
status: pass
```

Current sign-off roots:

| Label | Root |
| --- | --- |
| smoke | `eval/runs/current-all-local-llm/smoke/20260623T203030` |
| focused | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` |

Current result:

```text
status: fail
```

Current blockers are tracked in:

- `workspace/mvp/logic/anvil/loadmap2/phase_32/current_eval_manifest.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_32/recovery_task_ledger.md`

## Completion Checklist

- [ ] Current eval manifest and sign-off roots cover the same case set.
- [ ] Current broad local LLM sign-off exits zero.
- [ ] Current focused assertions pass recheck or have explicit row-level
  disposition.
- [ ] Current large failures are owned, actionable, and target/evidence bound,
  or explicitly accepted as external limitations.
- [ ] No adopted row depends only on historical roots that omit current cases.
- [ ] Final report states the current decision without relying on superseded
  evidence.
- [x] No raw `rc:1` remains without diagnostic classification in the accepted sign-off roots.
- [x] No profile failure is disconnected from recovery job selection in the accepted sign-off roots.
- [x] No evidence/completion success is claimed without bound evidence in the accepted sign-off roots.
- [x] No repair prompt is built without selected owner, target, action, tool policy, and rerun authority in the accepted sign-off roots.
- [x] No repeated no-progress repair continues without strategy switch or explicit stop in the accepted sign-off roots.
- [x] Final architecture remains one minimal loop with explicit contracts and bounded recovery.

## Current Limitations

No accepted external proof limitation is being used to close the current
Phase32 recovery state.

The historical large proof root is non-timeboxed for proof purposes and still
contains large-task failures. Under the current eval roots, large failures must
be reclassified against the current case set before migration completion can be
declared.

## Current Statement

The Anvil migration is not complete under the current eval case set. The
historical Phase32 evidence remains useful regression evidence, but it is not a
complete final migration proof because it omitted 44 current cases.
