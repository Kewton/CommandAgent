# Phase37 Implementation Report

Date: 2026-06-24 JST

Status: completed

## Summary

Phase37 completed row-to-case proof reconciliation for the current eval case
set.

Implemented artifacts:

- `row_case_proof_matrix.md`
- `proof_gap_ledger.md`

Updated upstream Phase32 recovery files:

- `phase_32/recovery_task_ledger.md`
- `phase_32/followup_phase_split.md`

No runtime, provider, profile, minimal-loop, verifier, or sign-off code was
changed.

## Measured Inputs

| input | value |
| --- | --- |
| current smoke cases | 3 |
| current focused cases | 82 |
| current large cases | 6 |
| current total cases | 91 |
| focused recheck status | `passed_recheck=82` |
| large disposition status | `closed_owned_failure=6` |
| broad sign-off after Phase36 | `status=pass` |

## Closure Results

| gate | result |
| --- | --- |
| C01-C54 all represented in proof matrix | pass |
| C01-C45 adopted rows have current or accepted proof | pass |
| C46-C54 excluded rows have rationale | pass |
| 91 current cases mapped or supplemental | pass |
| 44 historically omitted current cases visible | pass |
| adopted rows closed only by omitted historical roots | none |
| open proof gaps | 0 |
| Phase38/39 handoff retained | pass |

## P32-R009 Decision

P32-R009 is closed by Phase37.

Reason:

- The proof matrix contains all C01-C54 rows.
- Current eval cases are grouped and bound to adopted rows or supplemental
  regression surfaces.
- Excluded rows have coverage-table rationale.
- No open `proof_gap` remains.

This does not declare migration completion. Root admission remains Phase38 and
final closure retry/reporting remains Phase39.

## Verification

Documentation-only verification:

```text
git diff --check
rg -n "[ \t]+$" workspace/mvp/logic/anvil/loadmap2/phase_37
```

Current root regression check:

```text
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816

status: pass
```

No code was changed, so Rust and eval-report test suites were not required by
the Phase37 plan.

## Design Review Result

Review findings applied during implementation:

- Used a row matrix instead of narrative closure so C01-C54 cannot be counted
  incompletely.
- Kept Phase38 root admission out of Phase37.
- Kept Phase39 final closure out of Phase37.
- Treated large failures as row-disposition evidence, not task success.
- Preserved current eval roots as authority for current cases and historical
  roots as regression evidence only.
- Avoided hidden retries, provider/model branches, implicit setup, and
  verifier weakening.
