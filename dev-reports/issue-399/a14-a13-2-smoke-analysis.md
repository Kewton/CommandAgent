# A14-A13-2 Recovery smoke analysis

## Outcome

A14-A13-2 completed all 10 frozen pairs. The same report script returned
instrument NO-GO with 29 of 30 checks passing. The run is immutable diagnostic
evidence and is not rescored or used for a population effect claim.

- Repository run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a13-2-smoke-01`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial/phase6-recovery-v4-20260830-a14-a13-2-smoke-01`
- Exact product/code SHA: `6a8b4743150cd3030110329c95406573538d4049`
- Report: `recovery-report-v4.json`

| Outcome | Count |
| --- | ---: |
| Attributed improved | 2 |
| Attributed harmed | 0 |
| No Recovery needed | 3 |
| No Recovery executed | 4 |
| Unchanged pass | 1 |
| Unchanged fail | 0 |
| Unusable | 0 |

The A13-2 eligibility amendment worked as intended. All three cell-08 pairs were
classified as `profile_or_completion_contract`, configured for zero Recovery,
and executed zero Recovery. The cell-06 dependency sentinel was likewise
classified ineligible and executed zero Recovery. No ineligible Recovery,
existing-artifact harm, or regression was observed.

Three eligible pairs executed one Recovery. The two cell-05 pairs changed from
external fail to pass. Cell-07 pair-01 remained external pass before and after
Recovery. Executed-Recovery overhead was 165,705 tokens and 444,523 ms in total:

| Pair | External transition | Added tokens | Added wall time |
| --- | --- | ---: | ---: |
| cell-05 pair-01 | fail to pass | 33,619 | 155,329 ms |
| cell-05 pair-03 | fail to pass | 77,673 | 133,678 ms |
| cell-07 pair-01 | pass to pass | 54,413 | 155,516 ms |

## Remaining instrument failure

Only `recovery_fix_terminal_completion` failed, for cell-07 pair-01. The frozen
external oracle already passed at the shared pre-Recovery boundary, but the
product's internal completion check reported a recoverable failure. Recovery
therefore executed and the registered final-success command passed, but the final
completion result remained partial:

- `completion_verify_passed: false`
- missing `bound_verify_command`
- missing `verification` obligation with target `tests/test_app.py`
- missing `acceptance_evidence` obligation with target `README.md`
- treatment promoted with reason `registered_final_success_passed`

The step-level Recovery handoff contained the failed step and registered verify
command, but omitted these completion-obligation targets. The generated Recovery
prompt correctly prohibited inventing README or test obligations unless they were
explicitly listed, so the model did not have a valid instruction to create them.
Promotion then checked only the registered command, not the complete product-visible
completion contract.

## A14-A13-3 amendment

A14-A13-3 keeps all thresholds, pair IDs, external oracles, exclusions, and the
maximum of one Recovery unchanged. It changes product behavior in two stricter
ways:

1. Before generating a Recovery plan, bind missing completion obligations and
   their concrete repair target paths into the typed handoff.
2. Before promoting a Recovery treatment, require both the registered final-success
   observation and the remaining product-visible completion contract to pass.
3. Treat Recovery completion as verified only when the post-Recovery contract pass,
   treatment promotion, and `recovery_succeeded` events are all present.

A successful execution of the exact registered fix reproducer may satisfy only
the `bound_verify_command` observation. It cannot waive missing capabilities,
obligations, weak evidence, or an inconclusive result. A treatment that still
fails the complete contract is rejected and the control snapshot is retained.
