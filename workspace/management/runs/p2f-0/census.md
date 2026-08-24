# P2F-0 census and pre-registration

> Generated evidence. Do not edit by hand.
> No repair measurement had started when this declaration was recorded.
> Regenerate with: `python3 workspace/management/scripts/p2f_campaign.py declare --recorded-at 2026-08-05T00:30:41+09:00`

## Scope lock

P2F-0 respects the F-BoN-V NO-GO: it adds no automatic BoN-to-repair connection and no repair wiring. Each selected failed workspace is copied, then its own already-saved recovery UltraPlan is invoked exactly once via `--run-ultra-plan`; the source workspace is not mutated and no directive is injected.
Execution copies are preassigned under `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0805_p2f_0/p2f-0-20260805-003041`; an existing destination fails closed.

Execution is pinned before spend to revision `80df5e39e1a0fb39cf9f1f4d5be6de31395f63eb` and binary SHA-256 `5998a1f53dfb74bbff9164fec8bed1f3254aad6046ba6a2691132b5ccf36cccd`. Production-path and all seven existing band byte hashes are pinned in `predeclaration.json`.

## Population census

The population is the complete formally failed inventory from bon0-001/002r/003r/004r, bon-local-001, and luna-006/007/008. Full runs are outside the failed population (4 excluded); failed census n=44. Workspace existence is 44/44; saved fix-continuation eligibility is 44/44. The fixed recovery-circle YAMLs are data-profile workflows, so none are applicable to this CLI/Next.js census.

| selected | census id | profile/family | workspace | failure class | failure stratum | score | score band | route |
|---|---|---|---|---|---|---:|---|---|
| no | `bon-local-001/breakout_local_bon_001` | nextjs/breakout | exists | `restart_or_recoverable_state_evidence_missing` | `nextjs_evidence` | not reached | `unreached` | fix continuation |
| no | `bon-local-001/breakout_local_bon_002` | nextjs/breakout | exists | `contract_instrumentation_missing:restart` | `nextjs_evidence` | not reached | `unreached` | fix continuation |
| no | `bon-local-001/breakout_local_bon_003` | nextjs/breakout | exists | `restart_or_recoverable_state_evidence_missing` | `nextjs_evidence` | not reached | `unreached` | fix continuation |
| yes | `bon-local-001/breakout_local_bon_004` | nextjs/breakout | exists | `restart_or_recoverable_state_evidence_missing` | `nextjs_evidence` | not reached | `unreached` | fix continuation |
| no | `bon-local-001/breakout_local_bon_006` | nextjs/breakout | exists | `restart_or_recoverable_state_evidence_missing` | `nextjs_evidence` | not reached | `unreached` | fix continuation |
| yes | `bon0-001/filter_bon0_001` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 62.5 | `mid:37.5-<75` | fix continuation |
| yes | `bon0-001/filter_bon0_002` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 25 | `low:<37.5` | fix continuation |
| no | `bon0-001/filter_bon0_003` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 25 | `low:<37.5` | fix continuation |
| yes | `bon0-001/filter_bon0_004` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `bon0-001/filter_bon0_006` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 25 | `low:<37.5` | fix continuation |
| yes | `bon0-002r/filter_bon0_001` | cli/filter | exists | `phase_failure:draft-readme` | `phase_verification` | not reached | `unreached` | fix continuation |
| no | `bon0-002r/filter_bon0_002` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 25 | `low:<37.5` | fix continuation |
| no | `bon0-002r/filter_bon0_003` | cli/filter | exists | `phase_failure:create-sample-data` | `phase_verification` | not reached | `unreached` | fix continuation |
| no | `bon0-002r/filter_bon0_004` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 25 | `low:<37.5` | fix continuation |
| no | `bon0-002r/filter_bon0_005` | cli/filter | exists | `phase_failure:implement-cli-tool` | `phase_verification` | not reached | `unreached` | fix continuation |
| yes | `bon0-002r/filter_bon0_006` | cli/filter | exists | `help_binding_failure` | `other_acceptance` | 37.5 | `mid:37.5-<75` | fix continuation |
| no | `bon0-003r/filter_bon0_001` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| yes | `bon0-003r/filter_bon0_002` | cli/filter | exists | `profile_behavior_probe_error` | `profile_probe` | not reached | `unreached` | fix continuation |
| no | `bon0-003r/filter_bon0_003` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `bon0-003r/filter_bon0_004` | cli/filter | exists | `profile_behavior_probe_error` | `profile_probe` | not reached | `unreached` | fix continuation |
| no | `bon0-003r/filter_bon0_005` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 25 | `low:<37.5` | fix continuation |
| yes | `bon0-003r/filter_bon0_006` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| yes | `bon0-004r/filter_bon0_001` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 25 | `low:<37.5` | fix continuation |
| no | `bon0-004r/filter_bon0_002` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `bon0-004r/filter_bon0_003` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | -12.5 | `low:<37.5` | fix continuation |
| no | `bon0-004r/filter_bon0_004` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `bon0-004r/filter_bon0_005` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 25 | `low:<37.5` | fix continuation |
| no | `bon0-004r/filter_bon0_006` | cli/filter | exists | `phase_failure:implement-cli-tool` | `phase_verification` | not reached | `unreached` | fix continuation |
| no | `luna-006/filter_luna_001` | cli/filter | exists | `cli_output_claims:observed_stdout_mismatch` | `cli_claim_binding` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `luna-006/filter_luna_002` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 0 | `low:<37.5` | fix continuation |
| no | `luna-006/filter_luna_003` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `luna-006/stats_luna_001` | cli/stats | exists | `phase_failure:create-documentation` | `phase_verification` | not reached | `unreached` | fix continuation |
| yes | `luna-006/stats_luna_002` | cli/stats | exists | `profile_behavior_evidence_failed` | `profile_probe` | 75 | `high:75-<100` | fix continuation |
| no | `luna-006/stats_luna_003` | cli/stats | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `luna-007/filter_luna_002` | cli/filter | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `luna-007/filter_luna_003` | cli/filter | exists | `phase_failure:final-verification` | `phase_verification` | not reached | `unreached` | fix continuation |
| no | `luna-007/stats_luna_001` | cli/stats | exists | `profile_behavior_probe_error` | `profile_probe` | not reached | `unreached` | fix continuation |
| no | `luna-007/stats_luna_002` | cli/stats | exists | `profile_behavior_probe_error` | `profile_probe` | not reached | `unreached` | fix continuation |
| no | `luna-007/stats_luna_003` | cli/stats | exists | `profile_behavior_probe_error` | `profile_probe` | not reached | `unreached` | fix continuation |
| no | `luna-008/filter_luna_001` | cli/filter | exists | `phase_failure:implement-cli-tool` | `phase_verification` | not reached | `unreached` | fix continuation |
| no | `luna-008/filter_luna_002` | cli/filter | exists | `profile_behavior_probe_error` | `profile_probe` | not reached | `unreached` | fix continuation |
| no | `luna-008/filter_luna_003` | cli/filter | exists | `final_acceptance_repair:path_missing` | `other_acceptance` | 37.5 | `mid:37.5-<75` | fix continuation |
| no | `luna-008/stats_luna_001` | cli/stats | exists | `cli_probe_polarity_violation` | `cli_polarity` | 62.5 | `mid:37.5-<75` | fix continuation |
| no | `luna-008/stats_luna_003` | cli/stats | exists | `phase_failure:implement-cli-tool` | `phase_verification` | not reached | `unreached` | fix continuation |

