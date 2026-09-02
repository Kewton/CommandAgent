# Issue 399 Phase 6 A14-A11 smoke analysis

## 1. Scope and working directories

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Frozen contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a11-contract.json`
- Exact implementation SHA: `2764795ab5b53711b6b55d1ce8c2b13d65732f6d`
- Clean exact-SHA source: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a11-source`
- Exact-SHA release target: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a11-exact-target`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run evidence: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a11-smoke-01`

The A11 run is an instrument diagnostic with three preregistered shared-history pairs. It compares zero automatic Recovery executions with at most one execution. It does not authorize a population effect claim.

## 2. Frozen implementation and CI

- Exact-SHA `CI`: completed / success, run `33294431312`.
- Exact-SHA `acceptance`: completed / success, run `33294431313`.
- Release binary version: `commandagent 0.1.0 2764795a 2026-08-30T14:19:09+09:00`.
- Release binary sha256: `5bb6012ab25c721eb562ce8596df3891197ebccb18b7c52e4b44a1b674572e9a`.
- Contract sha256: `3e798c1a964e8068cbe1b811509842885e0b3a12d9373c395a34562d34e6e871`.

## 3. Result

The report passed all 29 instrument checks. `instrument_ready` and `effect_attribution_ready` are true, while `effect_claim_allowed` remains false by contract.

| Pair | Initial external oracle | Recovery executions | Final external oracle | Classification | Added cost |
|---|---:|---:|---:|---|---:|
| task-01 | pass | 0 | pass | `no_recovery_needed` | 0 tokens / 0 ms |
| task-05 | pass | 0 | pass | `no_recovery_needed` | 0 tokens / 0 ms |
| task-10 | fail | 1 | fail | `unchanged_fail` | 32,313 tokens / 104,897 ms |

Aggregate counts are improved 0, harmed 0, no-recovery-needed 2, unchanged-fail 1, and unusable 0. The median resource delta is zero because two of the three pairs correctly executed no Recovery.

## 4. Improvement that is established

### 4.1 Unnecessary Recovery was suppressed

Task-05 ended internally as failed but its frozen external final-success oracle already passed. The product-visible registered final-success checks also passed at the Recovery boundary, so automatic Recovery was suppressed. The control artifact was retained, changed paths were zero, and the incremental resource cost was zero.

This is the safety behavior sought after the earlier case where an internal unjustified NG initiated Recovery and damaged a correct artifact. It is a concrete improvement in Recovery precision, not an improvement in repair success rate.

### 4.2 Fix-contract continuity was recorded

Task-10 executed one Recovery and emitted exactly one `recovery_fix_contract_resumed` event. The event retained:

- original intent `fix`;
- contract origin `fix_intent_v0`;
- contract version/ref `v0` / `docs/fix-intent-contract.md`;
- the same fix run ID;
- reproducer `python3 cli.py 16`;
- host-owned source and `external_oracle_used: false`.

The registered inner Recovery command and fix-continuity gates both passed. Therefore A11 removed the A10 contract-origin drift without weakening the generic CLI profile or exposing the frozen external oracle to the product.

## 5. Success-rate improvement is not established

Task-10 remained external fail after one Recovery. This is not evidence that the LLM's repair was semantically unsuccessful: the Recovery treatment was rejected before promotion because the host phase state machine returned:

`invalid phase transition: state=FinalAcceptance { cycle: 0 }, observation=IntentFinalized`

The same-fix continuation and registered completion checks ran, and `completion_verify_passed` was true. The host then reported the resumed intent runtime with `IntentFinalized`, although a standard Recovery plan had placed the phase machine in `FinalAcceptance`, where the valid success observation is `AcceptancePassed`. The transaction consequently retained the control workspace. The external oracle observed the unchanged control artifact and remained fail.

Thus the direct cause of task-10's retained failure is a host wiring defect introduced by A11, not demonstrated LLM inability. No success-rate improvement may be claimed from A11.

## 6. A12 corrective action

The A11 run and report remain immutable and are not rescored. A12 will:

1. map a successful intent-owned runtime to `AcceptancePassed` only when the phase machine is already in `FinalAcceptance`;
2. preserve `IntentFinalized` for ordinary fix/investigation intent states;
3. place the selection logic in a new leaf module so guarded runner chokepoints do not grow beyond their budgets;
4. retain the exact fix contract, registered inner commands, transaction isolation, maximum one Recovery, frozen external post-execution oracle, and all resource/harm measurements;
5. rerun an exact-SHA diagnostic smoke under a new run ID.

Acceptance requires all instrument checks, one observed contract continuation, no invalid phase transition, a promotion decision based on the frozen product-visible checks, and truthful external-oracle classification. A successful external transition would be diagnostic evidence only, not a population effect claim.

## 7. Evidence

- `eval/goal_verify/v0/exact-sha-ci-2764795a.json`
- `eval/goal_verify/v0/phase6-recovery-v4-a14-a11-contract.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a11-smoke-01/record-ledger.jsonl`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a11-smoke-01/recovery-report-v4.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a11-smoke-01/raw/phase6-main-c05-task-05/pair-01.json`
- `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a11-smoke-01/raw/phase6-main-c05-task-10/pair-01.json`
