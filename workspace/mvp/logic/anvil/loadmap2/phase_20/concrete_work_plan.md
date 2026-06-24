# Phase 20 Concrete Work Plan

## Step 0: Establish Phase20 Baseline

Run:

```bash
git status --short
git rev-parse --short HEAD
```

Record:

- commit hash;
- dirty flag;
- branch;
- selected proof roots.

Read:

```text
workspace/mvp/logic/anvil/loadmap2/recovery_plan.md
workspace/mvp/logic/anvil/loadmap2/README.md
workspace/mvp/logic/anvil/loadmap2/phase_17/blocking_ledger.md
workspace/mvp/logic/anvil/loadmap2/phase_17/signoff_reconciliation.md
docs/eval/legacy-control-stack-coverage-20260621.md
docs/eval/loadmap2-phase18-focused-recovery-20260623.md
docs/eval/loadmap2-phase19-large-recovery-20260623.md
```

Expected baseline:

- Phase18 focused rows are `closed_proven`.
- Phase19 large rows are `closed_proven` or valid `blocked_external`.
- Any `blocked_external` row, including `P17-L001`, prevents pure
  `migration_complete` unless it is re-proven as `closed_proven`; otherwise it
  must be handled as an explicit exclusion/accepted limitation or a remaining
  blocker.
- Coverage table may still contain stale or real `Partial` / `Missing` rows.

## Step 1: Build Coverage Closure Inventory

Create:

```text
workspace/mvp/logic/anvil/loadmap2/phase_20/coverage_closure.md
```

For every row in
`docs/eval/legacy-control-stack-coverage-20260621.md`, record:

| Field | Meaning |
| --- | --- |
| source mechanism | exact coverage table row name |
| prior status | current table status before Phase20 |
| adoption decision | current table adoption decision |
| final status candidate | `Implemented`, `Excluded`, or `Unresolved` |
| proof source | phase report, test, eval root, code/doc reference |
| owner layer | responsible CommandAgent layer |
| reason | why the final status is justified |
| final decision impact | complete / excluded / not complete |

Rules:

- `Implemented` requires named implementation/eval evidence.
- `Excluded` requires architecture/design rationale.
- `Unresolved` means Phase20 cannot declare full migration complete.
- Do not collapse multiple mechanisms into one row unless the original
  coverage table already groups them.

## Step 2: Decide Whether The Coverage Table Is Stale Or Truly Open

For each `Partial` / `Missing` row:

1. Search phase reports under `docs/eval/loadmap2-phase*.md`.
2. Search code/tests for the owner module named in the row.
3. Check whether the behavior is eval-proven or only structurally present.
4. Assign one of:
   - `implemented_with_proof`;
   - `excluded_by_design`;
   - `real_gap`.

Recommended command patterns:

```bash
rg -n "<source mechanism keywords>" docs src tests scripts eval
rg -n "Implemented|Partial|Missing|Excluded" docs/eval/legacy-control-stack-coverage-20260621.md
```

Do not update the main coverage table until every row has a proposed
Phase20 disposition in `coverage_closure.md`.

## Step 3: Reconcile Phase17 Ledger

Create:

```text
workspace/mvp/logic/anvil/loadmap2/phase_20/ledger_reconciliation.md
```

Include:

- every P17 row;
- final status;
- proof report;
- proof command;
- whether `blocked_external` is accepted;
- whether any `blocked_external` status changes the final decision from pure
  `migration_complete` to `migration_complete_with_explicit_exclusions` or
  `migration_not_complete`;
- any remaining risk.

Required checks:

- no `open` rows;
- no CI-only closure;
- no `blocked_external` row missing owner/action/evidence;
- every row maps to Phase18 or Phase19 proof.

## Step 4: Regenerate Recheck Reports If Needed

Before final sign-off, verify these files exist:

```text
eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759/recheck_summary.tsv
eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638/recheck_summary.tsv
eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659/recheck_summary.tsv
eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149/recheck_summary.tsv
```

If a file is missing, regenerate with the matching cases directory:

```bash
python3 scripts/eval_report.py <root> --cases-dir eval/cases/<family> --recheck
```

Do not overwrite or reinterpret the original `summary.tsv`.

## Step 5: Run Final Broad Sign-off

Run:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Record:

- exit code;
- stdout;
- root paths;
- whether any finding appears.

If the sign-off fails:

1. Do not declare migration complete.
2. Add a row to a continuation ledger under Phase20.
3. Map each finding to owner layer and proof command.
4. Decide whether it is a current-phase documentation/audit gap or a new
   implementation phase.

