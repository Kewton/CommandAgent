# Issue 399 Phase 6 main-v4 preregistered smoke

## Decision

Instrument readiness is **GO**. The preregistered smoke completed exactly 12 paired runs and 24 candidate lanes. All 19 frozen instrument checks in `main-smoke-report-v4.json` are `true`.

This is not an effectiveness result. Smoke records and responses are isolated from `phase6-main-v4-20260828-live-01` and must not be copied, rescored into, or used to change that run's thresholds, exclusions, denominator, corpus, or sampling rule.

## Frozen boundary

- Contract: `phase6-main-v4-20260828-live-01`
- Contract SHA-256: `fd99d58055ccd09177fb58aca54a22f8f8df97f2c4d58fdfeb2b1ab96a0126b8`
- Exact implementation code SHA: `4b46342c3848bc49d3675d5aad15e3422d54fae1`
- Exact-SHA CI and acceptance: completed/success
- Smoke request namespace: `phase6-main-v4-20260828-smoke-01`
- Selection: source task 01, run 01, once in each of the 12 frozen matrix cells
- Recovery Plan automatic executions: 0

## Results

| Item | Result |
|---|---:|
| Paired runs | 12 / 12 |
| Candidate lanes | 24 / 24 |
| Raw and canonical schema-valid lanes | 24 / 24 |
| Candidate oracles evaluated | 47 |
| Executable candidate oracles | 47 / 47 |
| Host repairs | 0 |
| Reference fallbacks | 0 |
| Gold used for execution | 0 |
| Shadow false-full | 0 |
| Baseline product runs discovered | 12 / 12 |
| Baseline honest terminals | 12 / 12 |
| Baseline failures retained | 12 / 12 |
| Missing baseline/candidate resource measurements | 0 |

All baselines returned nonzero, but every run was task-contract-bound, discovered, resource-measured, and recorded as an honest terminal. None was excluded or overridden by candidate evidence.

## Resource observation (not a smoke selection gate)

The 12-pair smoke projection exceeds all four preregistered budgets:

| Budget metric | Smoke observation | Frozen maximum |
|---|---:|---:|
| p50 wall-time increase | 45.628683% | 10% |
| p95 wall-time increase | 62.882868% | 20% |
| p50 total-token increase | 81.571333% | 10% |
| p95 total-token increase | 220.672098% | 20% |

Per the frozen contract, these smoke observations do not authorize stopping, redesigning, or filtering the main collection. The four budgets are hard GO gates only on the complete 360-pair primary-lane result. Missing measurements are never imputed or excluded.

## Integrity evidence

- `campaign-manifest.json` SHA-256: `d513059e3fc3b302c8bfeec63f06fa0d2aeda120113e62e14dcac72980910b2a`
- `campaign-summary.json` SHA-256: `5a516c2d7b6233ba7fad251e3d9221f7d59da9e1d493dc0c580b946feac07d25`
- `record-ledger.jsonl` SHA-256: `a85ab1d43c2b620ac72ddc6772dd0a9a63a34eae1d9d1e69b5b6a2cbd49bed94`
- Record-ledger head SHA-256: `1ea1638a727381e9ac4003d49d3704e79f83f7c71c4f9ed3d21531f8cac91dc9`
- `main-smoke-report-v4.json` SHA-256: `125a615c353072ca11674ff97be545f396bbafb7b6cadcde34c456c09bc94a39`
- Same-script replay: byte-identical, pre-annotation SHA-256 `f8af0615a38f6bf0692d9e5103ad0a371aaa157cc5a66e52d7d8f03e04a42ba7`

Raw records and execution workspaces remain untracked diagnostic evidence. The curated manifest, summary, ledger, report, and this analysis are the commit candidates.
