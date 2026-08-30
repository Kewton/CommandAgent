# Issue 399 A14-A5 Recovery smoke-01 analysis

## Scope and evidence boundary

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run: `phase6-recovery-v4-20260830-a14-a5-smoke-01`
- Exact code SHA: `d985fa10da1d2834ed72f29fe91dd6694ca9eb24`
- Binary: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a5-final-target/release/commandagent`
- Binary SHA-256: `8e5940b66f501d2d75e9a60e744cb01f5b24eca10078f6945916c1ea2ae5c703`
- CI: <https://github.com/Kewton/CommandAgent/actions/runs/33284200010>
- acceptance: <https://github.com/Kewton/CommandAgent/actions/runs/33284200103>
- Ledger head: `42ddd0f6f35ff2183cc7f4b1269ef3656183cd5aa14f2d7335c6fbd3b2e446ec`

CI and acceptance both completed successfully for the exact SHA. The contract,
runner sources, frozen inputs, SSD execution root, and clean binary passed the
pre-run frozen-input check.

## Result

All four preregistered pairs completed. The same frozen report script produced
25/25 true checks and exited zero:

- `instrument_ready: true`
- `effect_attribution_ready: false`
- `effect_claim_allowed: false`

The GO is limited to the A14-A5 measurement-instrument corrections. It is not a
GO for a Recovery success-rate claim or for increasing the retry count.

## Browser-oracle separation

The candidate-independent frozen reference preflight completed before pair
collection:

- registered Next.js 16.3.1 reference build: pass
- `ui-copy-text` browser observation: `executed: true`, pass
- `ui-style-background` browser observation: `executed: true`, pass
- observed heading: `開始`
- observed computed background: `rgb(0, 0, 255)`

The preflight record identifies the contract and run, records
`source: frozen_reference_workspace`, and records
`passed_to_product_or_recovery: false`. Candidate c04 server readiness therefore
remains a product outcome and no longer invalidates oracle executability.

## Pair outcomes

| Pair | Preregistered category | Runtime category | Configured | Executed | Stop reason | Transition |
| --- | --- | --- | ---: | ---: | --- | --- |
| c01 | recoverable candidate | dependency/provisioning | 1 | 0 | current success protected | no recovery needed |
| c04 | recoverable candidate | dependency/provisioning | 1 | 0 | preflight unavailable | no recovery executed |
| c06 | dependency/provisioning | dependency/provisioning | 0 | 0 | disabled | no recovery executed |
| c07 | recoverable candidate | initial success | 1 | 0 | initial success | no recovery needed |

Counts were `no_recovery_needed: 2`, `no_recovery_executed: 2`,
`attributed_improved: 0`, and `attributed_harmed: 0`. No treatment was executed,
so resource deltas were zero and no before/after treatment effect exists.

## What was established

- A preregistered configured-one pair may honestly become runtime-ineligible and
  finish with executed zero without failing the configuration gate.
- The maximum-one-execution gate independently checks the executed count.
- An already successful deliverable is not mutated merely because internal
  completion evidence is incomplete.
- Dependency/capability failures remain outside the Recovery treatment.
- A candidate server failure is not used as proof that the frozen browser
  oracle itself is unavailable.
- No snapshot, manifest, resource, action, information-boundary, or frozen
  oracle-source violation was observed.

## What remains unproven

None of the four pairs executed a Recovery treatment. Consequently this run
does not test, on a live treatment, whether:

- a Recovery Plan repairs a genuinely recoverable failure;
- isolated treatment promotion preserves a passing control artifact;
- the handoff carries the correct failed step and verification commands;
- one Recovery attempt improves external-oracle success probability; or
- one Recovery attempt has acceptable token and wall-time cost.

The true statement is therefore “A14-A5 instrument corrections are ready for a
live-treatment smoke,” not “Recovery improves success.”

## Required next step

Create a prospective A14-A6 smoke with at least one frozen, known-recoverable
case that meets all of the following before collection:

1. initial frozen external oracle fails for an intentional, local artifact bug;
2. dependency, browser capability, policy, sandbox, and missing-information
   failures are excluded;
3. the product-visible read-only preflight is executable and reports the same
   failure;
4. exactly one automatic Recovery attempt is configured and expected to run;
5. success and regression are judged only by frozen external oracles;
6. treatment changed paths, promotion/rollback, token use, and wall time are
   recorded;
7. a passing-control fixture is retained to detect damage;
8. smoke inference remains diagnostic until a non-zero executed-treatment count
   and all attribution gates are observed.

A14-A5 evidence must remain immutable and must not be rescored as A14-A6
evidence.