## Step 6: Decide Whether A Read-only Closure Checker Is Needed

Add `scripts/eval_migration_closure.py` only if manual reconciliation leaves a
real risk of missing adopted `Partial` / `Missing` rows.

If added, the checker should:

- parse the Markdown coverage table;
- parse the Phase17 ledger;
- report unresolved adopted rows and open ledger rows;
- exit non-zero on unresolved complete-decision blockers;
- never mutate files;
- never run eval or runtime.

Add tests under `tests/` if the helper is implemented.

If not added, document why manual table reconciliation is sufficient for this
phase.

## Step 7: Update Coverage Table

Update:

```text
docs/eval/legacy-control-stack-coverage-20260621.md
```

Allowed updates:

- mark stale `Partial` rows as `Implemented` only with proof source;
- mark intentional non-migration as `Excluded` only with rationale;
- leave real gaps visible as `Partial` or `Missing` if final decision is
  `migration_not_complete`;
- update summary counts.

Do not change `Excluded` rows unless design rationale changed.

## Step 8: Write Final Decision Report

Create:

```text
docs/eval/loadmap2-phase20-final-migration-decision-20260623.md
```

Required sections:

1. Scope and inputs.
2. Commit hash and dirty flag.
3. Coverage final counts.
4. Ledger reconciliation summary.
5. Final broad sign-off command and result.
6. Final decision.
7. `blocked_external` treatment and whether it prevents pure completion.
8. Explicit exclusions, if any.
9. Remaining blockers, if decision is not complete.
10. Next action.

The final decision must be exactly one of:

- `migration_complete`;
- `migration_complete_with_explicit_exclusions`;
- `migration_not_complete`.

## Step 9: Update Supporting Docs

Update only as needed:

- `docs/known-limitations.md`
  - when final accepted limitations or explicit exclusions change.
- `docs/architecture.md`
  - when a new closure checker becomes part of architecture/eval workflow.
- `docs/evaluation.md`
  - when the final closure workflow becomes a reusable eval command.
- `workspace/mvp/logic/anvil/loadmap2/phase_20/README.md`
  - completion result and final decision.
- `workspace/mvp/logic/anvil/loadmap2/phase_20/implementation_tasks.md`
  - completed checkboxes.

## Step 10: Run Verification

If only docs and manual reconciliation changed:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
git diff --check
```

If eval helper scripts changed:

```bash
python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
python3 -m unittest <new checker tests>
```

If Rust/runtime code changed:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
bash scripts/eval_smoke.sh
```

## Step 11: Phase20 Exit Review

Fill this table before closing Phase20:

| Question | Required answer for completion |
| --- | --- |
| Are all adopted coverage rows implemented or explicitly excluded? | yes |
| Are all exclusions documented with rationale? | yes |
| Are all Phase17 ledger rows `closed_proven` for pure completion, or explicitly accepted/excluded for complete-with-exclusions? | yes |
| Does final broad sign-off exit zero? | yes |
| Does the final report state exactly one decision? | yes |
| If not complete, are all blockers owned and proof commands named? | yes |

If any answer is not yes, Phase20 can still finish only as
`migration_not_complete` with a continuation ledger.

## Step 12: Commit Readiness

Before commit:

```bash
git status --short
git diff --check
```

Expected tracked/forced additions:

- Phase20 plan files under `workspace/mvp/logic/anvil/loadmap2/phase_20/`
- `coverage_closure.md`
- `ledger_reconciliation.md`
- final decision report under `docs/eval/`
- updated coverage table and supporting docs
- optional checker/tests if implemented

## Review Result Reflected

The concrete plan was reviewed for the main Phase20 failure modes:

- It separates coverage closure from sign-off proof.
- It forbids marking rows `Implemented` without named evidence.
- It keeps `migration_not_complete` as an honest valid outcome.
- It makes a new checker optional and read-only to avoid adding orchestration
  complexity.
- It treats `blocked_external` as incompatible with pure
  `migration_complete` unless the row is re-proven as `closed_proven`.
- It requires a row-level continuation ledger instead of vague follow-up work
  when completion gates are not met.

## Execution Result

Phase20 completed as a reconciliation and decision phase.

Created:

- `coverage_closure.md`
- `ledger_reconciliation.md`
- `continuation_ledger.md`
- `docs/eval/loadmap2-phase20-final-migration-decision-20260623.md`

Updated:

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- Phase20 plan files with execution status.

Verification:

- final broad sign-off: pass
- final migration decision: `migration_not_complete`

The result is intentional: the sign-off proof is green, but coverage parity is
not yet complete.
