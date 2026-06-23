# Phase34 Concrete Work Plan

Date: 2026-06-23 JST

Status: implemented / verified

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and leave them untouched.
3. Re-read:
   - `workspace/mvp/logic/anvil/loadmap2/phase_32/followup_phase_split.md`;
   - `workspace/mvp/logic/anvil/loadmap2/phase_32/recovery_task_ledger.md`;
   - `workspace/mvp/logic/anvil/loadmap2/phase_32/focused_worklist.md`;
   - `docs/evaluation.md`;
   - `eval/README.md`;
   - `scripts/eval_signoff.py`;
   - `scripts/eval_report.py`;
   - `scripts/eval_failure_observation.py`;
   - `tests/test_eval_report.py`;
   - `tests/test_eval_signoff.py`.

Exit criteria:

- Phase34 scope is confirmed as raw diagnostic classification / sign-off
  admission only.
- Phase35+ owners are preserved for setup/profile/dev-server, large ownership,
  row-proof, and final sign-off admission blockers.

## Step 1: Build The Phase34 Raw Diagnostic Inventory

1. Run current broad sign-off:

   ```bash
   python3 scripts/eval_signoff.py --require-recheck \
     --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
     --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
     --root large=eval/runs/current-all-local-llm/large/20260623T204816
   ```

2. Extract findings with codes:
   - `raw_undiagnostic_rc`;
   - `unknown_contract_layer`;
   - `unknown_terminal_state`;
   - target/evidence findings that are direct consequences of raw diagnostic
     admission gaps.
3. Inspect the corresponding row in `recheck_summary.tsv`.
4. Inspect the run's `meta.json`, `stdout.txt`, `stderr.txt`, repair packets,
   and workspace artifact list.
5. Record inventory fields:
   - case id;
   - run;
   - reason;
   - diagnostic code;
   - terminal state;
   - contract layer;
   - active job;
   - owner;
   - action;
   - target path / selected target;
   - repair packet diagnostic evidence;
   - workspace/profile target candidates;
   - Phase34 or later owner.

Exit criteria:

- `large-rust-app-new` is either confirmed as the only Phase34 raw diagnostic
  row or additional rows are listed.
- Focused assertion failures from the normal summary are not misclassified as
  Phase34 unless they are raw diagnostic admission failures.

## Step 2: Design The Diagnostic Projection

1. Add a small design note in implementation comments or implementation report
   before code changes:

   ```text
   raw rc evidence -> deterministic diagnostic candidate -> target admission
   candidate -> sign-off finding removal or explicit remaining blocker
   ```

2. Choose the implementation boundary:
   - `eval_failure_observation.py` for diagnostic extraction from evidence;
   - `eval_report.py` for recheck projection from run artifacts;
   - `eval_signoff.py` only for interpreting already-classified rows.
3. Define the diagnostic mapping for current evidence:
   - `minimal loop reached max iterations` -> `minimal_loop_max_iterations`;
   - `bash command blocked ... compound shell commands` ->
     `blocked_bash_command_policy`;
   - repair packet `reason: turn_error` -> `turn_error` only when no more
     specific evidence is available.
4. Define target admission mapping:
   - explicit repair target wins;
   - verifier-mentioned path wins over profile hint;
   - profile entrypoint/integration artifact can be admitted only if it exists
     in the run workspace and the selected owner/action can legitimately target
     source;
   - tool-policy boundary may use `not_applicable` only with explicit
     owner/action/attempt outcome and stop reason.

Exit criteria:

- The design is generic across profiles.
- The design does not add a case-id branch for `large-rust-app-new`.
- Evidence-poor `rc_1` rows remain failures.

## Step 3: Add Regression Tests

Add tests before or alongside implementation:

1. `eval_failure_observation` or `eval_report` test for:
   - raw `rc:1` plus `minimal loop reached max iterations`;
   - raw `rc:1` plus blocked Bash policy evidence;
   - repair packet evidence overriding `rc_1`.
2. `eval_signoff` test for:
   - raw `rc_1` still fails when no useful diagnostic exists;
   - classified tool-policy/loop-boundary row no longer fails
     `raw_undiagnostic_rc`;
   - target remains required for repairable source/profile/verifier rows;
   - target may be non-applicable only for accepted boundary rows.
3. Optional fixture test for `large-rust-app-new`-like row without using a
   case-id-specific branch.

Exit criteria:

- Tests express the raw diagnostic admission contract.
- Tests do not weaken broad sign-off by accepting all non-`rc_1` diagnostics
  blindly.

## Step 4: Implement The Narrow Classifier

1. Implement deterministic extraction in the selected eval/report helper.
2. Prefer existing structured fields over heuristic text parsing.
3. Use text patterns only for stable evidence phrases already emitted by
   CommandAgent:
   - `minimal loop reached max iterations`;
   - `bash command blocked`;
   - `compound shell commands`;
   - `tool error: bash command blocked`.
