# A14-A9 Recovery contract binding smoke analysis

## Scope and immutable inputs

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a9-smoke-01`
- Contract: `eval/goal_verify/v0/phase6-recovery-v4-a14-a9-contract.json`
  - contract ID: `phase6-recovery-v4-20260830-a14-a9-live-01`
  - SHA-256: `9bcfed9a2f43fbd53023d81263851c8dfd85c02dcb6b8be02a3da2d142872822`
- Product source SHA: `4962f4725a23b312264c3391ab28a1ca3824a638`
- Exact binary: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a9-exact-target/release/commandagent`
  - version: `commandagent 0.1.0 4962f472 2026-08-30T12:52:34+09:00`
  - SHA-256: `79ac8d032aecc4c1c0aac087a3e2c736e8c84582277245393f8c2e33ca930c8b`
- Exact-SHA CI: CI and acceptance both completed successfully. Evidence is
  `eval/goal_verify/v0/exact-sha-ci-4962f472.json`.

This is a three-pair instrument diagnostic. It is not an effect estimate and does not
authorize increasing the automatic Recovery count.

## Frozen report result

The unchanged report script completed successfully. All 27 instrument checks are true,
including the new `registered_recovery_verify_commands` check. The report is
`recovery-report-v4.json` (SHA-256
`a74a64e06ca588bbbdcb4b69944e6ef7f7f571a6c229e870772a6665cf4aa4ac`).

- `instrument_ready: true`
- `effect_attribution_ready: true`
- `effect_claim_allowed: false`
- no Recovery needed: 1
- unchanged fail: 2
- attributed improved / harmed: 0 / 0
- unusable: 0
- median Recovery increment: 74,792 tokens and 141,186 ms

## Pair-level observations

| Pair | Recovery | External transition | Product transition | Recovery-only artifact change | Finding |
| --- | ---: | --- | --- | ---: | --- |
| task-01 | 0 | pass -> pass | completed -> completed | 0 | Read-only current-success preflight correctly suppressed Recovery. |
| task-05 | 1 | fail -> fail | failed_recoverable -> failed | 0 | Top-level handoff used the registered contract commands, but an inner `repair-unknown` StepPlan generated `test $? -eq 2`; independent command execution made the stateful fragment fail. Treatment was rejected. |
| task-10 | 1 | fail -> fail | failed_recoverable -> failed | 0 | Recovery repaired `cli.py`, and all three registered commands passed inside the treatment. Final acceptance nevertheless reported `missing_required_evidence:bound_verify_command`; treatment was rejected and the control retained. |

No historical record was rescored. Zero promoted artifact changes means no observed harm in
the retained outputs, not that no harmful mutation occurred inside the isolated treatment.

## What A9 proved

A9 fixed the outer Recovery candidate boundary. Both executed Recovery attempts emitted
`recovery_candidate_verify_commands_bound`; the Recovery start event reported
`recovery_verify_command_source: completion_contract`, and the report found no provenance
violation. Unregistered step-level shell fragments from the original failed plan were no
longer copied into the top-level Recovery handoff.

The transaction boundary also worked: neither failed treatment was promoted, the control
snapshot matched the captured boundary, and the report recorded no regression or existing
artifact harm in the retained output.

## Newly isolated product defects

### 1. Inner Recovery StepPlans are not contract-bound

The contract binding stops at the generated Recovery UltraPlan. Each phase is later expanded
by the model into a StepPlan, and those steps may introduce new verification commands. In
task-05 the model generated `python3 cli.py 11` followed by `test $? -eq 2`. CommandAgent
executes verify entries as independent commands, so `$?` has no relationship to the preceding
entry. The resulting failure is a host/plan-contract mismatch, not evidence that the repair
failed.

The same boundary allows an `inspect-current-state` phase to contain a failing verify step.
Bounded step repair can then mutate the workspace during a phase described as read-only.
Isolation prevents promotion, but it wastes tokens and weakens the semantic precision of the
Recovery Plan.

### 2. `pytest` is not recognized as semantic bound verification

`src/minimal_loop/evidence.rs::verify_command_kind` recognizes `unittest`, Cargo tests, and
several Node test forms, but not `python3 -m pytest` / `python -m pytest` / direct `pytest`.
Consequently the registered command `python3 -m pytest -q tests` is classified as `Other`.
Although task-10 executed the registered reproducer, pytest suite, and contract check
successfully, `bound_verify_command` remained `absent` and final acceptance rejected a repair
that the frozen external oracle would otherwise have evaluated.

This classifier omission is generic command semantics; it is not a per-profile setting.

### 3. Successful phase execution is not sufficient to close structural evidence debt

The task-10 phase events prove the exact registered checks passed, but runtime acceptance
derives `bound_verify_command` from the command classifier. The execution trace and structural
classifier therefore disagree. A10 must fix the classifier and retain the existing final
acceptance run; it must not bypass the evidence gate or treat an LLM success statement as
proof.

## A14-A10 implementation direction

1. Add a leaf Recovery StepPlan binding module and keep `runner.rs` / `loop_run.rs` wiring
   minimal.
2. Apply it only when `UltraPlan.intent == recover`; behavior is shared across profiles.
3. Keep `inspect-current-state` read-only: reject or deterministically exclude mutating and
   verify-failure-triggering steps from that phase, with an explicit event.
4. For repair and final verification phases, accept only exact commands from
   `CompletionContract.verify_commands`; discard no registered final check and add no new
   acceptance claim. Record before/after commands and source.
5. Recognize established pytest invocation forms as `Test` only when a test artifact exists.
6. Preserve transactional promotion: a treatment is promoted only after internal acceptance,
   frozen external success oracles, and frozen regression oracles pass.
7. Add focused Rust tests and update the issue-399 corpus fixture and reporter gate.

Acceptance requires: no unregistered inner Recovery verify command; no mutation during the
inspection phase; task-01 still suppresses Recovery; at least one initially failing task
executes exactly one Recovery; task-10 can be promoted only after all registered commands and
external oracles pass; control retention and resource accounting remain intact. A smoke is an
instrument check only and cannot establish the population effect of Recovery.
