# Issue 399 A14-A3 smoke-01 analysis

- Repository worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Product execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Contract: `phase6-recovery-v4-20260829-a14-a3-live-01`
- Run: `phase6-recovery-v4-20260829-a14-a3-smoke-01`
- Exact implementation SHA: `9c6768831eba3e6daf9ca8c46bf03b2d2e3c7269`
- Binary: `commandagent 0.1.0 9c676883 2026-08-29T23:58:17+09:00`
- Instrument verdict: **GO**
- Recovery effect claim: **not allowed by this smoke**

## Result

All 20 readiness checks passed. In particular:

- all four target records completed;
- the executed Recovery count was at most one per record;
- the c06 preregistered dependency exclusion executed zero Recovery runs;
- both executed Recovery records matched their captured control snapshot;
- both failure handoffs were recorded;
- eligible oracle semantics and fix polarity were valid;
- initial success was not attributed to Recovery;
- changed paths and internal/external outcome matrices were recorded; and
- resource measurements were complete.

The transition counts were:

| transition | count |
| --- | ---: |
| attributed improvement | 0 |
| attributed harm | 1 |
| unchanged failure | 1 |
| initial success / no Recovery needed | 1 |
| preregistered exclusion / no Recovery executed | 1 |

This is a GO for the measurement instrument, not a GO for enabling more
automatic Recovery. The four-case diagnostic provides no population-level
effect estimate and the contract explicitly forbids an effect claim.

## Per-case observations

| case | Recovery | control external | treatment external | result |
| --- | ---: | --- | --- | --- |
| c01 | 0 | pass | not applicable | no Recovery needed |
| c04 | 1 | fail | fail | unchanged failure |
| c06 | 0 | unusable | not applicable | preregistered exclusion |
| c07 | 1 | pass | fail | harmed |

For c04, Recovery changed nine paths but did not satisfy the frozen browser and
HTTP oracles. Incremental Recovery usage was 150,573 input tokens, 9,642 output
tokens, 160,215 total tokens, and 313,881 ms.

For c07, Recovery changed only `app.py`. Incremental Recovery usage was 92,062
input tokens, 8,929 output tokens, 100,991 total tokens, and 269,188 ms.

Across all four records, the report median includes zero-treatment cases and
was 50,495.5 tokens and 134,594 ms. For operational decisions, the two executed
Recovery cases must also be read directly rather than relying on that median.

## Harm event

The captured c07 control already passed the final external oracle. It used:

```python
print(sum(item.get("amount", 0) for item in payload["items"]))
```

Recovery rewrote it to:

```python
print(sum(item["amount"] for item in payload["items"]))
```

The frozen fixture contains an item without `amount`, so the rewrite restored
the `KeyError` failure. The same final-success command changed from pass to
fail, while `app.py` was the only changed source path.

The product-internal/external matrix makes the trigger visible:

- control: internal `failed_recoverable`, external `pass`;
- treatment: internal `failed`, external `fail`.

During Recovery, the generated plan included a `reproduce-crash` verification
step that expected the command to fail. Because the pre-Recovery workspace was
already fixed, the command passed. Bounded repair then rewrote `app.py` until
the historical failure was reproduced. A historical before-failure condition
was incorrectly used as a current treatment obligation.

## Problem and root cause

The remaining product problem is not that Recovery lacks enough retries.
Recovery was allowed to mutate a workspace whose registered final behavior was
already passing, and its plan treated historical failure reproduction as a
post-fix target. Increasing the retry count would give that incorrect objective
more opportunities and cost; it would not correct the objective.

The root contract mismatch has two parts:

1. the automatic Recovery decision consumes the internal incomplete/failure
   state without a read-only current-success safety check; and
2. the Recovery plan does not distinguish immutable historical precondition
   evidence from post-Recovery final-success obligations.

## Recommended A14-A4 safeguards

1. Add a read-only pre-Recovery safety gate over registered, candidate-visible
   final observations. If they currently pass while internal completion says
   recoverable failure, suppress automatic mutation and emit an explicit
   divergence event. Do not silently convert the run to success.
2. Keep historical failing reproducers as evidence only. A Recovery treatment
   must never require a currently fixed workspace to re-enter the historical
   failure state.
3. After Recovery, rerun the same registered observations against the captured
   control and treatment. On pass-to-fail, classify harm and restore or retain
   the captured control as the deliverable through a separately authorized,
   transactional rollback design.
4. Add a focused corpus fixture reproducing c07: control command passes,
   internal state requests Recovery, and the proposed Recovery plan contains an
   expected-failure reproduction step. The test must prove zero destructive
   mutation or a verified rollback.
5. Pre-register a strict incremental Recovery token/wall budget. The observed
   100,991 and 160,215 token costs show that more retries must not be enabled
   before the safety gate is effective.
6. Repeat only a small A14-A4 smoke. Do not start a 360-pair Recovery experiment
   and do not increase automatic Recovery beyond one until no pass-to-fail harm
   is observed under the frozen safeguards.

## Evidence

- Exact-SHA CI: `eval/goal_verify/v0/exact-sha-ci-9c676883.json`
- Frozen contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a3-contract.json`
- Report: `dev-reports/issue-399/runs/phase6-recovery-v4-20260829-a14-a3-smoke-01/recovery-report-v4.json`
- Raw c07 record: `dev-reports/issue-399/runs/phase6-recovery-v4-20260829-a14-a3-smoke-01/raw/phase6-main-c07-task-01/pair-01.json`
- Product event stream and boundary snapshot remain under the SSD execution
  root recorded above.
