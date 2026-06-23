# Phase37 Implementation Tasks

Date: 2026-06-24 JST

Status: completed

## Phase Admission

- [ ] Confirm Phase37 owns row-to-case proof reconciliation only.
- [ ] Confirm Phase35 focused assertions remain closed.
- [ ] Confirm Phase36 large dispositions remain closed.
- [ ] Confirm Phase38 still owns sign-off root admission.
- [ ] Confirm Phase39 still owns final closure retry/reporting.
- [ ] Record current dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.

## Evidence Inventory

- [ ] Read:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/focused_worklist.md`;
  - `phase_32/current_eval_manifest.md`;
  - `docs/eval/legacy-control-stack-coverage-20260621.md`;
  - `workspace/mvp/logic/anvil/loadmap2/README.md`;
  - `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`;
  - `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`;
  - Phase22-Phase36 implementation reports where row proof is referenced;
  - current `summary.tsv` and `recheck_summary.tsv` files for smoke, focused,
    and large current roots.
- [ ] Rebuild or inspect current root summaries:

  ```bash
  python3 scripts/eval_report.py \
    eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --cases-dir eval/cases/focused/control-recovery \
    --recheck

  python3 scripts/eval_report.py \
    eval/runs/current-all-local-llm/large/20260623T204816 \
    --cases-dir eval/cases/large \
    --recheck
  ```

- [ ] Re-run current broad sign-off for regression context:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

## Matrix Construction Tasks

- [ ] Create `row_case_proof_matrix.md`.
- [ ] Add one row for every coverage ID C01-C54.
- [ ] Include at least:
  - coverage id;
  - source responsibility;
  - adoption decision;
  - current status;
  - owning phase;
  - proof mode;
  - current eval case id when present;
  - matrix row when present;
  - proof root;
  - current recheck status;
  - proof authority;
  - disposition;
  - gap reason or closure note.
- [ ] Mark current eval cases not mapped to a C row as supplemental only when
  their owner/proof purpose is explicit.
- [ ] Reject row-order-only mapping; use stable coverage ids and stable case
  ids.

## Proof Gap Tasks

- [ ] Create `proof_gap_ledger.md`.
- [ ] Add every row with missing current proof, ambiguous proof authority, or
  historical-only proof.
- [ ] For each gap, record:
  - coverage id or case id;
  - owner layer;
  - missing proof;
  - current evidence;
  - proposed destination phase;
  - proof command;
  - closure condition.
- [ ] Split root-admission gaps to Phase38.
- [ ] Split final-report/sign-off interpretation gaps to Phase39.
- [ ] Do not split a vague blocker; split only with exact row/case/proof.

## Reconciliation Tasks

- [ ] Reconcile `current_eval_manifest.md` 91-case surface against the matrix.
- [ ] Reconcile the 44 historically omitted current cases against current
  proof rows.
- [ ] Reconcile Phase22-Phase36 implementation reports against coverage rows.
- [ ] Confirm excluded rows C46-C54 have explicit rationale and are not counted
  as proof gaps.
- [ ] Confirm large rows remain `closed_owned_failure` and are not recast as
  external limitations.
- [ ] Confirm focused rows remain `passed_recheck: 82` after any report-only
  changes.

## Implementation Tasks

- [ ] Prefer documentation/eval-ledger changes only.
- [ ] Add a small deterministic helper script only if manual matrix upkeep is
  clearly error-prone and the helper can stay read-only/report-only.
- [ ] If a helper is added, keep it independent of provider, profile runtime,
  and minimal loop behavior.
- [ ] If proof fields are missing from eval reports, add report-only projection
  only from already available deterministic evidence.
- [ ] Do not add retry, hidden continuation, provider/model branches, implicit
  setup, or verifier weakening.

## Test Tasks

- [ ] If only Markdown ledgers change, run:

  ```bash
  git diff --check
  ```

- [ ] If eval report code changes, run:

  ```bash
  python3 tests/test_eval_report.py
  python3 tests/test_eval_signoff.py
  python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py scripts/eval_runtime_job_report.py
  ```

- [ ] If Rust runtime/recovery code changes unexpectedly, run:

  ```bash
  cargo fmt --check
  cargo test
  cargo build --release
  ```

- [ ] Always re-run the current broad sign-off after any eval/report code
  change.

## Documentation Tasks

- [ ] Add `implementation_report.md` after implementation.
- [ ] Update `phase_32/recovery_task_ledger.md` for P32-R009.
- [ ] Update `phase_32/followup_phase_split.md` if Phase37 creates Phase38/39
  handoff rows.
- [ ] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only if a
  coverage proof reference or row state is corrected.
- [ ] Update `docs/evaluation.md` or `eval/README.md` only if the proof matrix
  becomes a reusable eval workflow.

## Review Gate

- [ ] Verify C01-C54 all appear in `row_case_proof_matrix.md`.
- [ ] Verify C01-C45 adopted rows do not close on historical-only roots.
- [ ] Verify C46-C54 excluded rows include rationale.
- [ ] Verify every current eval case is mapped or explicitly supplemental.
- [ ] Verify every proof gap has owner, phase, proof command, and closure
  condition.
- [ ] Verify Phase38/39 handoff is limited to root admission or final closure,
  not unresolved row accounting.
- [ ] Verify broad sign-off pass is not described as migration completion.
- [ ] Verify no hidden retry, provider branch, implicit setup, or verifier
  weakening is added.

## Plan Review Result

Review updates applied:

- Added required rows for all C01-C54, including excluded rows, to avoid
  partial accounting.
- Added explicit proof-gap ledger requirements so Phase37 cannot close by
  narrative summary.
- Added current-manifest reconciliation for the 44 omitted cases that caused
  the Phase32 recovery.
- Added limits on helper scripts: report-only, deterministic, provider-free,
  and runtime-free.
- Added Phase38/39 split rules so root admission and final closure remain in
  their responsible phases.

## Implementation Closure

Phase37 closure artifacts:

| artifact | status |
| --- | --- |
| `row_case_proof_matrix.md` | created |
| `proof_gap_ledger.md` | created |
| `implementation_report.md` | created |
| `phase_32/recovery_task_ledger.md` | updated |
| `phase_32/followup_phase_split.md` | updated |

Closure results:

| gate | result |
| --- | --- |
| C01-C54 all represented | pass |
| C01-C45 adopted rows avoid historical-only closure | pass |
| C46-C54 excluded rows have rationale | pass |
| current 91 cases mapped or supplemental | pass |
| open proof gaps | 0 |
| Phase38/39 handoff limited to root admission/final closure | pass |
| runtime/provider/minimal-loop change | none |

The unchecked list above is retained as the original reviewed task checklist.
The table in this section records the implemented Phase37 closure state.
