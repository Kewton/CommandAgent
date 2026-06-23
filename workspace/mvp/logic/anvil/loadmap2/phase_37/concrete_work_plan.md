# Phase37 Concrete Work Plan

Date: 2026-06-24 JST

Status: completed

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and leave them untouched.
3. Confirm Phase37 scope from:
   - `phase_32/followup_phase_split.md`;
   - `phase_32/recovery_task_ledger.md`;
   - `phase_32/current_eval_manifest.md`.
4. Confirm the current root set:
   - smoke: `eval/runs/current-all-local-llm/smoke/20260623T203030`;
   - focused: `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236`;
   - large: `eval/runs/current-all-local-llm/large/20260623T204816`.

Exit criteria:

- Phase37 owns row-to-case proof reconciliation only.
- Phase38 and Phase39 remain responsible for root admission and final closure.

## Step 1: Build Coverage Row Inventory

1. Parse or manually extract C01-C54 from
   `docs/eval/legacy-control-stack-coverage-20260621.md`.
2. For each row, record:
   - source responsibility;
   - adoption decision;
   - current implementation status;
   - owning phase;
   - closure proof currently cited.
3. Flag rows whose cited proof is historical-only.
4. Flag rows whose cited proof references a current eval case.

Exit criteria:

- The inventory contains exactly C01-C54.
- Excluded rows are visible and not silently dropped.

## Step 2: Build Current Case Inventory

1. Extract current cases from:
   - `phase_32/current_eval_manifest.md`;
   - current focused `recheck_summary.tsv`;
   - current large `recheck_summary.tsv`;
   - current smoke summary.
2. For every current case, record:
   - case id;
   - matrix row;
   - proof family;
   - current terminal state;
   - current recheck assertion status;
   - proof root.
3. Identify current cases absent from the historical Phase32 sign-off roots.

Exit criteria:

- The 91 current cases have an inventory entry or a documented exclusion such
  as no-case small family.
- The 44 historically omitted current cases remain explicitly visible.

## Step 3: Create Row-to-case Proof Matrix

1. Create `phase_37/row_case_proof_matrix.md`.
2. Add one matrix row per C01-C54 coverage ID.
3. For rows with direct current cases, bind:
   - coverage id;
   - current eval case id;
   - matrix row;
   - proof root;
   - recheck status.
4. For rows without direct current cases, bind:
   - unit or fixture proof;
   - historical proof root if still useful as regression evidence;
   - rationale for why no current case is required.
5. Add a disposition:
   - `current_eval_proven`;
   - `unit_or_fixture_proven`;
   - `excluded_with_rationale`;
   - `split_forward`;
   - `proof_gap`.

Exit criteria:

- No adopted row is closed by historical evidence only when a current case
  exists.
- Any row with `proof_gap` is carried to Step 4.

## Step 4: Create Proof Gap Ledger

1. Create `phase_37/proof_gap_ledger.md`.
2. Add every `proof_gap` row and every unmapped current case.
3. Classify the gap:
   - missing current case binding;
   - missing proof root;
   - missing recheck result;
   - coverage row ambiguity;
   - root admission problem;
   - final report/sign-off interpretation problem.
4. Assign destination:
   - Phase37 if it is a matrix repair;
   - Phase38 if it is root admission;
   - Phase39 if it is final report/sign-off interpretation.
5. Add proof command and closure condition.

Exit criteria:

- No gap is left as "investigate later".
- Every split-forward row names a responsible phase and proof command.

## Step 5: Reconcile Phase32 Recovery Ledger

1. Update P32-R009 in `phase_32/recovery_task_ledger.md`.
2. If all adopted rows are reconciled, mark P32-R009 completed with references
   to the matrix and gap ledger.
3. If any row is split forward, keep P32-R009 open only for those named rows
   and add Phase38/39 ownership.
4. Do not change P32-R005/P32-R006/P32-R008 unless a measured recheck result
   changed.

Exit criteria:

- P32-R009 status matches the actual matrix result.
- Phase32 recovery files do not imply migration completion.

## Step 6: Documentation And Handoff

1. Add `phase_37/implementation_report.md` during implementation.
2. Update `phase_32/followup_phase_split.md` if Phase37 closes or splits its
   assigned responsibility.
3. Update coverage docs only for real row-state/proof-reference corrections.
4. If a reusable proof matrix process is introduced, document it in
   `docs/evaluation.md` or `eval/README.md`.

Exit criteria:

- Future Phase38/39 work can consume Phase37 output without reinterpreting
  the coverage table manually.
- Current proof authority is clear from docs, not just commit history.

## Step 7: Verification

For documentation-only implementation, run:

```bash
git diff --check
```

If eval report helper code is added, also run:

```bash
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py scripts/eval_runtime_job_report.py
```

If runtime code is changed unexpectedly, also run:

```bash
cargo fmt --check
cargo test
cargo build --release
```

After any eval/report behavior change, rerun:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Exit criteria:

- Checks pass for the files touched.
- Any failing sign-off row is mapped to a row/case/proof entry or a later phase.

## Step 8: Exit Review

Before closing Phase37, verify:

- C01-C54 are all represented.
- C01-C45 adopted rows have current or accepted proof.
- C46-C54 excluded rows have rationale.
- All 91 current eval cases are mapped or explicitly supplemental.
- The 44 historically omitted current cases are no longer invisible.
- P32-R009 status is updated.
- Any split-forward row has owner, target, proof command, and closure
  condition.
- Broad sign-off is not treated as migration completion.
- No hidden retry, provider/model branch, implicit setup, or verifier
  weakening was added.

## Plan Review Result

Review findings incorporated:

- Added a separate Step 1 and Step 2 so coverage rows and current cases cannot
  be conflated.
- Required one row for every C01-C54 coverage entry, including excluded rows.
- Required proof-gap ledger creation before Phase37 can update P32-R009.
- Limited split-forward to exact root admission or final-closure gaps assigned
  to Phase38/39.
- Kept verification proportional: docs-only work uses diff checks, while
  eval/report or runtime changes trigger the relevant test suites.

## Execution Result

Phase37 executed the plan as documentation/eval-ledger work only.

| step | result |
| --- | --- |
| Step 0 | Preflight recorded one unrelated dirty file: `phase_21/implementation_report.md`; it was left untouched. |
| Step 1 | C01-C54 were represented in `row_case_proof_matrix.md`. |
| Step 2 | Current eval surface was reconciled as 3 smoke, 82 focused, and 6 large cases. |
| Step 3 | Row-to-case proof matrix was created with dispositions for every coverage row. |
| Step 4 | Proof gap ledger was created with zero open proof gaps and exact Phase38/39 handoffs. |
| Step 5 | P32-R009 was marked completed by Phase37. |
| Step 6 | Phase32 follow-up split was updated to record Phase37 closure. |
| Step 7 | Documentation-only checks passed. |
| Step 8 | No hidden retry, provider/model branch, implicit setup, or verifier weakening was added. |

Phase38 still owns sign-off root admission. Phase39 still owns final closure
retry/reporting.
