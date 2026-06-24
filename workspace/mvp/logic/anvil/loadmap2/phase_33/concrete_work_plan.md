# Phase33 Concrete Work Plan

Date: 2026-06-23 JST

Status: implemented / reviewed

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and leave them untouched.
3. Re-read:
   - `workspace/mvp/logic/anvil/loadmap2/phase_32/followup_phase_split.md`;
   - `workspace/mvp/logic/anvil/loadmap2/phase_32/recovery_task_ledger.md`;
   - `workspace/mvp/logic/anvil/loadmap2/phase_32/focused_worklist.md`;
   - `docs/evaluation.md`;
   - `scripts/eval_report.py`;
   - `scripts/eval_failure_observation.py`;
   - `scripts/eval_case_schema.py`;
   - `tests/test_eval_report.py`.

Exit criteria:

- Phase33 scope is confirmed as eval/report recheck projection only.
- Phase34+ owners are preserved for non-Phase33 blockers.

## Step 1: Build The Phase33 Failure Inventory

1. Read current focused recheck summary:

   ```text
   eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236/recheck_summary.tsv
   ```

2. Extract all rows with `expected_assertion_status=failed_recheck`.
3. For each row, inspect `meta.json` and compare:
   - expected fields from case metadata;
   - explicit top-level meta fields;
   - `fixture_fields`;
   - observed recheck row fields.
4. Classify each row:
   - Phase33 projection-caused;
   - Phase34 raw diagnostic/sign-off classification;
   - Phase35 setup/profile/readiness;
   - later phase or true runtime behavior.

Exit criteria:

- Phase33 owns only projection-caused rows.
- Non-Phase33 rows have explicit downstream owner phase.

## Step 2: Design Recheck Projection Boundary

1. Identify where recheck currently constructs `observation_input`.
2. Add a small projection helper if needed, with this precedence:
   1. `fixture_fields`;
   2. explicit top-level `meta.json` observation fields;
   3. parsed failure evidence;
   4. derived defaults.
3. Confirm `normalize_observation` does not overwrite explicit fields with
   generic defaults.
4. Keep the helper deterministic and side-effect-free.

Exit criteria:

- The rule is generic and field-based.
- There is no provider/model/profile/case-id behavior branch.

## Step 3: Add Regression Tests First

Add tests in `tests/test_eval_report.py` for projection cases that failed in
current focused recheck:

1. explicit-stop row preserves `terminal_state=explicit_stop`;
2. completion evidence row preserves `completion_evidence_status=missing`;
3. stale deliverable row preserves `completion_evidence_status=stale`;
4. evidence-binding row preserves `evidence_binding_status=failed`;
5. attempt ledger row preserves `attempt_outcome=duplicate`;
6. step-policy/verifier-specific row preserves specialized terminal state.

Exit criteria:

- Tests fail before the projection fix or prove the intended behavior is
  currently missing.
- Tests do not weaken focused expected assertions.

## Step 4: Implement Projection Fix

1. Update `scripts/eval_report.py` and, only if needed,
   `scripts/eval_failure_observation.py`.
2. Keep the change inside eval/report.
3. Do not edit runtime, provider, profile, setup, minimal-loop, or repair
   code.
4. Avoid broad rewrites of report generation; only adjust recheck observation
   projection.

Exit criteria:

- Regression tests pass.
- Existing eval-report tests continue to pass.

## Step 5: Re-run Focused Recheck

Run:

```bash
python3 scripts/eval_report.py \
  eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --cases-dir eval/cases/focused/control-recovery \
  --recheck
```

Then record:

- total focused cases;
- focused successes;
- remaining `failed_recheck`;
- which remaining rows are Phase34, Phase35, or later-phase owned.

Exit criteria:

- Zero Phase33-owned projection failures remain.
- Remaining failures are not hidden or relabeled as success.

## Step 6: Documentation And Ledger Updates

1. Add `phase_33/implementation_report.md`.
2. Update `phase_32/recovery_task_ledger.md` with measured Phase33 results.
3. Update `phase_32/focused_worklist.md` if group membership changes.
4. Update `docs/evaluation.md` if the projection precedence is now public
   behavior.

Exit criteria:

- Phase32 remains open.
- Phase33 result is consumable by Phase34/35/36/37/38/39.

## Step 7: Verification

Run:

```bash
python3 tests/test_eval_report.py
python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_case_schema.py
git diff --check
```

If code outside eval/report changes, stop and review scope before continuing.

Exit criteria:

- All commands pass.
- Any current broad sign-off failure is attributable to Phase34+ blockers, not
  Phase33 projection.

## Step 8: Exit Review

Review against the lessons from Phase32:

- current eval roots remain authoritative;
- historical roots are not used as final proof;
- successful omitted current cases are not ignored;
- raw diagnostics remain blockers for Phase34;
- no broad sign-off root duplication is introduced;
- no runtime behavior is changed to satisfy report tests;
- no focused assertion is weakened.

Exit criteria:

- Phase33 can close only the eval/report projection blocker.
- Phase32 final decision remains
  `migration_not_complete_pending_current_eval_reconciliation`.

## Implementation Notes

Phase33 was implemented within the eval/report layer only.

Code changes:

- `scripts/eval_report.py` now prepares recheck observation input through a
  deterministic helper and preserves explicit `fixture_fields` on existing
  recheck row fields.
- `scripts/eval_runtime_job_report.py` no longer overwrites meaningful
  explicit evidence, completion, attempt, runtime outcome, target admission,
  or repair-plan status fields with generic derived values.
- `tests/test_eval_report.py` covers fixture-field projection, explicit-stop
  meta preservation, observed evidence-state preservation, and explicit
  target-admission preservation.
- `docs/evaluation.md` documents recheck source precedence.

Measured focused recheck result:

```text
root: eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236
passed_recheck: 78
failed_recheck: 4
```

Remaining failures:

- `focused-dispatch-manifest-repair`: Phase34/35 dispatch-action semantics.
- `focused-nextjs-dependency-setup`: Phase35 setup/profile/readiness.
- `focused-nextjs-endpoint-smoke`: Phase35 dev-server/profile readiness.
- `focused-nextjs-route-integration`: Phase35 profile/route/step-policy
  connection.

## Plan Review Result

Review changes incorporated:

- Added a failure inventory step before implementation.
- Added explicit source precedence to avoid another fixture/meta-field loss.
- Added test-first projection examples tied to current failing focused groups.
- Added split-forward criteria for non-Phase33 rows.
- Added Phase32 lesson checks to prevent premature migration-complete claims.
