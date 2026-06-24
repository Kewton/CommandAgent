# Phase 20 Implementation Tasks

## Execution Status

Phase20 was executed on 2026-06-23. The final decision is
`migration_not_complete`.

Completed outputs:

- `coverage_closure.md`
- `ledger_reconciliation.md`
- `continuation_ledger.md`
- `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`
- Phase20 appendix in
  `docs/eval/legacy-control-stack-coverage-20260621.md`

The optional read-only closure checker was not added because the current
Phase20 outcome is a documentation/evidence reconciliation decision, and the
manual table audit identified unresolved rows without ambiguity.

## 1. Rebaseline Inputs

- [ ] Read `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`.
- [ ] Read `workspace/mvp/logic/anvil/loadmap2/README.md`.
- [ ] Read `workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md`.
- [ ] Read
  `workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md`.
- [ ] Read `docs/eval/legacy-control-stack-coverage-20260621.md`.
- [ ] Read Phase18 and Phase19 final reports.
- [ ] Confirm Phase18 and Phase19 assigned ledger rows are no longer `open`.
- [ ] Capture current `git rev-parse --short HEAD` and dirty flag.

## 2. Coverage Table Audit

- [ ] Count current coverage rows by `Current status` and `Adoption decision`.
- [ ] Identify every row where the final decision cannot be complete:
  - adopted `Partial`;
  - adopted `Missing`;
  - `Partial` adoption decision without final rationale;
  - stale row text contradicted by Phase1-Phase19 reports.
- [ ] For every row currently `Partial`, classify it as:
  - `implemented_with_proof`;
  - `excluded_by_design`;
  - `real_gap`.
- [ ] For every row currently `Missing`, classify it as:
  - `implemented_with_proof`;
  - `excluded_by_design`;
  - `real_gap`.
- [ ] Do not change a row to `Implemented` unless the proof source is named.
- [ ] Do not change a row to `Excluded` unless the design rationale is named.

## 3. Coverage Closure Table

Create a Phase20 closure table with these columns:

- coverage row / source mechanism;
- prior status;
- adoption decision;
- final status candidate;
- evidence source;
- proof command or report;
- reason;
- owner layer;
- final decision impact.

Required output:

- [ ] `workspace/mvp/logic/anvil/loadmap2/phase_20/coverage_closure.md`

## 4. Ledger Reconciliation

- [ ] Verify every Phase17 ledger row has final status:
  - `closed_proven` for pure `migration_complete`; or
  - `blocked_external` with owner/action/evidence and accepted rationale only
    for `migration_complete_with_explicit_exclusions` or
    `migration_not_complete`.
- [ ] Verify no Phase17 ledger row remains `open`.
- [ ] Verify no Phase17 ledger row is closed by CI-only evidence.
- [ ] Verify `blocked_external` is not hiding missing owner/action/evidence.
- [ ] Decide whether `P17-L001` is re-proven as `closed_proven`, explicitly
  accepted/excluded, or keeps the final decision at `migration_not_complete`.
- [ ] Add a Phase20 ledger reconciliation summary.

Required output:

- [ ] `workspace/mvp/logic/anvil/loadmap2/phase_20/ledger_reconciliation.md`

## 5. Final Broad Sign-off

Use the current proof roots unless the coverage audit proves a fresh run is
needed:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

- [ ] Confirm every selected root has `summary.tsv`.
- [ ] Confirm every selected root has `recheck_summary.tsv` or regenerate it.
- [ ] Run the final broad sign-off command.
- [ ] Record stdout, exit status, and root paths in the final report.
- [ ] If sign-off fails, map every finding to a coverage row and ledger row
  before deciding next work.

## 6. Optional Read-only Closure Checker

Add a helper only if manual closure would be error-prone.

Allowed shape:

- read-only;
- deterministic;
- parses coverage/ledger/sign-off artifacts;
- reports unresolved rows;
- does not edit files;
- does not run eval or runtime.

Candidate output:

