# A14-A8 Recovery treatment contract binding smoke analysis

## Outcome

A14-A8 completed all three preregistered CLI pairs and passed all 26 report
checks. One initially failing pair executed exactly one isolated Recovery
treatment. The treatment loaded the copied CompletionContract from inside its
own workspace, so the A14-A7 stale absolute-path failure is resolved.

The Recovery treatment did not improve the frozen external oracle. It was
rejected without promotion and the control workspace was retained. The next
failure is not contract-file placement: the selected step-level Recovery
candidate carried an unregistered, stateful shell predicate split into three
independent verify commands. The final predicate ran without the variable
assignment and failed before a useful source repair was produced.

## Working directories and exact build

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Clean source worktree: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a8-source`
- Clean build target: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a8-exact-target`
- Run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a8-smoke-01`
- Exact code SHA: `41a6bd684d38141e478f788b73f086b88096c1ef`
- Binary version: `commandagent 0.1.0 41a6bd68 2026-08-30T12:13:26+09:00`
- Binary SHA-256: `a2a2d493ab82d8f5024d80fb7405c84c6b7fae63421cb5c0adc87b600d0d8084`
- Exact-SHA CI run 33289774307 and acceptance run 33289774251: completed successfully.

## Result summary

| Observation | Result |
|---|---:|
| Completed pairs | 3/3 |
| Report checks | 26/26 true |
| Recovery executed | 1/3 pairs, exactly once |
| Initial success / no Recovery needed | 2/3 |
| Attributed improvement / harm | 0 / 0 |
| Unchanged external fail | 1 |
| Treatment path changes promoted | 0 |
| Control retained | 1/1 executed treatment |
| Regression introduced | 0 |

The report has `instrument_ready: true` and
`effect_attribution_ready: true`. `effect_claim_allowed` remains false because
this is a three-case instrument diagnostic. Two zero-Recovery pairs make the
report-wide median incremental cost zero. The executed Recovery treatment
added 149,545 tokens and 247,271 ms.

## Confirmed A14-A8 effect

For `phase6-main-c05-task-10--pair-01` the event and artifact sequence was:

1. the exact registered final-success command failed in the read-only
   preflight;
2. the shared Recovery boundary was captured;
3. the isolated treatment workspace was created;
4. the CompletionContract was copied byte-for-byte to
   `.commandagent/recovery-runtime/completion-contract.json` inside the
   treatment and loaded successfully;
5. one Recovery treatment executed;
6. the failed treatment was rejected and the unchanged control was retained.

The treatment records `completion_contract_bound: true` with SHA-256
`d9aed659a40b0017d9e0e9e19579c0773137ffa34706995f9540e745c8162d94`.
The A14-A7 outside-workspace contract error did not recur.

## New failure mechanism

The initial LLM step `reproduce-failure` supplied these verify commands:

1. `python3 cli.py 16`
2. `exit_code=$?`
3. `[ $exit_code -eq 2 ]`

The verifier runs each list entry independently. Consequently the third
command has no `exit_code` value and fails with `unary operator expected`.
The step-level Recovery handoff copies this list verbatim from
`RepairContext.verify_commands`; `record_candidate` prefers the step-scoped
candidate, while the read-only Recovery preflight separately uses the correct
CompletionContract commands. Binding the contract into the treatment therefore
fixed file reachability but did not correct the selected Recovery Plan's
verification-command provenance.

The root cause is a split authority model: pre/post Recovery acceptance is
bound to the typed CompletionContract, but Recovery Plan construction can
still inherit LLM-authored step commands that are not exact registered
final-success commands. The current report validates the typed initial binding
but does not assert that every preferred Recovery check came from the same
registered contract.

## A14-A9 correction

1. When a typed `fix_reproducer_command` is configured, derive Recovery's
   preferred final-success checks from the validated CompletionContract, not
   from a failed step's locally generated command list.
2. Preserve the failure scope, but replace unregistered command evidence with
   the read-only preflight result and exact registered checks before automatic
   execution.
3. Reject the automatic Recovery candidate if authoritative rebinding cannot
   be proven; do not fall back to the LLM-authored command list.
4. Record old/new command provenance and add a report gate requiring the
   executed Recovery candidate's checks to be registered.
5. Keep maximum Recovery count at one, external oracles hidden, treatment
   isolation, control retention, regression checks, and resource accounting
   unchanged.
6. Add focused tests for stateful shell fragments, exact registered command
   rebinding, missing/invalid contracts, and unsupported capabilities.

The A14-A8 run remains immutable and must not be rescored as A14-A9 evidence.
