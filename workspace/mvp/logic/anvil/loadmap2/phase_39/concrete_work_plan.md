# Phase39 Concrete Work Plan

Date: 2026-06-24 JST

Status: completed / reviewed

## Step 0: Preflight

1. Run `git status --short --branch`.
2. Record unrelated dirty files and leave them untouched.
3. Confirm Phase39 scope:
   - final closure retry/reporting only;
   - no runtime, provider, profile, minimal-loop, or repair behavior changes.
4. Confirm current final roots:
   - smoke: `eval/runs/current-all-local-llm/smoke/20260623T203030`;
   - focused: `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236`;
   - large: `eval/runs/current-all-local-llm/large/20260623T204816`.

Exit criteria:

- Phase39 remains a docs/eval final-decision phase.
- Existing unrelated changes are not staged or modified.

## Step 1: Re-run Current Final Sign-off

Run:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Capture:

- `root_admission_status`;
- admitted roots;
- `family_case_counts`;
- `current_case_coverage`;
- final `status`.

Exit criteria:

- If sign-off fails, Phase39 writes `migration_not_complete` and a blocker
  rather than weakening sign-off.
- If sign-off passes, continue to row and coverage evidence reconciliation.

## Step 2: Build Decision Evidence Matrix

Create `decision_evidence_matrix.md`.

Rows:

1. coverage C01-C45 implemented/proven;
2. coverage C46-C54 excluded with rationale;
3. Phase32 exit gate item 1 root/case coverage;
4. Phase32 exit gate item 2 current broad sign-off;
5. Phase32 exit gate item 3 focused assertion closure;
6. Phase32 exit gate item 4 large owner/action/target/evidence closure;
7. Phase32 exit gate item 5 no adopted row depends only on omitted historical
   roots;
8. Phase32 exit gate item 6 final report written;
9. root admission stale/duplicate-root protection;
10. no accepted external proof limitation is hiding missing functionality.

For each row record:

- source document;
- command if applicable;
- observed value;
- pass/fail;
- final decision effect.

Exit criteria:

- Every final decision claim points to concrete evidence.
- Any missing evidence produces a named blocker.

## Step 3: Draft Final Closure Report

Create `final_closure_report.md`.

The report must include:

- decision;
- current roots;
- current sign-off output summary;
- coverage counts;
- excluded rows and rationale;
- Phase33-Phase38 proof summary;
- large task success caveat;
- superseded historical root statement;
- remaining limitations, if any.

Decision rule:

- Use `migration_complete_with_explicit_exclusions` if all adopted rows are
  implemented/proven, excluded rows are explicit, and current sign-off passes.
- Use `migration_complete` only if recovery-plan wording is confirmed to treat
  excluded rows as outside the adopted migration target.
- Use `migration_not_complete` if any evidence gate fails.

Exit criteria:

- The report states exactly one decision.
- It does not claim failed large app-generation rows are successful tasks.

## Step 4: Update Public And Roadmap Docs

Update:

1. `docs/eval/loadmap2-final-migration-decision-20260624.md`, or the current
   final-decision report with a clear date and supersession statement.
2. `docs/migration-progress.md`.
3. `docs/eval/legacy-control-stack-coverage-20260621.md` Phase32 appendix.
4. `workspace/mvp/logic/anvil/loadmap2/README.md`.
5. `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`.
6. `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`.
7. `phase_32/recovery_task_ledger.md`.
8. `phase_32/followup_phase_split.md`.

Exit criteria:

- No authoritative roadmap/status doc still says current broad sign-off is
  failing unless Phase39 actually records `migration_not_complete`.
- Phase39 is visible as the final closure proof phase.
- Historical Phase32 evidence remains marked as superseded.

## Step 5: Verification

Run:

```bash
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_signoff.py
git diff --check
```

Run current final-current sign-off again after docs are updated:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Run Rust checks only if code outside docs/eval Python is touched:

```bash
cargo fmt --check
cargo test
cargo build --release
```

Exit criteria:

- Sign-off remains pass.
- Diff has no whitespace or conflict artifacts.
- Test scope matches actual changed layers.

## Step 6: Exit Review

Before closing Phase39, verify:

- final report states exactly one decision;
- all Phase32 exit gate items are closed or explicitly fail to
  `migration_not_complete`;
- no stale current-signoff-failing text remains in authoritative docs;
- historical roots are regression evidence only;
- current roots are admitted and cover 91/91 current cases;
- large failed rows are owned failures, not successful user-task output;
- excluded rows are explicit architecture choices;
- no hidden retry, runtime orchestration, provider/model branch, implicit
  setup, or verifier weakening was added.

## Plan Review Result

Review findings incorporated:

- The work is split into sign-off rerun, decision matrix, final report, docs
  update, verification, and exit review.
- The plan treats `migration_not_complete` as an honest valid outcome if any
  proof is missing.
- The plan requires stale roadmap cleanup so Phase39 does not create another
  contradictory completion state.
- The plan avoids adding a final-closure engine; it uses a deterministic matrix
  and existing sign-off output.
- The plan distinguishes Anvil migration parity from generated application
  quality.

## Implementation Result

Phase39 followed the concrete work plan:

1. Confirmed current roots and existing dirty state.
2. Re-ran current final sign-off.
3. Created the decision evidence matrix and final closure report.
4. Updated public eval docs, migration progress, roadmap, recovery plan,
   current issue map, and Phase32 recovery files.
5. Ran focused verification.

Final state:

```text
migration_complete_with_explicit_exclusions
```
