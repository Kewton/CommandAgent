# Issue 399 A14-A13-3 Recovery smoke analysis

## 1. Scope and evidence

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run: `phase6-recovery-v4-20260830-a14-a13-3-smoke-01`
- Frozen product SHA: `f15c6535a71fd64c8e007631b4af603dab1e9c3d`
- Release binary SHA256: `2e6ef16d2ae4924ddff379f6c1d1f8d832bdeda4f19b0c636126d5b9fe5178bf`
- Exact-SHA CI: CI run `33307450372` and acceptance run `33307450379`, both `completed/success`
- Contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a13-3-contract.json`
- Report: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a13-3-smoke-01/recovery-report-v4.json`
- Report SHA256: `069239e3a17c51bd53b141a47878119fbca56dcb400c8fa9fbf6140b7f7ee307`
- Ledger SHA256: `a6922cd45eb461706550d116694eb05f4a1977bfbf14beda3a3b0dc7900adaa1`
- Ledger chain head: `da32d054f07adb9cdc564e471e8f5a378f72bff9198376a56a9ce708548c3cf6`

A13-2 is immutable historical evidence and was not rescored. A13-3 used a new run ID and the same report script.

## 2. Verdict

**GO for instrument readiness.** All 30 frozen checks are `true`; `instrument_ready` and `effect_attribution_ready` are both `true`.

This is not a population effect claim. The frozen report keeps `effect_claim_allowed: false` and `inference_role: instrument diagnostic only` because only one pair executed Recovery.

## 3. Observed outcomes

| Outcome | Count |
|---|---:|
| Attributed improved | 1 |
| Attributed harmed | 0 |
| No Recovery needed | 5 |
| No Recovery executed by preregistered exclusion | 4 |
| Unchanged pass/fail | 0 / 0 |
| Initial-attempt divergence | 0 |
| Unusable | 0 |

The one executed Recovery was `phase6-main-c05-task-05--pair-02`:

- frozen external oracle: `fail -> pass`;
- internal terminal: `failed_recoverable -> completed`;
- regression: pass before and after;
- existing artifact harmed: false;
- changed product path: `cli.py` plus host evidence records;
- added cost: 48,389 input tokens, 5,096 output tokens, 53,485 total tokens, and 141,610 ms wall time.

The executed treatment is below the draft full-experiment p50 budgets of 60,000 total tokens and 240,000 ms. One observation is insufficient to estimate p50 or p95, so the full experiment must still evaluate the frozen resource gates.

## 4. A13-2 defect and A13-3 effect

A13-2 failed only `recovery_fix_terminal_completion`: one c07 treatment was promoted after the registered command passed even though the product-visible completion contract remained incomplete.

A13-3 changes the common Recovery boundary as follows:

1. Read the existing profile/task completion contract before Recovery.
2. Bind missing obligation targets, such as test or acceptance-evidence paths, into the Recovery handoff.
3. After treatment, require both the exact registered final-success observation and the remaining completion contract to pass.
4. Count Recovery completion in the evaluator only when post-Recovery contract pass, promotion, and `recovery_succeeded` are all present.

The live smoke observed the intended result:

- all three c07 repetitions were external `pass` and internal `completed` on the shared initial attempt;
- all three executed zero Recoveries and were classified `no_recovery_needed`;
- `recovery_fix_terminal_completion` passed with zero violations;
- no treatment harm or regression was observed.

The protection also worked for `phase6-main-c05-task-05--pair-03`: the frozen external oracle already passed while the internal terminal was `failed`. The current-success boundary suppressed Recovery rather than mutating a passing artifact. This is recorded by `current_success_suppression_observed`.

## 5. Exclusion safety

- c06 dependency sentinel: one pair, `dependency_or_provisioning`, Recovery zero.
- c08 profile/contract sentinels: three pairs, `profile_or_completion_contract`, Recovery zero.
- Ineligible Recovery violations: zero.

Thus the run did not spend Recovery attempts on the preregistered dependency or profile-contract cases.

## 6. What is established and what is not

Established:

- the A13-3 instrument can distinguish initial success, recoverable failure, and preregistered non-Recovery cases;
- it prevents the observed c07 unnecessary-Recovery path;
- a Recovery treatment can be attributed to a frozen external `fail -> pass` transition with shared initial history;
- no harm, regression, unusable record, snapshot mismatch, or command-authority violation occurred in this smoke.

Not established:

- population-level success-rate improvement;
- a lower 95% confidence bound above zero;
- p50/p95 Recovery cost across heterogeneous tasks;
- safety beyond the frozen eligible population.

## 7. Next decision

The draft A14 full experiment is now technically eligible for freeze, but collection remains unauthorized. Its frozen design is 60 eligible pairs across 20 task clusters and 20 non-Recovery sentinels, with exactly 80 total pairs, 2,000 cluster-bootstrap samples, at least 30 actually executed Recoveries, zero harm/regression/unusable/sentinel Recovery, and the predeclared token/wall-time budgets.

Before starting that multi-hour run, freeze it against an exact product SHA and obtain explicit full-collection authorization. Do not change the 60+20 denominator, exclusions, minimum executed-Recovery count, or resource budgets after observing outcomes.
