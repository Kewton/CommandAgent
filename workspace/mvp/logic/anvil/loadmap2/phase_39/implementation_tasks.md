# Phase39 Implementation Tasks

Date: 2026-06-24 JST

Status: completed / reviewed

## Phase Admission

- [x] Confirm Phase39 owns only final closure retry/reporting.
- [x] Confirm Phase33, Phase34, Phase35, Phase36, Phase37, and Phase38 are
  completed and have implementation reports or equivalent closure reports.
- [x] Confirm existing dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.
- [x] Confirm Phase39 may produce `migration_complete`,
  `migration_complete_with_explicit_exclusions`, or `migration_not_complete`,
  but must derive the decision from evidence.

## Evidence Inventory

- [x] Read current source files:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/focused_worklist.md`;
  - `phase_32/current_eval_manifest.md`;
  - `phase_37/row_case_proof_matrix.md`;
  - `phase_37/proof_gap_ledger.md`;
  - `phase_38/root_admission_report.md`;
  - `phase_38/implementation_report.md`;
  - `docs/eval/legacy-control-stack-coverage-20260621.md`;
  - `docs/eval/loadmap2-final-migration-decision-20260623.md`;
  - `docs/migration-progress.md`.
- [x] Re-run final-current sign-off:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

- [x] Record root admission fields:
  - `root_admission_status`;
  - `family_case_counts`;
  - `current_case_coverage`;
  - `status`.
- [x] Confirm focused recheck still has 82 passing assertion rows.
- [x] Confirm large recheck still has 6 `closed_owned_failure` rows and no
  `accepted_external_limitation`.
- [x] Confirm row proof matrix still reports C01-C54 represented and proof
  gaps 0.

## Decision Matrix Tasks

- [x] Create `decision_evidence_matrix.md` with one row per final gate:
  - coverage table final states;
  - excluded surface rationale;
  - Phase32 exit gate items 1-6;
  - current root admission;
  - current broad sign-off;
  - focused assertion closure;
  - large owned-failure closure;
  - row-to-case proof closure;
  - stale historical-root exclusion.
- [x] For each gate, record:
  - evidence file;
  - command if any;
  - observed result;
  - pass/fail;
  - decision impact.
- [x] If any gate fails, stop Phase39 final completion and record
  `migration_not_complete` plus a named blocker.

## Final Report Tasks

- [x] Create `final_closure_report.md` under Phase39.
- [x] Create or update current final decision doc under `docs/eval/`.
- [x] State exactly one final decision.
- [x] Explain the difference between:
  - Anvil control/recovery responsibility migration;
  - large application-generation task success;
  - excluded legacy surfaces.
- [x] Include current final-current roots and sign-off output.
- [x] Include supersession note for
  `docs/eval/loadmap2-final-migration-decision-20260623.md`.

## Roadmap And Status Updates

- [x] Update `workspace/mvp/logic/anvil/loadmap2/README.md`:
  - Phase32 recovery appendix;
  - final migration checklist;
  - Phase39 closure state.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`:
  - Phase32/39 final closure result;
  - remove stale current sign-off failure wording;
  - preserve recovery rules for future final closure work.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`:
  - KI-011 final state;
  - Phase39 proof link;
  - no stale recovery-open statement if closed.
- [x] Update `phase_32/recovery_task_ledger.md`:
  - add Phase39 final decision task;
  - close exit gate item 6 if final report is written.
- [x] Update `phase_32/followup_phase_split.md`:
  - mark Phase39 closed or explicitly not complete with blocker link.
- [x] Update `docs/migration-progress.md`.
- [x] Update coverage-table appendix if it still records the Phase32 recovery
  state as open.

## Verification Tasks

- [x] Run:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

- [x] Run:

  ```bash
  python3 tests/test_eval_signoff.py
  python3 -m py_compile scripts/eval_signoff.py
  git diff --check
  ```

- [x] Run Rust checks only if implementation unexpectedly touches Rust/runtime
  code:

  ```bash
  cargo fmt --check
  cargo test
  cargo build --release
  ```

## Review Gate

- [x] Verify final decision is evidence-derived and exactly one value.
- [x] Verify no final report relies on historical roots alone.
- [x] Verify broad sign-off pass is not treated as the only proof.
- [x] Verify large failed rows are not claimed as successful applications.
- [x] Verify excluded rows remain explicit and justified.
- [x] Verify stale Phase32 wording is removed or superseded.
- [x] Verify any failure causes `migration_not_complete` with a named blocker.
- [x] Verify no hidden retry, runtime orchestration, provider/model branch,
  implicit setup, or verifier weakening is added.

## Plan Review Result

Review updates applied:

- Added a decision matrix requirement so Phase39 cannot close from prose alone.
- Added stale roadmap/status cleanup because existing files still say current
  sign-off fails.
- Added failure fallback to `migration_not_complete` to avoid optimistic
  completion if any proof is missing.
- Kept verification focused on eval/sign-off and docs unless runtime files are
  unexpectedly touched.
- Required exact distinction between migration completion and large task
  success.

## Implementation Result

All task groups were completed by the Phase39 reports and roadmap/doc updates.
The final decision is:

```text
migration_complete_with_explicit_exclusions
```

The decision evidence is recorded in `decision_evidence_matrix.md`, the final
report is recorded in `final_closure_report.md`, and verification evidence is
recorded in `implementation_report.md`.