Every row's absolute source workspace, recovery-plan path and SHA-256, source-meta SHA-256, original time/cost, and route reason are retained in `predeclaration.json`.

## Stratified sample pre-registration

Fixed seed: `p2f-0-stratified-v1`. Rule: allocate one to every non-empty failure-stratum x score-band cell; allocate remaining slots one at a time to the cell with the largest unselected count (lexical cell tie-break); within a cell rank by SHA-256(seed + NUL + census_id), then census_id. This produces n=10, within the declared 10 +/- 2 range. The rule was committed before any copied workspace or repair run existed.

| order | census id | failure stratum | starting score band | recovery plan SHA-256 |
|---:|---|---|---|---|
| 1 | `bon-local-001/breakout_local_bon_004` | `nextjs_evidence` | `unreached` | `642488797a6d5e2d2c7d486ea743ccad00e749ad22183e1cfe53e27c50beafc0` |
| 2 | `bon0-001/filter_bon0_001` | `cli_claim_binding` | `mid:37.5-<75` | `cc9d5be84a34bc5dd43fea23e9a9794455dfc88b3cd6b9e973aa121431ebaa68` |
| 3 | `bon0-001/filter_bon0_002` | `cli_polarity` | `low:<37.5` | `7d9584354e6349781fec48a6b254184359541ce9b5f752e2c14b636188d9f295` |
| 4 | `bon0-001/filter_bon0_004` | `cli_polarity` | `mid:37.5-<75` | `7d9584354e6349781fec48a6b254184359541ce9b5f752e2c14b636188d9f295` |
| 5 | `bon0-002r/filter_bon0_001` | `phase_verification` | `unreached` | `a3e148bb6a43721e8e331bb41f91e87531bdeb46048940f1d0952768deb79850` |
| 6 | `bon0-002r/filter_bon0_006` | `other_acceptance` | `mid:37.5-<75` | `0d5995747192736cc95251ed630b7d5fab4ced4644511156b0be49c4cfeb0d3e` |
| 7 | `bon0-003r/filter_bon0_002` | `profile_probe` | `unreached` | `7a76eab412b7680b23ae2251fbed8b41342ade7f5188846e8d2c749a8b127540` |
| 8 | `bon0-003r/filter_bon0_006` | `cli_polarity` | `mid:37.5-<75` | `cbe5acec2bb35c7ca59e6c9ef1101aaf74209d00b194f75ff3b6f50a1886289e` |
| 9 | `bon0-004r/filter_bon0_001` | `cli_claim_binding` | `low:<37.5` | `cc9d5be84a34bc5dd43fea23e9a9794455dfc88b3cd6b9e973aa121431ebaa68` |
| 10 | `luna-006/stats_luna_002` | `profile_probe` | `high:75-<100` | `079bdca9de316338605e29fd93b816be99b0e8ffa775ad6ab6e6ffbafcb6019f` |

## P2F@1 pre-declaration

The only prior measured repair rate is recovery-circle 1/3 = 33.3%; Wilson 95% CI [6.1%, 79.2%]. Its denominator is three, so the uncertainty must remain broad.

Using Jeffreys Beta(0.5, 0.5) updated by 1/3 gives posterior Beta(1.5, 2.5). For the pre-registered n=10 sample, the exact Beta-binomial equal-tail 95% predictive full-count band is 0..9 (predictive mean 3.75, an accounting reference rather than a stratum forecast). No failure-class or score-band point prediction is declared; those strata have no adequate denominator.

## Measurement gate

Before spend, the runner must fail closed on revision/binary, source workspace, recovery-plan SHA-256, production-path bytes, band bytes, or destination freshness. Observation and band verification will be written only after this declaration commit.
