# Phase38 Concrete Work Plan

Date: 2026-06-24 JST

Status: completed / reviewed

## Step 0: Preflight

1. Run `git status --short --untracked-files=all`.
2. Record unrelated dirty files and leave them untouched.
3. Confirm Phase38 scope from:
   - `phase_32/followup_phase_split.md`;
   - `phase_32/recovery_task_ledger.md`;
   - `phase_37/proof_gap_ledger.md`.
4. Confirm current roots:
   - smoke: `eval/runs/current-all-local-llm/smoke/20260623T203030`;
   - focused: `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236`;
   - large: `eval/runs/current-all-local-llm/large/20260623T204816`.

Exit criteria:

- Phase38 owns admission of sign-off inputs only.
- Phase39 remains responsible for final migration decision reporting.

## Step 1: Inspect Current Sign-off Boundary

1. Read `scripts/eval_signoff.py`.
2. Locate root parsing and any current validation.
3. Read `tests/test_eval_signoff.py`.
4. Identify the smallest insertion point for admission validation before row
   outcome checks.
5. Inspect `eval/README.md` and `docs/evaluation.md` for current sign-off
   documentation.

Exit criteria:

- The implementation target is a single eval/sign-off boundary, not runtime.
- Existing sign-off semantics are understood before changing them.

## Step 2: Define Root Admission Data

1. Define expected final-current families:
   - `smoke`: required, expected 3 cases;
   - `focused`: required, expected 82 cases;
   - `large`: required, expected 6 cases;
   - `small`: optional, expected 0 cases.
2. Define accepted labels and aliases.
3. Define rejected labels for final-current proof, including duplicated
   supplemental labels that point to an already-admitted root.
4. Define root identity:
   - normalized root path;
   - root label;
   - case ids from summary/recheck file;
   - family inferred from label and case id shape.

Exit criteria:

- Expected counts and family labels can be tested without running a model.
- Future family additions can extend a manifest-like structure.

## Step 3: Implement Admission Gate

1. Add root admission validation before sign-off finding interpretation.
2. Reject:
   - duplicate labels;
   - duplicate root paths under different labels;
   - missing required families;
   - wrong family under a label;
   - missing `recheck_summary.tsv` when `--require-recheck` is used;
   - current-case coverage mismatch.
3. Produce structured admission findings with:
   - status;
   - reason;
   - label;
   - root path;
   - expected count;
   - observed count.
4. Ensure admitted current roots continue to execute normal sign-off checks.

Exit criteria:

- Bad root bundles fail before normal sign-off can be interpreted.
- Good current root bundle still returns `status: pass`.

## Step 4: Add Tests

1. Add positive test for the current root bundle.
2. Add negative tests:
   - duplicate label;
   - duplicate path under different labels;
   - missing required family;
   - historical smaller bundle;
   - missing recheck artifact under `--require-recheck`.
3. Add optional-family test for zero-case `small`.
4. Keep tests deterministic and file-local; do not require model execution.

Exit criteria:

- `python3 tests/test_eval_signoff.py` passes.
- Root-admission failure messages are stable enough for debugging.

## Step 5: Documentation And Reports

1. Update `eval/README.md` with final-current root admission requirements.
2. Update `docs/evaluation.md` if the admission gate is user-visible behavior.
3. Add `phase_38/root_admission_report.md`.
4. Add `phase_38/implementation_report.md`.
5. Update `phase_32/recovery_task_ledger.md` for exit gate item 1 closure.
6. Update `phase_32/followup_phase_split.md` to mark Phase38 closed.

Exit criteria:

- A reader can tell which root bundle was admitted and why.
- Phase39 has a concrete admitted-root proof to consume.

## Step 6: Verification

Run targeted checks:

```bash
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_signoff.py
git diff --check
```

Run current positive sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Run at least one negative admission check:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
  --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root focused-fixture=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
  --root large=eval/runs/current-all-local-llm/large/20260623T204816
```

Expected result for the negative check: nonzero exit with duplicate root/path
admission finding.

Run Rust checks only if Rust code is touched:

```bash
cargo fmt --check
cargo test
cargo build --release
```

Exit criteria:

- All applicable positive checks pass.
- Negative root admission check fails for the intended deterministic reason.

## Step 7: Exit Review

Before closing Phase38, verify:

- duplicate labels fail closed;
- duplicate paths fail closed for final-current sign-off;
- missing required families fail closed;
- historical smaller bundles fail closed;
- current roots admit with 3 smoke, 82 focused, and 6 large cases;
- admission findings are visible;
- Phase38 does not claim migration completion;
- Phase39 handoff is documented;
- no hidden retry, provider/model branch, implicit setup, or verifier
  weakening was added.

## Plan Review Result

Review findings incorporated:

- Split inspection, data definition, implementation, tests, docs, and exit
  review into separate steps to avoid jumping directly to code.
- Required one positive current-root proof and one negative duplicated-root
  proof in the verification plan.
- Made root admission fail before row interpretation, preserving honest
  sign-off behavior.
- Kept the expected family counts close to eval/sign-off and left room for a
  future manifest input.
- Preserved Phase39 as the only final closure reporting phase.

## Implementation Result

Phase38 was implemented in the eval/sign-off boundary:

- root admission now runs before row finding interpretation;
- current roots admit with 3 smoke, 82 focused, and 6 large cases;
- duplicate focused root paths fail closed with `duplicate_root_path`;
- stale smaller focused roots fail with case coverage mismatch;
- `small` remains optional while its expected count is zero;
- admission evidence is rendered in sign-off output.

Verification completed:

```text
python3 tests/test_eval_signoff.py
python3 -m py_compile scripts/eval_signoff.py
python3 scripts/eval_signoff.py --require-recheck --root smoke=... --root focused=... --root large=...
python3 scripts/eval_signoff.py --require-recheck --root smoke=... --root focused=... --root focused-fixture=<same-focused-root> --root large=...
git diff --check
```

Rust checks were not required because no Rust/runtime files were changed.
