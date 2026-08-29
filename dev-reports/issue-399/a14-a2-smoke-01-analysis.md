# Issue 399 A14-A2 smoke-01 result and A14-A3 amendment

- Repository worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Implementation SHA: `97fa75fa33167f0f5dcc9b7f85efa4de96e789a5`
- Historical run: `phase6-recovery-v4-20260829-a14-a2-smoke-01`
- Historical verdict: **NO-GO** (`instrument_ready: false`)
- Policy: this run and its report are immutable and must not be rescored.

## What the smoke established

The collection completed all four preregistered cases. The exact-SHA `CI` and
`acceptance` workflows both completed successfully before collection, and the
binary reported `commandagent 0.1.0 97fa75fa` without a dirty suffix.

| case | automatic Recovery | external transition | interpretation |
| --- | ---: | --- | --- |
| c01 | 0 | pass; no treatment | Initial success was not miscounted as a Recovery improvement. |
| c04 | 1 | fail to fail | Shared boundary attribution worked; Recovery changed nine paths but did not make the frozen browser/HTTP oracle pass. |
| c06 | 0 | unusable | The dependency-unavailable case was preregistered outside Recovery and was not counted as pass, fail, or improvement. |
| c07 | 0 | pass; no treatment | The initial attempt fixed the reproducer, so the former unnecessary Recovery/harm path was not entered. |

For c04, the pre-Recovery snapshot matched the control, the failure handoff was
recorded, and the single Recovery added or changed nine paths. Its incremental
provider usage was 118,841 input tokens, 6,922 output tokens, 125,763 total
tokens, and 224,926 ms. The external outcome remained failed.

## NO-GO event

Eighteen of twenty report checks passed. These two checks failed:

- `final_success_oracle_semantics_validated`
- `fix_before_and_after_polarity_distinct`

The c01, c04, and c07 oracle semantics were valid. The sole invalid semantic
record was c06, for which `fix_precondition_oracle_missing` and
`fix_before_after_fixture_pair_missing` were recorded.

## Direct cause

The A14-A2 report appended every completed shared-boundary record to the
Recovery semantic-validation population. In the shared-run representation, a
preregistered exclusion is represented as a completed no-treatment record so
that its zero Recovery count and external `unusable` state remain auditable.
The report consequently applied a before/after Recovery requirement to c06
even though c06 was deliberately ineligible for Recovery.

The independent exclusion gate behaved correctly:
`ineligible_recovery_not_executed: true`. The NO-GO therefore came from mixing
the exclusion population with the Recovery-effect semantic population.

## Root cause

A14-A2 separated the runtime execution policy but did not apply the same
population boundary in report aggregation. The earlier report test represented
an excluded case as `recovery_one.status: not_run`; it did not cover the A14-A2
shared-run representation (`status: completed`, configured/executed Recovery
both zero). This left a representation-specific aggregation defect untested.

## A14-A3 correction

A14-A3 keeps both controls strict and separates their scopes:

1. `ineligible_recovery_not_executed` continues to require zero Recovery for
   preregistered dependency/capability exclusions.
2. Final-success and before/after polarity semantics are aggregated only for
   pairs eligible for a Recovery treatment.
3. Eligible initial-success pairs remain in semantic validation even when they
   correctly execute zero Recovery runs.
4. A focused regression test covers an excluded shared-run record with invalid
   fix polarity alongside an eligible attributed record.
5. A14-A2 smoke-01 remains NO-GO. A14-A3 uses a new contract and run ID and
   requires a new implementation SHA, exact-SHA CI, and clean binary.

This is a population-boundary correction, not a weakening of verification.
It prevents an honest exclusion from failing an unrelated treatment-semantic
gate while retaining the dedicated exclusion gate.

## Evidence

- Frozen contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a2-contract.json`
- Exact-SHA CI: `eval/goal_verify/v0/exact-sha-ci-97fa75fa.json`
- Historical report: `dev-reports/issue-399/runs/phase6-recovery-v4-20260829-a14-a2-smoke-01/recovery-report-v4.json`
- Raw records: `dev-reports/issue-399/runs/phase6-recovery-v4-20260829-a14-a2-smoke-01/raw/`
- Corrected report aggregation: `scripts/eval_lib/goal_verify_recovery_report_v4.py`
- Regression test: `tests/eval/test_goal_verify_main_v4.py`
- A14-A3 generator: `scripts/eval_lib/generate_goal_verify_recovery_v4_a14_a3.py`