4. Populate row-visible fields needed by sign-off:
   - `diagnostic_code`;
   - `source_of_truth`;
   - `failure_signature`;
   - `active_job` / `recovery_owner` / `repair_action` only if already
     derivable from deterministic evidence;
   - target fields only from deterministic target sources;
   - `attempt_outcome` / explicit stop or boundary reason only when supported.
5. Do not change runtime success criteria.

Exit criteria:

- `large-rust-app-new` no longer appears as `raw_undiagnostic_rc`.
- If target remains blank, the row has a documented accepted non-target
  boundary or remains a Phase36 blocker instead of being hidden.

## Step 5: Re-run Recheck And Sign-off

1. Re-run current large recheck:

   ```bash
   python3 scripts/eval_report.py \
     eval/runs/current-all-local-llm/large/20260623T204816 \
     --cases-dir eval/cases/large \
     --recheck
   ```

2. Re-run current focused recheck only if implementation touches shared report
   projection fields:

   ```bash
   python3 scripts/eval_report.py \
     eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
     --cases-dir eval/cases/focused/control-recovery \
     --recheck
   ```

3. Re-run current broad sign-off:

   ```bash
   python3 scripts/eval_signoff.py --require-recheck \
     --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
     --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
     --root large=eval/runs/current-all-local-llm/large/20260623T204816
   ```

4. Compare findings before/after:
   - Phase34-owned findings should be gone;
   - Phase35/36/37/38/39 findings should remain explicit and owned.

Exit criteria:

- No `raw_undiagnostic_rc` remains for current roots.
- No raw diagnostic is hidden by relabeling it as success.

## Step 6: Documentation And Ledger Updates

1. Add `phase_34/implementation_report.md`.
2. Update `phase_32/recovery_task_ledger.md` P32-R007 with measured result.
3. Update `phase_32/followup_phase_split.md` only if ownership shifts.
4. Update `docs/evaluation.md` / `eval/README.md` only for public eval or
   sign-off semantics.
5. Keep Phase32 decision open unless Phase35+ and final sign-off conditions
   are also satisfied.

Exit criteria:

- Phase34 result is consumable by Phase35/36/37/38/39.
- Remaining blockers have owner layer, source evidence, and proof command.

## Step 7: Verification

Run:

```bash
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_signoff.py
git diff --check
```

If any runtime, provider, profile, setup, or minimal-loop file changes, stop
and review the layer boundary before continuing.

Exit criteria:

- All commands pass.
- Any current broad sign-off failure is attributable to Phase35+ blockers, not
  Phase34 raw diagnostic coverage.

## Step 8: Exit Review

Review against the lessons from Phase32 and Phase33:

- current eval roots remain authoritative;
- historical roots are not used as final proof;
- successful omitted current cases are not ignored;
- raw diagnostics are closed only by useful deterministic evidence;
- no broad sign-off root duplication is introduced;
- no runtime behavior is changed to satisfy report tests;
- no focused assertion is weakened;
- target admission does not invent a source file from task intent alone.

Exit criteria:

- Phase34 can close only the raw diagnostic classification blocker.
- Phase32 final decision remains
  `migration_not_complete_pending_current_eval_reconciliation` unless all
  later phase conditions are also met.

## Execution Result

Phase34 execution followed this plan with one bounded implementation choice:
the sign-off script itself was not changed because existing gates were already
strict enough once recheck rows had useful diagnostic and target attribution.

Implemented projection:

```text
stderr/stdout/repair packet evidence
  -> diagnostic_code_from_evidence
  -> useful diagnostic on recheck row
  -> deterministic profile/workspace target admission
  -> unchanged sign-off gate
```

Measured output for `large-rust-app-new` after recheck:

```text
reason=rc:1
diagnostic_code=blocked_bash_command_policy
terminal_state=verifier_command_failed
active_job=source_implementation_repair
recovery_owner=source
repair_action=edit_source_for_diagnostic
target_path=src/main.rs
target_admission_status=admitted
evidence_binding_status=bound
completion_evidence_status=failed
```

Current broad sign-off still exits nonzero because focused assertion failures
remain open. It no longer reports Phase34-owned raw diagnostic or large target
findings.

## Plan Review Result

Review changes incorporated:

- Added explicit inventory before implementation to avoid repeating the Phase32
  incomplete-surface problem.
- Added target admission constraints because Phase34 currently has both
  `raw_undiagnostic_rc` and `missing_target` on the same row.
- Added a focused recheck rerun only when shared report fields are changed,
  limiting unnecessary eval churn.
- Kept Phase34 out of setup/profile/dev-server readiness and large quality
  ownership, preserving Phase35 and Phase36 boundaries.
- Added negative tests for evidence-poor raw `rc` rows so the sign-off gate is
  not weakened.
