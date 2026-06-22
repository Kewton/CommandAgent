# Phase 17 Concrete Work Plan

## Work Package 1: Reproduce Phase 16 Sign-off Findings

Run the current sign-off checker against the recorded Phase 16 roots:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase16-focused-local-llm/20260622T173940 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Expected result: non-zero exit with the known focused and large blockers.

## Work Package 2: Build The Blocking Ledger

Create:

```text
workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md
```

Start from the Phase 16 report seed and expand each row with:

- exact sign-off finding;
- owning layer;
- failed contract;
- suspected module;
- responsible phase;
- proof command;
- closure condition.

Do not merge unrelated root causes into one row. If one visible failure has
both planning and recovery-task causes, split it unless one is clearly
downstream.

## Work Package 3: Build The Sign-off Reconciliation

Create:

```text
workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md
```

Use the exact current sign-off output as input. The table must contain one row
per finding with:

- finding id;
- family;
- case;
- signoff code;
- detail summary;
- ledger row;
- coverage responsibility;
- downstream phase;
- proof command;
- reconciliation status.

Expected current count:

```text
focused findings: 5
large findings: 14
total sign-off findings: 19
context blocker from Phase16 report: provider timeout evidence
```

The context blocker is not a separate sign-off finding, but it must remain in
the blocking ledger because it explains why the large time-boxed root cannot be
treated as a release-quality migration proof.

Stop before runtime work if:

- any sign-off finding has no ledger row;
- any ledger row has no sign-off or context source;
- any finding has no coverage responsibility;
- any finding has no downstream phase;
- any finding has no proof command.

## Work Package 4: Assign Phase18 Focused Recovery Rows

Assign focused blockers to Phase18:

- docs literal mismatch;
- Next.js dependency setup;
- Next.js endpoint smoke;
- Next.js route integration;
- raw focused diagnostic classification.

For each focused row, define a targeted proof command. Prefer running a
specific case directory or filtered case set before running the full focused
matrix.

## Work Package 5: Assign Phase19 Large Recovery Rows

Assign large blockers to Phase19:

- timeout rows;
- missing evidence binding;
- missing completion evidence;
- missing target;
- profile dependency conflict mapped to source repair.

For each large row, define whether the fix belongs to:

- eval timeout observation;
- completion evidence projection;
- active job arbitration;
- profile failure mapping;
- target admission/report projection.

## Work Package 6: Define Phase20 Declaration Gate

Update the final closure criteria so Phase20 can run only after:

- focused sign-off has zero failed expected assertions;
- large sign-off has no unowned failures;
- coverage table has no adopted `Partial` or `Missing`;
- the sign-off checker exits zero on final evidence roots.

## Work Package 7: Validate Horizontal Rollout Consistency

Confirm that the reconciliation table covers the current families without
profile-specific accounting branches:

| Family | Expected Phase17 accounting |
| --- | --- |
| focused | each failed expected assertion maps to ledger row and Phase18 proof |
| focused fixture | remains proof support; no separate runtime-completion claim |
| smoke | remains in sign-off command; no blocker currently emitted |
| large Next.js | manifest/profile/target/evidence blockers map to Phase19 |
| large Python/FastAPI | timeout/evidence blockers map to Phase19 |
| large Rust | timeout/evidence blockers map to Phase19 |

If a family requires a different accounting shape, stop and update the
recovery plan before runtime work starts.

## Work Package 8: Validate Documentation Consistency

Check:

```bash
rg -n "Phase 17|Phase 18|Phase 19|Phase 20|migration complete|sign-off" \
  workspace/mvp/logic/anvil/loadmap2 docs/eval docs/evaluation.md
```

Ensure no document says migration is complete before Phase20.

## Work Package 9: Review The Plan

Review questions:

1. Does this plan close the Phase16 process gap rather than only adding more
   docs?
2. Does every current sign-off finding have an accounting row?
3. Does every accounting row have coverage responsibility, downstream phase,
   proof command, and closure condition?
4. Does the plan avoid runtime behavior and hidden repair loops?
5. Does it avoid profile-specific completion processes?
6. Is Phase20 still the only migration-complete declaration point?

If a review answer is no, update the README, implementation tasks, and this
concrete work plan before Phase17 execution starts.

## Work Package 10: Phase17 Exit Review

Before closing Phase17, answer:

1. Can every Phase16 finding be traced to a ledger row?
2. Can every ledger row be traced back to a sign-off finding or context
   blocker?
3. Does every row have exactly one owner?
4. Does every row map to a coverage responsibility?
5. Does every row have a proof command?
6. Are Phase18 and Phase19 finite?
7. Is migration-complete declaration still blocked?

If any answer is no, Phase17 remains open.

## Review Result Reflected

The initial Phase17 plan correctly introduced a ledger and reconciliation, but
it still needed stronger controls in three areas:

- horizontal rollout across focused, fixture, smoke, and every large profile;
- documentation update boundaries;
- explicit review questions before execution.

This concrete plan now includes those controls. Phase17 remains docs/eval-only
unless reconciliation exposes a coverage-table schema gap.
