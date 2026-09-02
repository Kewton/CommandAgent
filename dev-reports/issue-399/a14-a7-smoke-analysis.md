# A14-A7 Recovery preflight integration smoke analysis

## Outcome

A14-A7 completed all three preregistered CLI pairs and corrected the A14-A6.1
blanket preflight stop. Two initially failing pairs crossed the read-only
preflight, captured a shared boundary, and executed exactly one isolated
Recovery treatment. One pair completed on its initial attempt and correctly
executed zero Recovery runs.

The two Recovery treatments did not improve the frozen external oracle. Both
were rejected safely before promotion because the isolated treatment retained
the control workspace's absolute completion-contract path. This run therefore
qualifies the typed capability-to-preflight integration but exposes the next
transaction binding defect.

## Working directories and exact build

- Repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- Execution root: `/Volumes/SSD_NX/tmp/commandagent_trial`
- Clean source worktree: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a7-source`
- Clean build target: `/Volumes/SSD_NX/tmp/CommandAgent-a14-a7-exact-target`
- Run: `dev-reports/issue-399/runs/phase6-recovery-v4-20260830-a14-a7-smoke-01`
- Exact code SHA: `e4cf4dec70a8486e41e9fdb7abc34964fc0652ae`
- Binary version: `commandagent 0.1.0 e4cf4dec 2026-08-30T11:48:59+09:00`
- Binary SHA-256: `3077a836bd32db998aa08811c9e9bab098cd98cf423e559bb8da37160c94b9a9`
- Exact-SHA CI and acceptance: completed successfully.

## Result summary

| Observation | Result |
|---|---:|
| Completed pairs | 3/3 |
| Report checks | 26/26 true |
| Recovery executed | 2/3 pairs, exactly once each |
| Initial success / no Recovery needed | 1/3 |
| Attributed improvement / harm | 0 / 0 |
| Unchanged external fail | 2 |
| Treatment path changes promoted | 0 |
| Control retained | 2/2 executed treatments |
| Regression introduced | 0 |

The report has `instrument_ready: true` and
`effect_attribution_ready: true`, because the two executed pairs share the exact
pre-Recovery history and boundary and were scored by the frozen external
oracle. `effect_claim_allowed` remains false because this is a three-case
instrument diagnostic, not a population effect estimate.

The report-wide median incremental cost, which includes the zero-delta initial
success pair, was 1,365 total tokens and 6,996 ms. The two actually executed
Recovery increments were 5,855 tokens / 55,919 ms and 1,365 tokens / 6,996 ms.

## Confirmed A14-A7 effect

For both initially failing pairs the event sequence was:

1. typed `fix_reproducer_command` bound to the exact registered command;
2. registered read-only preflight executed and returned fail;
3. Recovery boundary snapshot captured;
4. `recovery_plan_auto_run_start` emitted;
5. one treatment workspace executed;
6. failed treatment rejected and control retained.

This is the expected behavior change from A14-A6.1, which stopped before step
3 in all three pairs. Unsupported browser and unbound capabilities remain
fail-closed by unit tests.

## New failure mechanism

Both treatments stopped with:

`completion contract file must be under workspace or temp directory`

The configured file was the host-owned
`before/.goal-verify-baseline/completion-contract.json` in the control
workspace. `RunnerRecoveryDriver::start` clones `Config` and changes only
`workspace_root` to the isolated treatment. It does not copy and rebind the
completion contract. The contract loader then correctly rejects the old
control path because it is outside the new treatment root.

The direct cause is a stale absolute path in `transaction_config`. The root
cause is that isolated treatment creation copied source artifacts and selected
runtime dependencies, but the newly added host-owned completion contract was
not included in the transaction's runtime binding model.

## A14-A8 correction

1. Resolve the already validated source completion contract from the control
   configuration.
2. Copy its exact bytes into a host-owned, source-excluded location under the
   treatment workspace.
3. Rebind only the treatment configuration to that copied file before Recovery
   execution.
4. Keep the control configuration, external oracle boundary, source snapshot,
   maximum Recovery count, and promotion checks unchanged.
5. Reject treatment preparation if copying or rebinding fails; never fall back
   to an outside-workspace path.
6. Test explicit contract paths, byte identity, treatment confinement, missing
   sources, and absence of the binding when no contract is configured.

The A14-A7 run remains immutable and must not be rescored as A14-A8 evidence.
