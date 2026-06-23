# Phase36 Implementation Tasks

Date: 2026-06-24 JST

Status: completed

## Phase Admission

- [ ] Confirm Phase36 owns only large real-LLM blocker ownership and row-level
  large disposition.
- [ ] Confirm Phase35 focused assertions remain closed and are not reopened.
- [ ] Confirm Phase37/38/39 still own row-proof reconciliation, root admission,
  and final closure reporting.
- [ ] Record current dirty files before implementation and avoid unrelated
  changes, especially pre-existing Phase21 edits.

## Evidence Inventory

- [ ] Read:
  - `phase_32/followup_phase_split.md`;
  - `phase_32/recovery_task_ledger.md`;
  - `phase_32/focused_worklist.md`;
  - `phase_35/implementation_report.md`;
  - current large `summary.tsv` and `recheck_summary.tsv`;
  - `scripts/eval_report.py`;
  - `scripts/eval_signoff.py`;
  - `scripts/eval_failure_observation.py`;
  - `scripts/eval_runtime_job_report.py`;
  - `scripts/eval_agent_slice.sh`;
  - large case YAML files under `eval/cases/large`;
  - relevant source helpers in `src/agent/step_runner` only if eval/report
    evidence shows runtime owner/action selection is inconsistent.
- [ ] Re-run current large recheck:

  ```bash
  python3 scripts/eval_report.py \
    eval/runs/current-all-local-llm/large/20260623T204816 \
    --cases-dir eval/cases/large \
    --recheck
  ```

- [ ] Re-run current broad sign-off:

  ```bash
  python3 scripts/eval_signoff.py --require-recheck \
    --root smoke=eval/runs/current-all-local-llm/smoke/20260623T203030 \
    --root focused=eval/runs/current-all-local-llm/focused-control-recovery/20260623T203236 \
    --root large=eval/runs/current-all-local-llm/large/20260623T204816
  ```

- [ ] Build a Phase36 large row inventory with:
  - case id;
  - profile/intent;
  - terminal state;
  - diagnostic code;
  - active job;
  - owner/action;
  - target/role/admission;
  - evidence binding/completion evidence;
  - verifier command;
  - source excerpt availability;
  - tool protocol details;
  - explicit stop reason;
  - disposition candidate.

## Row Disposition Tasks

- [ ] Create a large row ledger for all six current large rows.
- [ ] Assign one disposition per row:
  - `closed_owned_failure`;
  - `implementation_blocker`;
  - `accepted_external_limitation`;
  - `split_forward`.
- [ ] Reject `accepted_external_limitation` unless provider/model throughput,
  network, or environment evidence exists.
- [ ] For every `closed_owned_failure`, record why the row is attributable
  but still failed.
- [ ] For every `implementation_blocker`, record the exact responsible layer
  and required code/doc/test change.
- [ ] For every `split_forward`, name the next phase and closure proof.

## Implementation Tasks

- [ ] Add or adjust verifier failure projection only if current evidence can
  deterministically replace weak diagnostics such as
  `unknown_verifier_failure`.
- [ ] Add or adjust tool-protocol projection so failed tool and missing field
  are visible for large rows when the evidence exists.
- [ ] Add or adjust owner/action consistency checks so rows do not report
  `recovery_owner=source` with `repair_action=correct_tool_protocol` unless
  the row is explicitly tool-protocol-owned.
- [ ] Add or adjust edit-target-not-found handling so stale/missing target
  evidence is distinct from ordinary source implementation failure.
- [ ] Add or adjust explicit-stop large disposition for read-only mutation
  rows with admitted target and structured stop reason.
- [ ] Add large-disposition fields or report sections only if they are
  shared across failure families and remain report-only.
- [ ] Do not add model retries, hidden continuation, provider/model branches,
  implicit setup, or verifier weakening.

## Test Tasks

- [ ] Add Python tests for large row disposition and owner/action consistency
  if eval/report changes are made.
- [ ] Add Python tests for tool-protocol field projection if
  `tool_protocol_failed_tool` or `tool_protocol_missing_field` projection is
  changed.
- [ ] Add Python tests for weak verifier diagnostic replacement if
  `unknown_verifier_failure` projection changes.
- [ ] Add Rust tests only if runtime/recovery owner/action selection changes.
- [ ] Re-run focused tests to ensure Phase35 remains closed:

  ```bash
  python3 tests/test_eval_report.py
  python3 tests/test_eval_signoff.py
  ```

- [ ] Re-run current focused recheck only if shared eval/report logic changes
  could affect focused rows.
- [ ] Re-run current large recheck and broad sign-off.

## Documentation Tasks

- [ ] Update `docs/evaluation.md` if large disposition fields or source-excerpt
  semantics are public eval behavior.
- [ ] Update `eval/README.md` if broad sign-off gains large-disposition
  semantics.
- [ ] Add `phase_36/implementation_report.md` after implementation.
- [ ] Update Phase32 recovery files with measured post-Phase36 results.
- [ ] Add Phase37 handoff notes if a row needs row-to-case reconciliation.

## Verification

- [ ] Run:

  ```bash
  cargo fmt --check
  ```

- [ ] Run:

  ```bash
  cargo test
  ```

- [ ] Run:

  ```bash
  cargo build --release
  ```

- [ ] Run relevant Python checks:

  ```bash
  python3 tests/test_eval_report.py
  python3 tests/test_eval_signoff.py
  python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py scripts/eval_failure_observation.py scripts/eval_runtime_job_report.py scripts/eval_case_schema.py
  bash -n scripts/eval_agent_slice.sh
  ```

- [ ] Run:

  ```bash
  git diff --check
  ```

## Review Gate

- [ ] Verify all six large rows have row-level disposition.
- [ ] Verify no large row is closed as external without provider/model
  throughput, network, or environment evidence.
- [ ] Verify sign-off pass is not treated as task success.
- [ ] Verify owner/action/target/evidence are internally consistent.
- [ ] Verify implementation-quality failures remain visible as failed rows.
- [ ] Verify Phase37 receives proof inputs for adopted rows affected by large
  cases.
- [ ] Verify no hidden retry, provider branch, implicit setup, or verifier
  weakening was added.

## Implementation Closure

Phase36 closure artifacts:

| artifact | status |
| --- | --- |
| `large_row_ledger.md` | created |
| `implementation_report.md` | created |
| `docs/evaluation.md` | updated |
| `eval/README.md` | updated |
| `phase_32/recovery_task_ledger.md` | updated |
| `phase_32/followup_phase_split.md` | updated |

Closure results:

| gate | result |
| --- | --- |
| six large rows have row disposition | pass |
| owner/action/target/evidence internally consistent | pass |
| invalid external limitation used | no |
| focused closure preserved | focused recheck reports `passed_recheck: 82` |
| broad sign-off | `status: pass` |
| runtime/provider/hidden retry change | none |

The unchecked list above is retained as the original reviewed task checklist.
The table in this section records the implemented Phase36 closure state.

## Plan Review Result

Review updates applied:

- Added Phase35 non-regression admission so focused closure is preserved.
- Added a large row disposition vocabulary and rejected generic large closure.
- Added owner/action consistency checks because current rows show mismatched
  `source` owner and `correct_tool_protocol` action.
- Added explicit limits for `accepted_external_limitation`.
- Added documentation and Phase37 handoff tasks to keep later proof
  reconciliation from being lost.
