# Phase35 Concrete Work Plan

Date: 2026-06-23 JST

Status: implemented / verified

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and leave them untouched.
3. Confirm Phase35 scope from:
   - `phase_32/followup_phase_split.md`;
   - `phase_32/recovery_task_ledger.md`;
   - `phase_32/focused_worklist.md`.
4. Confirm Phase34 is already closed and do not re-open raw diagnostic work.

Exit criteria:

- Phase35 scope is setup/profile/dev-server/readiness plus manifest action
  semantics.
- Phase36+ ownership remains intact.

## Step 1: Build A Current Row Inventory

1. Re-run focused recheck:

   ```bash
   python3 scripts/eval_report.py \
     eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
     --cases-dir eval/cases/focused/control-recovery \
     --recheck
   ```

2. Re-run broad sign-off:

   ```bash
   python3 scripts/eval_signoff.py --require-recheck \
     --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
     --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
     --root large=eval/runs/current-all-local-llm/large/20260623T204816
   ```

3. Extract Phase35-owned rows:
   - `focused-dispatch-manifest-repair`;
   - `focused-nextjs-dependency-setup`;
   - `focused-nextjs-endpoint-smoke`;
   - `focused-nextjs-route-integration`;
   - any setup/profile/dev-server rows that reappear after recheck.
4. Extract non-Phase35 rows still reported by broad sign-off and assign them
   to Phase36+, Phase38, or a new explicit follow-up phase.

Exit criteria:

- The current row list distinguishes original `summary.tsv` failures from
  `recheck_summary.tsv` failures.
- Each Phase35 row has owner layer, proof mode, source evidence, and expected
  closure.

## Step 2: Classify Each Failure Before Editing

For each Phase35 row, fill this table in the implementation report before code
changes:

| field | required value |
| --- | --- |
| case id | Focused case id. |
| proof mode | `real_llm`, `deterministic_fixture`, or `report_fixture`. |
| observed failure | Current terminal/owner/action/target. |
| expected assertion | Exact expected field mismatch. |
| responsible layer | Runtime/recovery, profile, setup, eval/report, case definition, sign-off, or later phase. |
| proposed fix | Narrow contract change or explicit non-Phase35 assignment. |
| risk control | How the fix avoids hidden retry, provider branch, or assertion weakening. |

Exit criteria:

- No code edit starts without row-level classification.
- Real-LLM success expectation is either preserved and fixed by behavior, or
  explicitly justified as the wrong proof mode.

## Step 3: Implement Manifest Action Semantics

1. Inspect action selection in:
   - `src/agent/step_runner/recovery_orchestration.rs`;
   - `src/agent/step_runner/recovery_policy.rs`;
   - `src/agent/step_runner/runtime/repair_loop.rs`;
   - `scripts/eval_agent_slice.sh`;
   - `scripts/eval_report.py`.
2. Ensure evidence distinguishes:
   - missing dependency;
   - missing manifest;
   - invalid manifest;
   - version conflict.
3. Ensure selected action maps deterministically:
   - missing dependency -> `add_missing_manifest_dependency`;
   - version conflict -> `resolve_manifest_conflict`;
   - invalid manifest -> manifest repair with setup readiness evidence.
4. Update focused fixture fields only if the fixture's source evidence was
   incorrectly labeled.

Exit criteria:

- `focused-dispatch-manifest-repair` passes recheck or has a documented
  accepted row disposition without weakening the intended manifest action
  assertion.

## Step 4: Implement Setup Readiness Projection

1. Inspect setup fields in runtime evidence and eval report:
   - `setup_state`;
   - `setup_readiness`;
   - `setup_command_authority`;
   - `setup_artifact_validation_status`;
   - `setup_result`;
   - `setup_failure_signature`.
2. Add or correct deterministic projection for:
   - dependency missing;
   - dependency artifact missing;
   - manifest invalid;
   - setup command blocked or not authorized.
3. Ensure setup readiness does not imply implicit `npm install` or network
   setup during normal eval.

Exit criteria:

- `focused-nextjs-dependency-setup` and
  `focused-phase26-setup-node-readiness` semantics are either passing or
  explicitly assigned with evidence.

## Step 5: Implement Dev-server And Endpoint Smoke Projection

1. Inspect dev-server fields:
   - `dev_server_state`;
   - `requested_port`;
   - `port_preflight`;
   - `endpoint_smoke`;
   - `setup_state`.
