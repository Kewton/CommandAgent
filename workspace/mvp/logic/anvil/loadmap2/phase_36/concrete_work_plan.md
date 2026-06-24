# Phase36 Concrete Work Plan

Date: 2026-06-24 JST

Status: completed

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and leave them untouched.
3. Confirm Phase36 scope from:
   - `phase_32/followup_phase_split.md`;
   - `phase_32/recovery_task_ledger.md`;
   - `phase_32/focused_worklist.md`;
   - `phase_35/implementation_report.md`.
4. Confirm the current root set:
   - smoke: `eval/runs/current-all-local-llm/smoke/20260623T203030`;
   - focused: `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236`;
   - large: `eval/runs/current-all-local-llm/large/20260623T204816`.

Exit criteria:

- Phase36 owns large row disposition only.
- Phase35 focused assertion closure remains out of scope except for
  non-regression checks.

## Step 1: Rebuild Current Large Evidence

1. Re-run large recheck:

   ```bash
   python3 scripts/eval_report.py \
     eval/runs/current-all-local-llm/large/20260623T204816 \
     --cases-dir eval/cases/large \
     --recheck
   ```

2. Re-run broad sign-off:

   ```bash
   python3 scripts/eval_signoff.py --require-recheck \
     --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
     --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
     --root large=eval/runs/current-all-local-llm/large/20260623T204816
   ```

3. Extract all six large rows from `recheck_summary.tsv`.
4. For each row, capture:
   - terminal state;
   - diagnostic code;
   - owner/action;
   - target/admission;
   - evidence status;
   - command;
   - artifact candidates;
   - stdout/stderr/repair packet evidence if available.

Exit criteria:

- The row inventory matches the six cases listed in `README.md`.
- Any newly observed large row is either added to the inventory or explicitly
  excluded with rationale.

## Step 2: Create The Large Row Ledger

1. Create `phase_36/large_row_ledger.md` during implementation.
2. Add one row per large case.
3. Include columns:
   - case id;
   - observed failure family;
   - owner layer;
   - active job;
   - selected action;
   - target;
   - evidence binding;
   - completion evidence;
   - source excerpt / diagnostic evidence;
   - disposition;
   - closure proof.
4. Require one of:
   - `closed_owned_failure`;
   - `implementation_blocker`;
   - `accepted_external_limitation`;
   - `split_forward`.

Exit criteria:

- No large case is represented only by narrative text.
- No row remains "unknown" without an owner phase and next action.

## Step 3: Check Owner/Action Consistency

1. Identify rows where owner and action disagree:
   - `source` owner with `correct_tool_protocol`;
   - `tool_protocol` owner without failed tool/missing field evidence;
   - `explicit_stop` without explicit stop reason;
   - source/verifier row without a target.
2. Decide whether the mismatch is:
   - eval/report projection bug;
   - runtime recovery owner/action selection bug;
   - acceptable row disposition with explicit rationale.
3. Prefer eval/report repair when the runtime already emitted sufficient
   evidence.
4. Touch runtime only if the runtime selected contradictory owner/action data
   and no report-only projection can honestly fix it.

Exit criteria:

- Every large row has internally consistent owner/action/target/evidence or an
  explicit `implementation_blocker`.

## Step 4: Add Shared Projection Or Sign-off Rules

Apply only the changes justified by Step 3.

Potential shared changes:

1. Verifier failure projection:
   - replace weak `unknown_verifier_failure` only from deterministic stderr,
     verifier command, or repair packet evidence;
   - keep `unknown_verifier_failure` if the evidence is genuinely absent.
2. Tool-protocol projection:
   - surface failed tool and missing field when present in stderr, stdout,
     meta, or repair packets;
   - map tool-protocol rows to `tool_protocol` owner/action consistently.
3. Edit-target failure projection:
   - distinguish stale edit target, missing target, and replacement target;
   - keep target admission bound to workspace/profile evidence.
4. Explicit-stop projection:
   - preserve read-only mutation as explicit stop;
   - require explicit stop reason, target, and evidence binding.
5. Large-disposition sign-off:
   - add report-only checks only if current broad sign-off admits
     contradictory rows that Phase36 must block.

Exit criteria:

- Shared behavior keys off failure family and evidence fields, not case id.
- No model-facing prompt, provider transport, hidden retry, or implicit setup
  behavior changes.

## Step 5: Row-by-row Closure

Close each row with the following target:

