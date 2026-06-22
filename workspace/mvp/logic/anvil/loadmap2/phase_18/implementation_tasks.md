# Phase 18 Implementation Tasks

## 1. Reconfirm Phase18 Inputs

- [x] Read `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`.
- [x] Read `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`.
- [x] Read
  `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`.
- [x] Confirm Phase18 owns only P17-F001, P17-F002, P17-F003, and P17-F004.
- [x] Confirm S001-S005 are the only current focused sign-off findings.

## 2. Create Focused Recovery Worklist

- [x] Add or update
  `workspace/mvp/logic/anvil/loadmap2/phase_18/focused_worklist.md`.
- [x] For each row, record:
  - current observed behavior;
  - expected behavior;
  - owning layer;
  - suspected module;
  - targeted proof command;
  - closure condition;
  - fallback if the targeted proof fails.

## 3. Reproduce Targeted Findings

- [x] Reproduce `focused-docs-literal-mismatch`.
- [x] Reproduce `focused-nextjs-dependency-setup`.
- [x] Reproduce `focused-nextjs-endpoint-smoke`.
- [x] Reproduce `focused-nextjs-route-integration`.
- [x] Save roots separately from Phase16 roots.
- [x] Generate normal and `--recheck` reports for each targeted root.

## 4. Classify Root Cause Before Editing

For each failing row, classify exactly one primary root cause:

- stale expected assertion;
- eval/report projection bug;
- runtime contract projection bug;
- planning/plan-lint bug;
- setup/profile mapping bug;
- recovery task / active job arbitration bug;
- step policy bug.

If more than one primary root cause is plausible, split the ledger row before
editing runtime behavior.

## 5. Implement Narrow Fixes

- [x] P17-F001 docs literal mismatch:
  - verify whether explicit stop is correct or source repair should be
    admitted;
  - update expected assertion or recovery ownership only after classification.
- [x] P17-F002 Next.js dependency setup:
  - verify whether setup completion expectation is valid in offline focused
    eval;
  - repair setup/profile/recovery fields or expected assertion.
- [x] P17-F003 Next.js endpoint smoke:
  - confirm `plan_lint.invalid_expected_path` extraction removes raw `rc:1`;
  - fix remaining plan-lint/report projection or focused expectation mismatch.
- [x] P17-F004 Next.js route integration:
  - verify route integration vs manifest ownership;
  - fix profile obligation projection, active job selection, or stale
    expected fields.

## 6. Horizontal Regression Checks

- [x] Rerun the full focused control-recovery matrix after targeted fixes.
- [x] Confirm passing Python/Rust/docs/data focused rows do not regress.
- [x] Confirm deterministic fixtures remain assertion-clean.
- [x] Confirm smoke remains clean.

## 7. Update Ledger And Docs

- [x] Update Phase17 blocking ledger statuses for closed Phase18 rows.
- [x] Add `docs/eval/loadmap2-phase18-focused-recovery-<date>.md`.
- [x] Update `docs/evaluation.md` only if focused sign-off semantics change.
- [x] Update `docs/known-limitations.md` only if an accepted focused
  limitation remains.

## 8. Verification

Run:

```bash
cargo fmt --check
cargo test
cargo build --release
bash scripts/eval_smoke.sh
```

Focused proof:

```bash
bash scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase18-focused-local-llm \
  --runs 1 \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --binary target/release/commandagent \
  --timeout-secs 900
```

Reports:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase18-focused-local-llm/<root> \
  --cases-dir eval/cases/focused/control-recovery

python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase18-focused-local-llm/<root> \
  --cases-dir eval/cases/focused/control-recovery \
  --recheck
```

Sign-off check:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=<latest-smoke-root> \
  --root focused=<phase18-focused-root> \
  --root focused-fixture=<latest-fixture-root> \
  --root large=<phase16-or-latest-large-root>
```

Phase18 does not require the large findings to pass, but the sign-off output
must no longer contain focused findings S001-S005.

## 9. If Verification Fails

- [x] Same finding remains: keep the row open and create a narrower task.
- [x] New focused finding appears: add it to Phase17 reconciliation or create a
  Phase18 addendum before continuing.
- [x] Same row fails twice after targeted fixes: write a design review note and
  decide whether to split owner/layer.
- [x] Large-only findings remain: keep them assigned to Phase19.

## Completion Criteria

Phase18 is complete only when:

- P17-F001 through P17-F004 are `closed_proven`;
- focused sign-off has no failed expected assertions;
- focused sign-off has no raw undiagnostic `rc:*`;
- full focused matrix report and recheck report are recorded;
- docs/eval Phase18 report exists;
- Phase19 remains the only owner for large evidence/timeout blockers.

## Review Result Reflected

- Added root-cause classification before editing to avoid patching symptoms.
- Added horizontal regression checks across non-Next.js focused rows.
- Kept Phase18 out of large eval ownership and final migration declaration.
- Added failure handling for repeated targeted proof failures.
