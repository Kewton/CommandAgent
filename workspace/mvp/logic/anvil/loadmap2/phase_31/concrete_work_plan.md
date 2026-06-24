# Phase31 Concrete Work Plan

Date: 2026-06-23 JST

Status: completed / closed_proven

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and keep them out of Phase31 changes.
3. Re-read:
   - `workspace/mvp/logic/anvil/loadmap2/README.md`;
   - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`;
   - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`;
   - `docs/eval/loadmap2-phase19-large-recovery-20260623.md`;
   - `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`.
4. Confirm Phase31 owns only `P17-L001`.

Exit criteria:

- Phase31 scope is limited to fresh large timeout proof.
- Phase32 remains the only phase that can declare migration completion.

## Step 1: Reconstruct Existing Evidence

1. Read the Phase17 ledger row for `P17-L001`.
2. Read the Phase19 implementation proof and report.
3. Inspect the old large root:
   `eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149`.
4. Confirm that the old root proves ownership/evidence only, not completion.

Exit criteria:

- `blocking_ledger.md` records the exact old proof and why it is insufficient
  for Phase31.
- `source_alignment_matrix.md` maps evidence producers and reports to their
  role in Phase31.

## Step 2: Decide Proof Route

1. Inspect `scripts/eval_agent_slice.sh` and `scripts/eval_large_tasks.sh`.
2. Determine whether a true no-timeout proof can be expressed today.
3. If not, choose the smallest eval-only representation:
   - add explicit `--no-timeout`; or
   - implement `--timeout-secs 0` as no subprocess timeout.
4. Record the route before making changes or running large eval.

Exit criteria:

- The proof mode is explicit and observable.
- No runtime behavior or hidden continuation is introduced.

## Step 3: Produce Closed-Proven Proof

1. Build or identify the binary:
   `cargo build --release`.
2. Run a fresh large root using the selected no-timeout proof mode, for
   example:

   ```bash
   scripts/eval_large_tasks.sh \
     --runs 1 \
     --out eval/runs/loadmap2-phase31-large-non-timeboxed \
     --binary target/release/commandagent \
     --provider ollama \
     --model <local-model> \
     --no-timeout
   ```

3. Recheck the root:

   ```bash
   python3 scripts/eval_report.py <large-root> \
     --cases-dir eval/cases/large \
     --recheck
   ```

4. Run broad sign-off with current smoke/focused roots and the fresh large
   root.
5. Mark `P17-L001` `closed_proven` only if the root completes and the
   sign-off has no unowned large findings.

Exit criteria:

- Fresh root exists.
- Recheck summary exists.
- Broad sign-off status is recorded.
- `P17-L001` has row-specific completion proof.

## Step 4: Update Phase-local Artifacts

1. Update `source_alignment_matrix.md`.
2. Update `row_closure_matrix.md`.
3. Update `blocking_ledger.md`.
4. Update `reconciliation.md`.
5. Update `focused_worklist.md` or record that no focused proof is used.
6. Add `implementation_report.md`.

Exit criteria:

- Every artifact agrees on the chosen path and row disposition.
- No artifact claims final migration completion.

## Step 5: Update Project Docs

1. Update `docs/evaluation.md` if no-timeout proof mode is added.
2. Update roadmap files:
   - `workspace/mvp/logic/anvil/loadmap2/README.md`;
   - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`;
   - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`.
3. Update coverage docs only if the final proof state changes migration
   interpretation.

Exit criteria:

- Phase31 state is consistent across roadmap files.
- Phase32 handoff is explicit.

## Step 6: Verification

Run the smallest checks for the chosen path:

1. Always:

   ```bash
   git diff --check
   ```

2. If eval scripts change:

   ```bash
   python3 tests/test_eval_report.py
   python3 tests/test_eval_signoff.py
   scripts/eval_large_tasks.sh --dry-run --out /tmp/commandagent-phase31-large-dry-run --runs 1
   ```

3. If a fresh large root is produced:

   ```bash
   python3 scripts/eval_report.py <large-root> --cases-dir eval/cases/large --recheck
   python3 scripts/eval_signoff.py --require-recheck \
     --root smoke=<smoke-root> \
     --root focused=<focused-root> \
     --root focused-fixture=<fixture-root> \
     --root large=<large-root>
   ```

Exit criteria:

- Verification matches the chosen row disposition.
- No new unknown/raw large finding appears without a ledger row.

## Step 7: Exit Review

1. Confirm `P17-L001` is `closed_proven`.
2. Confirm no large timeout is treated as success.
3. Confirm Phase32 receives pure completion proof.
4. Confirm no runtime or provider policy changed.

Exit criteria:

- Phase31 can close its assigned blocker without claiming final migration
  completion.

## Plan Review Result

Review changes incorporated:

- Added an explicit no-timeout support decision because the current
  `--timeout-secs` path is still a timebox.
- Removed the external-limitation completion branch after direction was set to
  fresh large `closed_proven` proof.