| case | target closure |
| --- | --- |
| `large-fastapi-app-modify` | `closed_owned_failure` with verifier command/source evidence, or `implementation_blocker` if diagnostic remains too weak. |
| `large-fastapi-app-new` | `closed_owned_failure` under tool-protocol owner after failed tool/missing field evidence is visible. |
| `large-nextjs-app-modify` | `closed_owned_failure` or `implementation_blocker` after source/tool-protocol owner/action mismatch is resolved. |
| `large-nextjs-app-new` | `closed_owned_failure` as explicit stop for read-only mutation, or `implementation_blocker` if explicit-stop reason/evidence is insufficient. |
| `large-rust-app-modify` | `closed_owned_failure` or `implementation_blocker` after source/tool-protocol owner/action mismatch is resolved. |
| `large-rust-app-new` | `closed_owned_failure` retaining Phase34 target admission and adding large disposition evidence. |

Exit criteria:

- `large_row_ledger.md` has no unclassified row.
- Any `implementation_blocker` is backed by a concrete next code change, not a
  vague "large still failed" note.

## Step 6: Docs And Handoff

1. Update `docs/evaluation.md` if large-disposition fields or source-excerpt
   semantics change.
2. Update `eval/README.md` if broad sign-off interpretation changes.
3. Update Phase32 recovery files with Phase36 measured results.
4. Add `phase_36/implementation_report.md` during implementation.
5. If any row must move to Phase37+, add a handoff section naming:
   - row;
   - destination phase;
   - remaining proof gap;
   - closure command.

Exit criteria:

- Phase36 output can be read without inferring hidden decisions from commit
  messages.
- Phase37 receives concrete row-to-case proof inputs.

## Step 7: Verification

Run targeted checks based on touched files.

Always run after code changes:

```bash
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py scripts/eval_failure_observation.py scripts/eval_runtime_job_report.py scripts/eval_case_schema.py
bash -n scripts/eval_agent_slice.sh
cargo fmt --check
cargo test
cargo build --release
git diff --check
```

Always rerun Phase36 evidence commands:

```bash
python3 scripts/eval_report.py \
  eval/runs/current-all-local-llm/large/20260623T204816 \
  --cases-dir eval/cases/large \
  --recheck

python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Exit criteria:

- All applicable tests pass.
- Broad sign-off remains pass or any failure is assigned to a later phase with
  row-level evidence.

## Step 8: Exit Review

Before closing Phase36, verify:

- all six large rows have a disposition;
- broad sign-off pass is not described as task success;
- no implementation-quality failure is hidden as external limitation;
- owner/action/target/evidence are internally consistent or explicitly
  blocked;
- focused rows remain closed after shared eval/report changes;
- no hidden retry, provider branch, implicit setup, or verifier weakening was
  added;
- Phase37 handoff is complete for row-proof reconciliation.

## Execution Result

Phase36 executed the plan with eval/report-only changes.

| step | result |
| --- | --- |
| Step 0 | Preflight recorded one unrelated dirty file: `phase_21/implementation_report.md`; it was left untouched. |
| Step 1 | Current large and focused rechecks were regenerated from existing roots. |
| Step 2 | `large_row_ledger.md` was created with one row per current large case. |
| Step 3 | Source/tool-protocol owner/action mismatches were resolved by failure-family projection where tool-protocol evidence existed. |
| Step 4 | `large_disposition*` report fields and sign-off checks were added as report-only behavior. |
| Step 5 | All six current large rows closed as `closed_owned_failure`; no external limitation was used. |
| Step 6 | `docs/evaluation.md`, `eval/README.md`, and Phase32 recovery files were updated. |
| Step 7 | Targeted eval/report tests, focused recheck, large recheck, broad sign-off, `cargo fmt --check`, `cargo test`, `cargo build --release`, and `git diff --check` passed. |
| Step 8 | No hidden retry, provider/model branch, implicit setup, or verifier weakening was added. |

## Plan Review Result

Review findings incorporated:

- Added Step 2 large ledger creation so Phase36 cannot close by summary only.
- Added Step 3 owner/action consistency before code edits because current
  large rows already show inconsistent source/tool-protocol mapping.
- Added Step 4 as conditional shared projection work instead of mandatory
  runtime edits.
- Added explicit row-by-row target closure to prevent grouping incompatible
  large failures.
- Added non-regression checks for Phase35 focused closure.
- Added Phase37 handoff requirements for any row that remains proof-related
  rather than Phase36-owned.
