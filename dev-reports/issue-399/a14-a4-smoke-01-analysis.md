# Issue 399 A14-A4 Recovery smoke-01 analysis

## Scope and evidence boundary

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run: `phase6-recovery-v4-20260830-a14-a4-smoke-01`
- Exact code SHA: `c1209e63f2dbc560c5988c1ff40ea29f27f8dbd0`
- Exact-SHA CI: CI and acceptance both completed successfully.
- Ledger head: `51213f74e2b1d8549a44492189fbf0d89e5d2b3ab5d28ff02888db890e6a8b3c`

This run is immutable A14-A4 diagnostic evidence. Its original report remains
NO-GO and must not be overwritten or rescored into A14-A5 evidence.

## Result

All four preregistered pairs completed. The report had 25 checks, of which 22
passed and three failed:

1. `browser_oracle_executability_preflight`
2. `maximum_one_recovery_executed`
3. `recovery_arm_configured_one_or_preregistered_not_run`

Therefore `instrument_ready` and `effect_attribution_ready` are both false.
The run does not support a Recovery-effect claim.

Observed transition counts were `no_recovery_needed: 2` and
`no_recovery_executed: 2`. No Recovery treatment ran, so the observed
`attributed_improved: 0` and `attributed_harmed: 0` are safety/instrument
observations only, not evidence that Recovery is beneficial or harmless when
executed.

## What A14-A4 established

- c01: the product-visible registered postcheck already passed, so automatic
  Recovery was suppressed with `current_success_protected`. This prevents the
  previously observed internal false-NG from starting a destructive Recovery.
- c07: the initial result was classified as successful and Recovery did not run.
  The A14-A3 rollback/harm path was not reproduced.
- c06: the preregistered dependency/capability exclusion kept Recovery disabled.
- Transaction-control, handoff-fidelity, treatment-isolation, snapshot,
  resource, manifest, execution-action, and frozen-oracle checks passed.

## Why the three gates failed

The failures are measurement-instrument defects, not evidence that a Recovery
treatment failed:

- Browser executability was inferred from the candidate artifact. In c04 the
  candidate server was not ready, so a product outcome was incorrectly used as
  an oracle-instrument failure.
- c04 was preregistered with one Recovery run configured, but a runtime
  dependency/provisioning exclusion was discovered only after the shared run
  started. `configured=1, executed=0` is the correct record for that sequence;
  the report incorrectly required `configured=0`.
- The maximum-one-execution check reused configuration validity instead of
  checking `0 <= executed_recovery_runs <= 1` directly.

## A14-A5 correction

A14-A5 keeps A14-A4 raw evidence and its NO-GO verdict unchanged, while making
the following prospective-only instrument corrections:

1. Run browser executability preflight against a frozen reference workspace,
   never against the candidate artifact.
2. Use the registered `/play-01` route and accept the browser-normalized CSS
   value `rgb(0, 0, 255)` as semantically equivalent to `blue`.
3. Record `executed: true` when the registered browser subprocess actually ran.
4. Validate preflight contract/run identity, frozen-reference provenance,
   information boundary, reference build success, and browser outcome success.
5. Judge Recovery configuration from preregistered eligibility, runtime
   execution from the later eligibility result, and maximum execution count as
   an independent check.

The frozen-reference diagnostic build and both registered browser observations
passed before A14-A5 collection authorization. This diagnostic is an
instrument check only. A formal A14-A5 smoke still requires a new committed
exact SHA, successful exact-SHA CI, a frozen contract, and a new run ID.
