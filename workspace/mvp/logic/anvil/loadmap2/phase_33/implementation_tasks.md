# Phase33 Implementation Tasks

Date: 2026-06-23 JST

Status: implemented / reviewed

## Phase Admission

- [x] Confirm Phase33 owns only eval/report recheck projection.
- [x] Confirm Phase34 owns raw diagnostic classification.
- [x] Confirm Phase35 owns setup/profile/dev-server/readiness contract
  connection.
- [x] Confirm Phase36 owns large real-LLM blocker ownership.
- [x] Record current dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.

## Evidence Inventory

- [x] Read:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/focused_worklist.md`;
  - `eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236/recheck_summary.tsv`.
- [x] Build a Phase33-only failure inventory with:
  - case id;
  - matrix row;
  - expected field;
  - observed field;
  - recorded meta value;
  - recorded `fixture_fields` value;
  - whether the failure is projection-caused or belongs to a later phase.
- [x] Exclude from Phase33 any failure whose expected behavior requires
  runtime behavior, setup/profile connection, raw diagnostic sign-off
  admission, large behavior, or root admission changes.

## Projection Design

- [x] Define deterministic recheck field precedence:
  1. `fixture_fields`;
  2. explicit `meta.json` observation fields;
  3. parsed failure evidence;
  4. derived defaults.
- [x] Identify the smallest code boundary for this precedence, preferably in
  `scripts/eval_report.py` or a small helper used by recheck.
- [x] Keep `scripts/eval_failure_observation.py` deterministic and
  side-effect-free.
- [x] Do not add case-id-specific branches unless a reviewed table proves a
  case is a fixture-spec outlier and not a shared projection rule.

## Implementation Tasks

- [x] Add or adjust a recheck projection helper that prepares observation input
  before `normalize_observation`.
- [x] Ensure explicit fixture/meta fields are not blanked or overwritten by
  generic derived values.
- [x] Preserve lifecycle equivalence rules already tested in
  `eval_case_schema.focused_assertions`.
- [x] Add regression tests for at least:
  - explicit-stop projection preservation;
  - completion/evidence status preservation;
  - evidence-binding failure preservation;
  - attempt outcome preservation;
  - verifier/step-policy terminal preservation.
- [x] Re-run focused recheck and record remaining failures.
- [x] Split every remaining focused assertion failure into:
  - Phase34 raw diagnostic;
  - Phase35 setup/profile/dev-server/readiness;
  - later non-Phase33 owner;
  - or Phase33 bug still open.

## Documentation Tasks

- [x] Update `docs/evaluation.md` if the projection precedence becomes a public
  eval-report contract.
- [x] Add `phase_33/implementation_report.md` after implementation.
- [x] Update `phase_32/recovery_task_ledger.md` only with measured counts after
  recheck.
- [x] Do not change Phase32 final decision to complete.

## Verification

- [x] Run:

  ```bash
  python3 tests/test_eval_report.py
  ```

- [x] Run:

  ```bash
  python3 -m py_compile scripts/eval_report.py scripts/eval_failure_observation.py scripts/eval_case_schema.py
  ```

- [x] Run focused recheck:

  ```bash
  python3 scripts/eval_report.py \
    eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --cases-dir eval/cases/focused/control-recovery \
    --recheck
  ```

- [x] Run `git diff --check`.

## Review Gate

- [x] Verify no focused expected assertion was weakened or removed.
- [x] Verify no runtime/minimal-loop/provider/profile/setup behavior changed.
- [x] Verify no hidden retry, hidden continuation, or provider/model-specific
  branch was added.
- [x] Verify remaining failures are not hidden; they are assigned to Phase34+.
- [x] Verify the implementation is generic over observation fields and proof
  modes, not a pile of case-specific patches.

## Plan Review Result

Review updates applied:

- Added explicit non-Phase33 ownership checks for raw diagnostic, setup/profile,
  large, row-proof, and root-admission blockers.
- Added a failure inventory task before implementation so Phase33 does not
  repeat the Phase32 mistake of claiming closure from an incomplete surface.
- Added field-precedence design before code changes to keep the eval/report
  mechanism small and attributable.
- Added documentation and count-update tasks only after measured recheck
  results, avoiding speculative roadmap edits.
