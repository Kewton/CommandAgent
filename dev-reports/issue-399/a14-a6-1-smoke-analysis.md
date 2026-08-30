# A14-A6.1 typed reproducer smoke analysis

## Outcome

A14-A6.1 completed all three preregistered CLI pairs. The task registry loaded,
all selected corpus rows bound successfully, and every treatment arm emitted
the exact typed fix reproducer requested by the contract. The previous A14-A6
pre-execution failure is therefore corrected.

The run does **not** establish a Recovery success-rate effect. Recovery executed
in 0/3 pairs, `effect_attribution_ready` is false, and `effect_claim_allowed` is
false. All three pairs stopped at the product's Recovery preflight before a
treatment workspace was created.

## Working directories and exact build

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Clean source worktree: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a6-1-source`
- Adopted clean build target: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a6-1-exact-target`
- Rejected mixed-target build: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a6-1-final-target`
- Run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a6-1-smoke-01`
- Exact code SHA: `20176e3db08b744af421e399e79a563bd74130f1`
- Binary version: `commandagent 0.1.0 20176e3d 2026-08-30T11:29:34+09:00`
- Binary SHA-256: `3b30af9c2e9f05d2515c8b4b4d92c2a2de30b7caff0ae70cf634d9536f49c977`
- Exact-SHA CI and acceptance: completed successfully.

The rejected target reported `20176e3d+dirty` because it reused build state
from an accidentally started main-worktree build. It was not used for the
formal smoke and remains separate from the adopted exact target.

## Observed results

| Observation | Result |
|---|---:|
| Completed pairs | 3/3 |
| Report checks | 26/26 true |
| Typed fix reproducer binding | 3/3 |
| Frozen final oracle after initial arm | fail 3/3 |
| Recovery executions | 0/3 |
| Attributed improvement / harm | 0 / 0 |
| Treatment mutations | 0 paths |
| Median treatment resource delta | 0 tokens / 0 ms |

The typed commands were `python3 cli.py 7`, `python3 cli.py 11`, and
`python3 cli.py 16`. Each treatment record contains one
`fix_plan_synthesized` binding with
`r_basis=completion_contract:fix_reproducer_command`, one reproduce-before
step, one before-failure observation, and the exact registered command.

`instrument_ready: true` is limited to the contract's typed-binding diagnostic
role. The contract intentionally froze both minimum executed-Recovery pairs and
minimum current-success suppressions at zero. It is not a Recovery-effect GO.

## Failure mechanism

All three initial attempts ended with the same structured terminal condition:

`preflight unavailable: required_capability_has_no_product_visible_read_only_observation:input_output_contract`

The completion contract already contains the product-visible typed reproducer
and includes that exact command in `verify_commands`. Nevertheless,
`recovery_preflight` currently rejects every non-empty
`required_capabilities` list before it executes the registered read-only final
success commands. It does not distinguish a capability backed by a typed fix
reproducer from browser or other capabilities for which no safe product-visible
observation exists.

This blanket rejection is the direct cause of 0 Recovery executions. The root
cause is that typed fix-reproducer support was added to plan synthesis without
also adding the corresponding capability-to-read-only-observation binding to
Recovery preflight.

The A14-A6 design diagnostic did not expose this branch because its sampled
initial attempt completed successfully; automatic Recovery preflight was never
entered. The formal three-case smoke supplied the necessary failing initial
attempts and exposed the missing integration.

## Next correction

The product should allow `input_output_contract` through Recovery preflight only
when all of the following are true:

1. `fix_reproducer_command` is present and passed normal product command policy;
2. the normalized typed command is also present in `verify_commands`;
3. the full registered verify suite is executed read-only at the boundary;
4. every other required capability still has an independently supported
   product-visible observation, otherwise preflight remains unavailable.

Tests must cover current-success suppression, observed failure leading to
Recovery eligibility, missing typed binding, command mismatch, and continued
rejection of browser capability. This is a capability-contract correction, not
a profile-specific exception and not a weakening of verification.
