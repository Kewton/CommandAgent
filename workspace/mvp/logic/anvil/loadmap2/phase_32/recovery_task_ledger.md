# Phase32 Recovery Task Ledger

Date: 2026-06-23 JST

Status: open / recovery in progress

## Recovery Decision

The previous Phase32 decision is superseded.

Current decision:

```text
migration_not_complete_pending_current_eval_reconciliation
```

Reason:

- Previous Phase32 signoff roots covered 47 unique cases.
- Current eval roots cover 91 unique cases.
- 44 current cases were not present in the previous accepted signoff roots.
- Current broad signoff on fresh roots fails.

## Recovery Tasks

| id | owner layer | task | status | evidence | closure condition |
| --- | --- | --- | --- | --- | --- |
| P32-R001 | Phase32 decision | Revoke final migration-complete decision and mark KI-011 open again. | completed | This ledger plus roadmap/final-report updates. | Public docs no longer claim final migration completion from stale roots. |
| P32-R002 | Eval manifest | Add a current eval manifest comparing accepted roots to current eval roots. | completed | `phase_32/current_eval_manifest.md`. | Missing 44 cases are listed with matrix row and proof mode. |
| P32-R003 | Eval report | Reproject deterministic fixture fields during `--recheck`. | completed | `scripts/eval_report.py`; `tests/test_eval_report.py`. | Fixture recheck no longer drops `fixture_fields`; regression test passes. |
| P32-R004 | Focused current eval | Re-run focused recheck after P32-R003. | completed | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236/recheck_summary.tsv`. | False fixture-field assertion failures drop from 45 to 35. |
| P32-R005 | Broad signoff | Re-run current broad signoff on current roots. | completed / pass after Phase35 | `python3 scripts/eval_signoff.py --require-recheck ...` returned `status: pass` after Phase35. | Current broad signoff exits zero on current roots. |
| P32-R006 | Focused assertions | Classify and fix or accept the remaining focused assertion failures. | completed by Phase35 | `focused_worklist.md`; `phase_33/implementation_report.md`; `phase_35/implementation_report.md`; updated focused `recheck_summary.tsv`. | Current focused recheck reports `passed_recheck: 82`. |
| P32-R007 | Raw diagnostic coverage | Eliminate or explicitly accept remaining raw `rc:1` / `rc_1` / unknown-contract findings. | completed by Phase34 | `phase_34/implementation_report.md`; current large `recheck_summary.tsv`; current broad signoff output. | Phase34 leaves no unowned raw diagnostic in signoff output. Remaining signoff failures are focused assertion blockers assigned to Phase35+. |
| P32-R008 | Large real LLM blockers | Classify six large failures into migration blocker, model-quality failure, or explicit limitation. | classified / implementation open | Current large summary below; `followup_phase_split.md` Phase36. | Large rows have owner/action/target/evidence or accepted limitation. |
| P32-R009 | Row-to-case mapping | Add row -> eval case -> proof root -> recheck result mapping for all C01-C54 adopted rows. | open / assigned | Current manifest plus coverage table; `followup_phase_split.md` Phase37 and Phase38. | No adopted row can be closed without a current proof case or documented rationale, and sign-off roots cover the current case set. |
| P32-R010 | Follow-up phase split | Create follow-up phases for remaining runtime/eval-report blockers after P32-R006 to P32-R009 classification. | completed | `followup_phase_split.md`. | Each open blocker has assigned phase, owner layer, source evidence, and proof command family. |

## Current Focused Recheck Summary

| metric | value |
| --- | ---: |
| focused cases | 82 |
| focused success | 9 |
| focused assertion failures after P32-R003 | 35 |
| focused assertion failures after Phase33 | 4 |
| focused assertion failures after Phase35 | 0 |
| focused assertion failures surfaced by current broad signoff after Phase35 | 0 |
| focused raw diagnostic / unknown findings | 65 |

The 35 count comes from focused `recheck_summary.tsv`
`expected_assertion_status=failed_recheck`. The 18 count comes from the
non-duplicated current broad signoff with `smoke`, `focused`, and `large`
roots. The raw diagnostic count includes rows whose recheck reason remains
`rc:1` even when some diagnostic metadata is available. These require follow-up
in either eval-report recheck classification or fixture case construction.

Focused failure groups are recorded in `focused_worklist.md`. Phase33 closed
the eval/report recheck projection subset. Phase35 closed the four remaining
focused assertion failures: `focused-dispatch-manifest-repair`,
`focused-nextjs-dependency-setup`, `focused-nextjs-endpoint-smoke`, and
`focused-nextjs-route-integration`. Current focused recheck reports
`passed_recheck: 82`, and current broad signoff no longer reports focused
assertion failures.

## Current Large Summary

| case | category | terminal state | diagnostic | owner/action status |
| --- | --- | --- | --- | --- |
| `large-fastapi-app-modify` | verifier | verifier_command_failed | unknown_verifier_failure | owner/action/target present |
| `large-fastapi-app-new` | verifier after recheck | verifier_command_failed | tool_args_missing_required_field | tool protocol evidence present |
| `large-nextjs-app-modify` | verifier | verifier_command_failed | edit_target_not_found | owner/action/target present |
| `large-nextjs-app-new` | step_policy | step_policy_failed | read_only_step_mutation | classified but contract layer still needs signoff-safe mapping |
| `large-rust-app-modify` | verifier | verifier_command_failed | edit_target_not_found | owner/action/target present |
| `large-rust-app-new` | verifier | verifier_command_failed | blocked_bash_command_policy | Phase34 admitted `src/main.rs` from profile/workspace evidence; large blocker remains failed but no longer raw/unowned |

These six rows are assigned to Phase36 by `followup_phase_split.md`. Phase32
does not classify them as accepted external limitations.

## Phase32 Exit Gate After Recovery

Phase32 may declare migration completion only when all are true:

1. current eval manifest and signoff roots cover the same case set;
2. current broad signoff exits zero;
3. current focused assertions pass or have explicit row-level disposition;
4. current large failures are owned, actionable, and target/evidence bound, or
   explicitly accepted as external limitations;
5. no adopted row depends only on historical roots that omit current cases;
6. final report states the current decision without relying on superseded
   evidence.
