# Phase39 Decision Evidence Matrix

Date: 2026-06-24 JST

Status: completed

## Final Decision

```text
migration_complete_with_explicit_exclusions
```

This decision is limited to the accepted Anvil control/recovery responsibility
surface in CommandAgent. It does not claim that every large application
generation task now succeeds.

## Current Proof Roots

| family | root | admitted cases |
| --- | --- | ---: |
| smoke | `eval/runs/current-all-local-llm/smoke/20260623T203030` | 3 |
| focused | `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236` | 82 |
| large | `eval/runs/current-all-local-llm/large/20260623T204816` | 6 |

Current case coverage: `91/91`.

## Evidence Matrix

| gate | evidence | command | observed result | status | decision effect |
| --- | --- | --- | --- | --- | --- |
| Coverage C01-C45 implemented/proven | `docs/eval/legacy-control-stack-coverage-20260621.md`; Phase22-Phase29 reports; Phase31 large proof | row-specific unit, focused fixture, and sign-off commands recorded in phase reports | 45 implemented rows; no adopted `Partial` or `Missing` row remains | pass | Allows accepted migration surface to close. |
| Coverage C46-C54 explicit exclusions | coverage table; Phase30 report; final decision report | n/a | 9 excluded rows with design rationale | pass | Requires final decision to be `migration_complete_with_explicit_exclusions`, not plain `migration_complete`. |
| Phase32 item 1: current roots cover current case set | `phase_38/root_admission_report.md` | `python3 scripts/eval_signoff.py --require-recheck --root smoke=... --root focused=... --root large=...` | `root_admission_status: pass`; `current_case_coverage: 91/91` | pass | Historical 47-case roots no longer drive final closure. |
| Phase32 item 2: current broad sign-off exits zero | current sign-off output; `phase_38/root_admission_report.md` | same current sign-off command | `status: pass` | pass | No current unowned sign-off finding remains. |
| Phase32 item 3: current focused assertions close | current focused `recheck_summary.tsv`; `phase_35/implementation_report.md` | `python3 scripts/eval_report.py ... --recheck` as recorded by Phase35 | 82 focused rows report `passed_recheck` | pass | Focused blockers no longer hold final closure open. |
| Phase32 item 4: current large failures are owned | current large `recheck_summary.tsv`; `phase_36/large_row_ledger.md`; `phase_36/implementation_report.md` | current large recheck as recorded by Phase36 | 6 rows are `closed_owned_failure`; no `accepted_external_limitation` is used | pass | Large task failures remain failed tasks, but they are not unowned migration blockers. |
| Phase32 item 5: no adopted row depends only on omitted historical roots | `phase_37/row_case_proof_matrix.md`; `phase_37/proof_gap_ledger.md` | Phase37 proof reconciliation | C01-C54 represented; 91 current cases mapped or supplemental; open proof gaps 0 | pass | Current-case proof replaces stale historical-only proof. |
| Phase32 item 6: final current decision report is written | `docs/eval/loadmap2-final-migration-decision-20260624.md`; this matrix; `phase_39/final_closure_report.md` | documentation/evidence review | report states exactly one current decision | pass | Closes Phase39 final closure retry/reporting. |
| Root admission stale/duplicate protection | `phase_38/root_admission_report.md`; `tests/test_eval_signoff.py` | duplicate focused-root negative sign-off command | duplicate root path fails admission before interpretation | pass | Prevents a smaller or duplicated proof bundle from being mistaken for final proof. |
| External limitation transparency | `phase_36/large_row_ledger.md`; current large recheck | n/a | no row is closed by external limitation | pass | Final decision does not hide missing functionality behind provider/model limits. |
| Runtime boundary preservation | Phase39 diff; plan files | `git diff --check` and review | docs/eval reporting only; no runtime/provider/minimal-loop/profile/repair change | pass | Final closure does not add hidden orchestration or weaken guards. |

## Review Result

All final gates pass under the admitted current roots. Because C46-C54 remain
explicitly excluded by design, the valid final state is
`migration_complete_with_explicit_exclusions`.