```bash
python3 scripts/eval_migration_closure.py \
  --coverage docs/eval/legacy-control-stack-coverage-20260621.md \
  --ledger workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md
```

Tasks:

- [ ] Decide whether a helper is needed.
- [ ] If added, create unit tests for adopted `Partial`/`Missing`, excluded
  rows, open ledger rows, and successful closure.
- [ ] Update `docs/evaluation.md` or `docs/architecture.md` if the helper
  becomes part of the documented eval workflow.

## 7. Documentation Updates

- [ ] Update `docs/eval/legacy-control-stack-coverage-20260621.md` with final
  statuses or a final closure appendix.
- [ ] Add `docs/eval/loadmap2-phase20-final-migration-decision-<date>.md`.
- [ ] Update `docs/known-limitations.md` if final accepted exclusions or
  limitations change.
- [ ] Update `docs/architecture.md` only if a new closure checker or final
  decision boundary is added.
- [ ] Update Phase20 README/tasks/plan with the review result and proof status.

## 8. Final Decision Report

The final report must include:

- commit hash and dirty flag;
- coverage table final counts;
- ledger status summary;
- treatment of any `blocked_external` row, including whether it prevents pure
  `migration_complete`;
- final broad sign-off command and result;
- final decision:
  - `migration_complete`;
  - `migration_complete_with_explicit_exclusions`; or
  - `migration_not_complete`;
- if not complete:
  - exact unresolved rows;
  - owner layer;
  - proof command needed;
  - recommended continuation phase/ledger.

Required output:

- [ ] `docs/eval/loadmap2-phase20-final-migration-decision-<date>.md`

## 9. Verification

Minimum checks if only docs/checker scripts change:

- [ ] `python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py`
- [ ] `python3 tests/test_eval_report.py`
- [ ] `python3 tests/test_eval_signoff.py`
- [ ] closure checker tests, if a helper is added
- [ ] final broad sign-off
- [ ] `git diff --check`

If code beyond docs/eval helper scripts changes, also run:

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo build --release`
- [ ] `bash scripts/eval_smoke.sh`

## 10. Failure Handling

- [ ] If coverage rows remain adopted `Partial` or `Missing`, write
  `migration_not_complete` unless they are explicitly excluded with rationale.
- [ ] If final sign-off fails, do not declare migration complete.
- [ ] If a new sign-off finding appears, add it to a continuation ledger with:
  - owner layer;
  - failed contract;
  - suspected module;
  - proof command;
  - closure condition.
- [ ] If the same row lacks proof after review, keep it unresolved rather than
  converting it to a limitation.

## 11. Phase20 Closure Review

Before closing Phase20, answer:

- [ ] Does the coverage table have zero adopted unresolved `Partial`/`Missing`
  rows for a complete decision?
- [ ] Does every excluded row have design rationale?
- [ ] Does every ledger row have a final state?
- [ ] If any ledger row is `blocked_external`, is the final decision
  `migration_complete_with_explicit_exclusions` or `migration_not_complete`
  rather than pure `migration_complete`?
- [ ] Does final broad sign-off exit zero?
- [ ] Does the final report state exactly one decision?
- [ ] If the decision is not complete, are all blockers row-level and owned?

## Review Result Reflected

- Added coverage closure as an explicit deliverable instead of relying on prose.
- Added a separate ledger reconciliation deliverable.
- Tightened `blocked_external` handling so it cannot be mistaken for pure
  migration completion.
- Made a read-only closure checker optional and bounded.
- Required `migration_not_complete` when adopted coverage remains unresolved.
- Kept fresh live eval optional unless the final decision depends on behavior
  not represented in existing roots.

## Closure Review Result

- Coverage table has unresolved adopted `Partial`/`Missing` rows: yes.
- Existing excluded rows have rationale: yes.
- Every Phase17 ledger row has a final state: yes.
- `P17-L001` is `blocked_external`, so pure `migration_complete` is not used.
- Final broad sign-off exits zero: yes.
- Final report states exactly one decision: yes, `migration_not_complete`.
- Remaining blockers are grouped and owned in `continuation_ledger.md`.
