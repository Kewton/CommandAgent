# Phase32 Implementation Tasks

Date: 2026-06-23 JST

Status: completed / reviewed

## Task Checklist

### 1. Coverage Authority Audit

- [x] Read `docs/eval/legacy-control-stack-coverage-20260621.md`.
- [x] Verify C01-C45 are `Implemented`.
- [x] Verify C46-C54 final exclusions are intentional and justified.
- [x] Verify there are no adopted `Partial` rows.
- [x] Verify there are no adopted `Missing` rows.
- [x] Record counts in Phase32 `implementation_report.md`.

### 2. Phase Ledger Audit

- [x] Check Phase22 through Phase31 `implementation_report.md`.
- [x] Confirm each assigned row is closed:
  - Phase22: C01-C03
  - Phase23: C04-C06
  - Phase24: C07-C10
  - Phase25: C11-C12
  - Phase26: C13-C20
  - Phase27: C21-C32
  - Phase28: C33
  - Phase29: C34-C44
  - Phase30: C49-C50
  - Phase31: P17-L001
- [x] Confirm no phase-local ledger has an open blocker.
- [x] Confirm any excluded row has rationale and does not hide an adopted
  responsibility.

### 3. Current Issue Map Closure

- [x] Update KI-011 from `open` to final disposition if exit gates pass.
- [x] Confirm KI-001 through KI-010 remain closed and consistent.
- [x] Ensure Phase32 is recorded as final closure, not a broad implementation
  phase.

### 4. Final Broad Sign-off

- [x] Run final sign-off with the current accepted roots.
- [x] Require recheck where applicable.
- [x] Confirm command exits zero.
- [x] No final sign-off finding appeared; no additional blocker mapping was
  required.

Expected command shape:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --root large=eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624
```

### 5. Final Report

- [x] Create or update `docs/eval/loadmap2-final-migration-decision-20260623.md`.
- [x] Include final decision:
  - `migration_complete`,
  - `migration_complete_with_explicit_exclusions`, or
  - `migration_not_complete`.
- [x] List migrated rows, excluded rows, proof roots, sign-off command, and
  remaining limitations if any.
- [x] Avoid the phrase "Anvil migration complete" unless the exit gate passed.

### 6. Roadmap And Documentation Updates

- [x] Update loadmap2 `README.md` Phase32 row and final checklist.
- [x] Update `recovery_plan.md` Phase32 exit result.
- [x] Update `current_issue_phase_map.md` KI-011 and Phase32 status.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only if
  reconciliation finds stale counts or state text.
- [x] Architecture/philosophy docs did not require changes because the final
  report matches the current minimal-loop contract-recovery architecture.

### 7. Verification

- [x] `python3 scripts/eval_signoff.py --require-recheck ...`
- [x] `python3 tests/test_eval_signoff.py`
- [x] `python3 tests/test_eval_report.py`
- [x] `python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py`
- [x] `git diff --check`
- [x] Confirm whether non-doc code changes were made. If they are made, also
  run:
  - `cargo fmt --check`
  - `cargo test`
  - `cargo build --release`

No non-doc code changes were made, so cargo checks were not required for the
Phase32 closure commit.

## Review Gate Before Implementation

- [x] Every task maps to a closure row in `row_closure_matrix.md`.
- [x] No task relies on CI alone.
- [x] No task depends on weakening sign-off gates.
- [x] No task introduces new runtime behavior.
- [x] Failure handling creates ledger rows instead of prose-only follow-up.

## Plan Review Result

The task list was reviewed for missing final-closure requirements.

Review changes applied:

- added a separate coverage authority audit before roadmap edits;
- added explicit KI-011 transition work;
- required a final report with one of the three allowed decisions;
- required test/report checks even if only docs change;
- made cargo checks conditional on non-doc code changes to avoid unnecessary
  runtime churn while still preserving safety.