2. Ensure endpoint smoke distinguishes:
   - setup failure before server launch;
   - port conflict;
   - server launch failure;
   - endpoint content mismatch;
   - successful endpoint evidence.
3. Keep dev-server smoke report-only unless verifier-owned commands already
   executed.

Exit criteria:

- `focused-nextjs-endpoint-smoke` no longer appears as a disconnected
  verifier row. It either passes as runtime success or reports an owned
  dev-server/setup boundary.

## Step 6: Implement Profile Route-integration Projection

1. Inspect Next.js profile facts:
   - selected route;
   - integration artifacts;
   - route/component binding;
   - failure mapping.
2. Ensure route integration failures select:
   - route integration owner when target can be repaired;
   - setup/profile owner when manifest/setup blocks route verification;
   - explicit stop when step policy blocks mutation.
3. Do not make the Next.js profile run a hidden workflow.

Exit criteria:

- `focused-nextjs-route-integration` has an honest route/profile/step-policy
  disposition and no disconnected setup/profile fields.

## Step 7: Recheck, Sign-off, And Residual Assignment

1. Run targeted tests from changed layers.
2. Run focused recheck.
3. Run broad sign-off.
4. Compare before/after:
   - Phase35-owned assertion failures should be gone or explicitly accepted;
   - Phase33-closed normal-summary findings should not be reintroduced as
     current blockers;
   - Phase36+ findings must remain visible.
5. Update Phase32 ledgers and add `phase_35/implementation_report.md`.

Exit criteria:

- Phase35 can close without claiming final migration completion.
- Any remaining sign-off failure has an owner phase and closure command.

## Step 8: Full Verification

Run the minimum repository checks for behavior changes:

```bash
cargo fmt --check
cargo test
cargo build --release
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
git diff --check
```

If implementation changes only docs/plans, run `git diff --check` and the
relevant documentation review instead.

Exit criteria:

- All applicable checks pass.
- Expected nonzero broad sign-off output is documented if Phase36+ remains
  open.

## Step 9: Exit Review

Review against the Phase32/33/34 lessons:

- current eval roots are authoritative;
- recheck evidence is not conflated with historical normal-summary failures;
- row-level classification precedes edits;
- no assertion is weakened to get a green result;
- setup remains explicit and verifier-owned;
- profiles provide facts, not hidden workflows;
- sign-off remains report-only;
- remaining blockers are visible and assigned.

## Execution Result

Phase35 executed the plan with these outcomes:

| step | result |
| --- | --- |
| current row inventory | Completed from current focused recheck and broad sign-off roots. |
| row classification | Completed in `implementation_report.md`; all Phase35 rows are boundary-proof or fixture-alignment rows. |
| manifest action semantics | Version-conflict fixture now expects `resolve_manifest_conflict`; missing dependency remains a separate action family. |
| setup readiness projection | `focused-nextjs-dependency-setup` now proves the plan-lint/setup-manifest boundary instead of pretending to be a runtime-success proof. |
| dev-server smoke projection | `focused-nextjs-endpoint-smoke` now asserts dev-server state, requested port, port preflight, and endpoint smoke result. |
| route integration projection | `focused-nextjs-route-integration` now asserts the step-policy explicit-stop boundary with target and action evidence. |
| sign-off interpretation | `--require-recheck` uses matching focused recheck rows as the current assertion authority. |
| recheck and sign-off | Focused assertions pass for all 82 current focused rows; broad sign-off exits zero. |

No provider behavior, minimal-loop retry behavior, implicit setup execution, or
hidden continuation was added.

Exit criteria:

- Phase35 closes only setup/profile/dev-server/readiness and manifest
  action-semantics blockers.
- Phase32 remains
  `migration_not_complete_pending_current_eval_reconciliation` unless later
  phases also close.

## Plan Review Result

Review changes incorporated:

- Added a mandatory row inventory and classification step before any code
  change.
- Added explicit handling for normal-summary focused assertion failures so
  Phase35 does not mask Phase33 or Phase38 issues.
- Added proof-mode decision gates for real-LLM focused cases.
- Split manifest, setup readiness, dev-server smoke, and route integration
  into separate implementation steps with independent exit criteria.
- Added stability controls for hidden retries, implicit setup, provider
  branching, and profile workflow creep.
