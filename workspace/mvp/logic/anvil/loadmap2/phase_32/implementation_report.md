# Phase32 Implementation Report

Date: 2026-06-23 JST

Status: superseded by Phase39 / closed_current_final_decision

## Recovery Notice

This implementation report is superseded by the Phase32 recovery work and the
Phase39 final closure report.

Current Phase39 decision:

```text
migration_complete_with_explicit_exclusions
```

Current closure evidence:

- `workspace/mvp/logic/anvil/loadmap2/phase_39/decision_evidence_matrix.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_39/final_closure_report.md`
- `docs/eval/loadmap2-final-migration-decision-20260624.md`

Historical recovery decision:

```text
migration_not_complete_pending_current_eval_reconciliation
```

Reason:

- previous Phase32 accepted signoff roots covered 47 unique cases;
- current eval roots cover 91 unique cases;
- 44 current cases were not covered by the previous accepted roots;
- current broad signoff on fresh roots fails.

Current recovery artifacts:

- `workspace/mvp/logic/anvil/loadmap2/phase_32/current_eval_manifest.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_32/recovery_task_ledger.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_32/blocking_ledger.md`

The historical report below is retained for traceability, but it is no longer
the final migration decision.

## Scope

Phase32 closed `KI-011` by reconciling the coverage table, Phase22-Phase31
phase-local ledgers, final broad sign-off, and final migration report.

No runtime, provider, profile, or hidden retry behavior changed.

## Implemented Changes

- Added final report:
  - `docs/eval/loadmap2-final-migration-decision-20260623.md`
- Updated loadmap2 final-closure status:
  - `workspace/mvp/logic/anvil/loadmap2/README.md`
  - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
  - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- Added Phase32 planning and closure artifacts under:
  - `workspace/mvp/logic/anvil/loadmap2/phase_32/`
- Added Phase32 final appendix to the coverage table.

## Coverage Audit

| Final coverage state | Count |
| --- | ---: |
| Implemented | 45 |
| Partial | 0 |
| Missing | 0 |
| Excluded | 9 |

There are no adopted `Partial` rows and no adopted `Missing` rows.

## Row Closure

| row | disposition | evidence |
| --- | --- | --- |
| FC-01 | `closed_proven` | Coverage table final counts show Implemented 45, Partial 0, Missing 0, Excluded 9. |
| FC-02 | `closed_proven` | Phase22-Phase31 implementation reports close their assigned ledgers. |
| FC-03 | `closed_proven` | C46-C54 exclusions have design rationale in the coverage table and final report. |
| FC-04 | `closed_proven` | Final broad sign-off returned `status: pass`. |
| FC-05 | `closed_proven` | `docs/eval/loadmap2-final-migration-decision-20260623.md` records the final decision and evidence. |
| KI-011 | `closed_proven` | FC-01 through FC-05 are closed and roadmap documents are updated. |

## Final Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --root large=eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624
```

Result:

```text
status: pass
```

## Final Decision

```text
migration_complete_with_explicit_exclusions
```

The accepted Anvil responsibilities are implemented in CommandAgent as
explicit, bounded contract-recovery controls. The non-adopted legacy advisory,
UI helper, engine-switch, hidden-loop, provider-policy, and model-issued setup
surfaces are excluded with rationale.

## Verification

Planned and completed verification:

- final broad sign-off: `status: pass`
- `python3 tests/test_eval_signoff.py`
- `python3 tests/test_eval_report.py`
- `python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py`
- `git diff --check`

No cargo check was required for Phase32 because the implementation changed
documentation and eval reports only.

## Review Result

- Phase32 did not use CI as migration proof.
- Phase32 did not weaken sign-off gates.
- Phase32 did not add hidden continuation, provider-specific behavior, or a
  legacy engine selector.
- Phase32 closed final migration only after the coverage table, recovery plan,
  current issue map, final report, and sign-off agreed.
