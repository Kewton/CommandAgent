# Phase38 Implementation Tasks

Date: 2026-06-24 JST

Status: completed / reviewed

## Phase Admission

- [x] Confirm Phase38 owns only sign-off root admission.
- [x] Confirm Phase37 row-to-case proof reconciliation is closed.
- [x] Confirm Phase39 still owns final migration closure retry/reporting.
- [x] Record current dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.
- [x] Confirm current expected family counts:
  - smoke: 3;
  - focused: 82;
  - large: 6;
  - small: 0, optional.

## Evidence Inventory

- [x] Read:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/current_eval_manifest.md`;
  - `phase_37/row_case_proof_matrix.md`;
  - `phase_37/proof_gap_ledger.md`;
  - `phase_37/implementation_report.md`;
  - `eval/README.md`;
  - `docs/evaluation.md`;
  - `scripts/eval_signoff.py`;
  - `tests/test_eval_signoff.py`.
- [x] Inspect current roots for summary/recheck artifacts:
  - `eval/runs/current-all-local-llm/smoke/20260623T203030`;
  - `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236`;
  - `eval/runs/current-all-local-llm/large/20260623T204816`.
- [x] Re-run current broad sign-off as baseline:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

## Admission Contract Tasks

- [x] Define the final-current root admission contract:
  - required labels;
  - optional labels;
  - duplicate-label rule;
  - duplicate-path rule;
  - expected family identity;
  - expected case counts;
  - required summary/recheck artifacts;
  - admission finding fields.
- [x] Decide where expected current family counts live:
  - prefer a small deterministic constant or manifest structure in
    eval/sign-off scope for Phase38;
  - avoid coupling to runtime/profile/provider code;
  - leave room for future manifest-file input.
- [x] Define accepted output fields:
  - `root_admission_status`;
  - `root_admission_reason`;
  - `admitted_roots`;
  - `rejected_roots`;
  - `family_case_counts`;
  - `current_case_coverage`.

## Implementation Tasks

- [x] Add or extend a root-admission helper in `scripts/eval_signoff.py`, or a
  focused helper imported by that script.
- [x] Reject duplicate labels before evaluating row outcomes.
- [x] Reject duplicate paths under different labels for final-current sign-off.
- [x] Reject missing `smoke`, `focused`, or `large` roots.
- [x] Allow absent `small` only while the expected small count is zero.
- [x] Validate root family identity from case ids, root path hints, or summary
  contents.
- [x] Validate `--require-recheck` roots have `recheck_summary.tsv` where
  required.
- [x] Validate admitted current roots cover 91 cases with the expected
  per-family counts.
- [x] Emit admission findings before normal sign-off findings.
- [x] Ensure normal current root bundle still passes.
- [x] Do not change row outcome interpretation except where root admission
  blocks interpretation.

## Test Tasks

- [x] Add tests in `tests/test_eval_signoff.py` for:
  - valid current root bundle admission;
  - duplicate label rejection;
  - duplicate path under different labels rejection;
  - missing required family rejection;
  - stale/historical smaller bundle rejection;
  - optional `small` omission when expected count is zero;
  - `--require-recheck` missing recheck artifact rejection.
- [x] Run:

  ```bash
  python3 tests/test_eval_signoff.py
  ```

- [x] Run:

  ```bash
  python3 -m py_compile scripts/eval_signoff.py
  ```

- [x] Run current broad sign-off and at least one negative admission command.
- [x] Run `git diff --check`.
- [x] Confirm Rust tests are not required because no Rust/runtime code was
  touched:

  ```bash
  cargo fmt --check
  cargo test
  cargo build --release
  ```

## Documentation Tasks

- [x] Update `eval/README.md` with final-current admission requirements.
- [x] Update `docs/evaluation.md` if root admission becomes public eval
  behavior.
- [x] Add `root_admission_report.md` after implementation.
- [x] Add `implementation_report.md` after implementation.
- [x] Update `phase_32/recovery_task_ledger.md` to record exit gate item 1
  closure.
- [x] Update `phase_32/followup_phase_split.md` to mark Phase38 closed.

## Review Gate

- [x] Verify root admission fails closed before row outcome interpretation.
- [x] Verify duplicate labels and duplicate paths are distinct findings.
- [x] Verify historical root bundles cannot satisfy final-current proof.
- [x] Verify the current root bundle admits 3 smoke, 82 focused, and 6 large
  cases.
- [x] Verify `small` is documented as optional only for zero-case current
  manifest state.
- [x] Verify Phase38 does not declare final migration completion.
- [x] Verify Phase39 receives the admitted root proof.
- [x] Verify no hidden retry, provider branch, implicit setup, or verifier
  weakening is added.

## Plan Review Result

Review updates applied:

- Added explicit expected counts so root admission is tied to the current eval
  manifest rather than to a vague "current roots" phrase.
- Added both positive and negative test requirements to prevent another
  accidental duplicated-root acceptance.
- Required admission output fields for observability.
- Kept final migration reporting out of Phase38 and assigned to Phase39.
- Restricted implementation to eval/sign-off boundaries.
