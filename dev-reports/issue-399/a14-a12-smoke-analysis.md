# Issue 399 Phase 6 A14-A12 smoke analysis

## 1. Scope and working directories

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Frozen contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a12-contract.json`
- Exact implementation SHA: `2139f3ebefc8d408947ccccbedca96c36d24bee0`
- Clean exact-SHA source: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a12-source`
- Exact-SHA release target: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a12-exact-target`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run evidence: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a12-smoke-01`

A12 is a three-pair instrument diagnostic. Each pair uses one physical initial attempt and compares its captured pre-Recovery control with a treatment that may execute at most one automatic Recovery Plan. The frozen external oracle runs after product execution and is not visible to CommandAgent or Recovery.

## 2. Exact-SHA evidence

- Exact-SHA `CI`: completed / success, run `33295675581`.
- Exact-SHA `acceptance`: completed / success, run `33295675588`.
- Release binary version: `commandagent 0.1.0 2139f3eb 2026-08-30T14:53:21+09:00`.
- Release binary sha256: `f5c720ef559526c4f3e4a38a0949e6aa08ecf839eaab7e0aceee1fcd6b8c8f91`.
- Frozen contract sha256: `0c13d631a1b276e4550f2c77586c1bf337f78199903a9d682ff067fe362ffb72`.
- Final report sha256: `bd6e16ac6599329ea3c4f7cea26e6de846639ec296edec81a976c1213bc41afc`.

## 3. Instrument result

All 30 checks passed. `instrument_ready` and `effect_attribution_ready` are true. `effect_claim_allowed` remains false by the frozen contract.

| Pair | Initial external | Recovery count | Final external | Classification | Harm | Regression | Resource delta |
|---|---:|---:|---:|---|---:|---:|---:|
| task-01 | pass | 0 | pass | `no_recovery_needed` | false | false | 0 tokens / 0 ms |
| task-05 | fail | 1 | pass | `improved` | false | false | 47,107 tokens / 196,148 ms |
| task-10 | pass | 0 | pass | `no_recovery_needed` | false | false | 0 tokens / 0 ms |

Counts are improved 1, harmed 0, no-recovery-needed 2, unchanged-fail 0, unusable 0, and initial-attempt-divergence 0. The median resource delta is zero because two pairs correctly suppressed Recovery.

## 4. Established improvement

### 4.1 One frozen external failure changed to success

Task-05 is valid within-run paired evidence:

- Control final-success oracle: executed, exit 2, `observation_mismatch`, fail.
- Treatment final-success oracle: executed, exit 0, stdout `89`, `observation_match`, pass.
- Control regressions: pytest and contract check both pass.
- Treatment regressions: pytest and contract check both pass.
- Internal outcome: `failed_recoverable` to `completed`.
- Transaction decision: exactly one `promoted` decision with reason `registered_final_success_passed`.

This is the first A14 smoke observation where one automatic Recovery execution changes a frozen external final-success result from fail to pass while preserving the registered regression set. It is not based on model self-report.

### 4.2 Recovery changed the intended artifact without collateral harm

The promoted treatment changed `cli.py` and the associated fix adjudication evidence, and added after/regression evidence. Four artifact paths changed in total. `existing_artifact_harmed` and `regression_introduced` are both false.

### 4.3 Unnecessary Recovery remained suppressed

Task-01 and task-10 already passed the frozen external final-success oracle. Both executed zero Recovery runs and had zero incremental token and wall-time cost. Task-01 was internally failed, so this also reproduces protection against an internal unjustified NG initiating a destructive Recovery.

## 5. A11 host defect is resolved

The task-05 treatment emitted one valid `recovery_fix_contract_resumed` event with the same fix run ID and reproducer `python3 cli.py 11`. It then satisfied all A12 terminal conditions:

- result status `completed` and process return code 0;
- product completion verification passed;
- terminal status `completed` with `ok: true`;
- Recovery attempt status `succeeded` and stop reason `recovery_succeeded`;
- exactly one promoted transaction.

The A11 failure `state=FinalAcceptance, observation=IntentFinalized` did not recur. The new `recovery_fix_terminal_completion` gate passed, and its diagnostics are empty.

## 6. Resource effect

For the single executed Recovery pair, incremental usage was:

- input tokens: 40,540;
- output tokens: 6,567;
- total tokens: 47,107;
- wall time: 196,148 ms (about 3 minutes 16 seconds).

These costs are material. The result supports keeping automatic Recovery bounded to one execution and suppressing it when registered current-success observations already pass. It does not support increasing the maximum run count beyond one.

## 7. Limits of the conclusion

The A12 result establishes a concrete conditional improvement, not a population success-rate improvement:

- only three preregistered smoke pairs were run;
- only one pair executed Recovery;
- initial artifact generation is stochastic, so task identities cannot be compared across A11 and A12 as if they were the same initial attempt;
- the contract explicitly forbids an effect claim from this diagnostic run.

The defensible conclusion is: the repaired mechanism can suppress unnecessary Recovery and can convert at least one genuine external failure to success without observed harm. A larger preregistered paired sample is required to estimate the success-rate lift and its confidence interval.

## 8. Evidence

- `eval/goal_verify/v0/exact-sha-ci-2139f3eb.json`
- `eval/goal_verify/v0/phase6-recovery-v4-a14-a12-contract.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a12-smoke-01/record-ledger.jsonl`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a12-smoke-01/recovery-report-v4.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a12-smoke-01/raw/phase6-main-c05-task-01/pair-01.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a12-smoke-01/raw/phase6-main-c05-task-05/pair-01.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a12-smoke-01/raw/phase6-main-c05-task-10/pair-01.json`
