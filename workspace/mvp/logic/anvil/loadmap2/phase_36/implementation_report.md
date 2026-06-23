# Phase36 Implementation Report

Date: 2026-06-24 JST

Status: completed

## Summary

Phase36 added report-only large failure disposition support and closed the
large real-LLM blocker ownership task from Phase32 P32-R008.

The large tasks still fail as application-generation tasks. Phase36 does not
relabel those task failures as success. It closes only the migration-accounting
question: every failed large row now has owner/action/target/evidence and a
row disposition.

## Code Changes

| area | change |
| --- | --- |
| `scripts/eval_runtime_job_report.py` | Added `large_disposition`, reason, owner/action status, and evidence projection. Tool-protocol, edit-target, explicit-stop, provider-boundary, and manifest-conflict rows are projected from existing evidence only. |
| `scripts/eval_report.py` | Passes original `success_check_reason` to recheck projection so tool missing-field evidence survives `rc:1` recheck normalization. Adds a `Large Disposition` report section. |
| `scripts/eval_signoff.py` | Requires failed large rows to carry a valid disposition and rejects owner/action inconsistency, unclosed implementation blockers, split-forward rows, and invalid external limitations. |
| `tests/test_eval_report.py` | Added coverage for tool-protocol large disposition, edit-target owner/action projection, read-only explicit stop projection, provider external limitation, and report rendering. |
| `tests/test_eval_signoff.py` | Added coverage for missing large disposition, invalid external limitation, owner/action mismatch, and updated accepted large failure fixtures. |

## Documentation Changes

| file | change |
| --- | --- |
| `docs/evaluation.md` | Documents large disposition fields, accepted values, and broad sign-off semantics. |
| `eval/README.md` | Documents large disposition requirements for broad sign-off and clarifies that disposition is attribution, not task success. |
| `phase_36/large_row_ledger.md` | Records one disposition row for each current large case. |
| `phase_32/recovery_task_ledger.md` | Marks P32-R008 complete by Phase36. |
| `phase_32/followup_phase_split.md` | Marks Phase36 closed and points to this report and ledger. |

## Eval Results

Current roots:

```text
smoke:   eval/runs/current-all-local-llm/smoke/20260623T203030
focused: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
large:   eval/runs/current-all-local-llm/large/20260623T204816
```

Commands:

```bash
python3 scripts/eval_report.py \
  eval/runs/current-all-local-llm/large/20260623T204816 \
  --cases-dir eval/cases/large \
  --recheck

python3 scripts/eval_report.py \
  eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --cases-dir eval/cases/focused/control-recovery \
  --recheck

python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Results:

| check | result |
| --- | --- |
| focused recheck | `passed_recheck: 82` |
| large recheck | `success: 0/6`; all failed rows have `large_disposition=closed_owned_failure` |
| broad sign-off | `status: pass` |
| `cargo fmt --check` | pass |
| `cargo test` | pass |
| `cargo build --release` | pass |
| `git diff --check` | pass |

## Large Row Disposition

| disposition | count |
| --- | ---: |
| `closed_owned_failure` | 6 |
| `implementation_blocker` | 0 |
| `accepted_external_limitation` | 0 |
| `split_forward` | 0 |

See `large_row_ledger.md` for row-level evidence.

## Design Review

- No hidden retry, implicit setup, provider/model branch, or verifier weakening
  was added.
- Runtime behavior was not changed; the new behavior is eval/report and
  sign-off interpretation only.
- Phase35 focused closure remains intact after recheck.
- Large sign-off now rejects missing or contradictory disposition data instead
  of silently treating attribution pass as task success.
- The implementation keys off failure families and evidence fields, not case
  id branches.

## Phase37 Handoff

Phase36 hands Phase37 a current large root whose failed rows are owned and
dispositioned. Phase37 still owns row-to-case proof reconciliation; it must not
infer adopted-row closure from the broad sign-off pass alone.
