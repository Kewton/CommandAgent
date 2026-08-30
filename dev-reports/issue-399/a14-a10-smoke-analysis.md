# A14-A10 Recovery inner binding smoke analysis

## Scope and immutable inputs

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a10-smoke-01`
- Contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a10-contract.json`
  - contract ID: `phase6-recovery-v4-20260830-a14-a10-live-01`
  - SHA-256: `c5bb58f1e472ec88918611f6c7e032c8936f2ea4b257a99bd4ad99f5ef9be4d7`
- Product source SHA: `070b5150a3f9c587be2efaf07567468c77d891fb`
- Exact binary: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a10-exact-target/release/commandagent`
  - version: `commandagent 0.1.0 070b5150 2026-08-30T13:32:53+09:00`
  - SHA-256: `801d2435f6dfac16d876236cc76545f1e21749eb4bb28b78ad78143938df4f94`
- Exact-SHA CI: CI run `33292737910` and acceptance run `33292737893` both
  completed successfully. Evidence is
  `eval/goal_verify/v0/exact-sha-ci-070b5150.json` (SHA-256
  `c946a35bcc2d02a8f9f01401c40a6b1549070ad5620757c27d0fedbd7e0454af`).

This is a three-pair instrument diagnostic. It is not an effect estimate and does not
authorize increasing the automatic Recovery count beyond one.

## Frozen report result

The preregistered report script completed successfully. All 28 instrument checks are true,
including `registered_inner_recovery_verify_commands`. The report is
`recovery-report-v4.json` (SHA-256
`7d4c4637b470ffdb559721ae3e72144aa70c9861012a99c5605923adef3d204e`).

- `instrument_ready: true`
- `effect_attribution_ready: true`
- `effect_claim_allowed: false`
- no Recovery needed: 2
- unchanged fail: 1
- attributed improved / harmed: 0 / 0
- unusable: 0
- executed Recovery pairs: 1
- median Recovery increment across all three pairs: 0 tokens and 0 ms
- task-05 Recovery increment: 52,422 tokens and 126,278 ms

The zero median is a consequence of two initial-success pairs executing no Recovery. It must
not be interpreted as zero Recovery cost.

## Pair-level observations

| Pair | Recovery | External transition | Product transition | Recovery-only retained change | Finding |
| --- | ---: | --- | --- | ---: | --- |
| task-01 | 0 | pass -> pass | completed -> completed | 0 | Initial success remained successful and Recovery was suppressed. |
| task-05 | 1 | fail -> fail | failed_recoverable -> failed | 0 | Inner commands were correctly bound, but Recovery final acceptance switched to the generic CLI profile contract and rejected the treatment because `cli/main.py` was absent. |
| task-10 | 0 | pass -> pass | completed -> completed | 0 | The new pytest evidence classification removed the A9 internal false NG. Initial execution completed, both frozen external oracles passed, and Recovery was correctly suppressed. |

No historical record was rescored. No failed treatment was promoted, and no retained
artifact was harmed.

## Confirmed improvements

### 1. Recovery StepPlan verification is now contract-bound

In task-05, the model proposed stateful or otherwise unregistered commands including
`exit_code=$?`, `[ $exit_code -eq 2 ]`, and `echo $?`. The host recorded three
`recovery_step_plan_verify_commands_bound` events:

- `inspect-current-state`: `read_only_inspection`; all verify steps removed;
- `repair-unknown`: `completion_contract_final_success`; all model verify steps replaced;
- `verify-recovery`: `completion_contract_final_success`; the final check was the complete
  registered three-command set.

The malformed commands were visible in `original_verify_commands` but absent from
`bound_verify_commands`. This directly closes the A9 inner-StepPlan escape path without
weakening final verification.

### 2. Inspection no longer triggers repair through a negative check

The inspection phase retained only inspection steps. Its model-proposed reproducer and
regression verifies were removed before execution. This prevents a deliberately failing or
semantically inverted check from opening a mutation path during read-only inspection.

### 3. Pytest evidence is accepted when it is structurally grounded

Task-10 completed on its initial attempt. Product final acceptance was `ok: true`, the exact
reproducer returned exit 0 with stdout `987`, and the frozen pytest and contract-check
regressions both passed. A9 executed one Recovery and ultimately rejected this task; A10
required zero Recovery. This is a product false-NG reduction caused by the evidence
classifier fix, not an attributed Recovery effect.

## Remaining failure isolated by A10

Task-05 passed the newly bound Recovery phase checks but failed final acceptance with:

`profile_behavior_probe_error: CLI entry is not an accessible workspace file`

The task's product-visible CompletionContract requires `cli.py`, while the generic Python CLI
profile manifest fixes its behavior probe entry to `cli/main.py`. More importantly, initial
fix execution is judged under `contract_origin: fix_intent_v0`, whereas Recovery final
acceptance is judged under `contract_origin: initial`. The Recovery path therefore changes
the final-acceptance contract and activates a profile probe that was not the initial fix
verdict's success condition.

This is not evidence that the registered repair commands failed. It is a contract-continuity
defect across the Recovery boundary. The treatment remained isolated and was rejected, so
the defect caused an unjustified retained failure rather than an unsafe promotion.

## Next implementation direction

1. Preserve the initial task intent and final-acceptance contract identity across automatic
   Recovery; do not silently replace `fix_intent_v0` with the generic profile contract.
2. Keep the external CompletionContract available for command provenance, but do not let it
   introduce profile obligations outside the initial acceptance estimand.
3. Record initial and Recovery final-acceptance contract origin/ref/version in a continuity
   event and add a reporter gate requiring equality for an executed Recovery.
4. Add focused tests for a root-level `cli.py` fix task and a conventional
   `cli/main.py` create task. The former must not be forced into the latter's profile layout;
   the latter must retain the existing strict behavior probe.
5. Re-run the same three-pair diagnostic with Recovery fixed at 0 versus 1. Success promotion
   still requires the registered internal commands and frozen external final-success and
   regression oracles.

The next amendment must not disable the CLI behavior probe globally, weaken the evidence
gate, or reinterpret this smoke as a Recovery success-rate estimate.
