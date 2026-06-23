# Phase32 Concrete Work Plan

Date: 2026-06-23 JST

Status: completed / reviewed

## Ordered Steps

### Step 1. Freeze Inputs

Read and record the exact state of:

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- Phase22-Phase31 implementation reports

Completed output:

- coverage counts;
- KI status counts;
- phase-local disposition counts;
- proof roots to reuse in final sign-off.

### Step 2. Reconcile Coverage Rows

Build a closure table for:

- C01-C45 as adopted and implemented;
- C46-C54 as excluded with rationale;
- P17-L001 as closed proof blocker;
- KI-011 as final closure blocker.

If any row is stale or contradictory, stop and add a blocker before editing the
final report.

### Step 3. Run Final Broad Sign-off

Run:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --root large=eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624
```

It passed. If it had failed:

- capture the exact finding;
- add a row to `blocking_ledger.md`;
- map the finding to coverage row, owner layer, target document/module, and
  proof command;
- do not declare migration complete.

### Step 4. Write Final Report

Created:

```text
docs/eval/anvil-migration-complete.md
```

The report must include:

- final decision;
- source baseline;
- coverage row counts;
- implemented row ranges;
- excluded row ranges and rationale;
- Phase22-Phase31 proof summary;
- final broad sign-off command and result;
- known limitations, if any;
- statement that no hidden retry/provider policy/legacy engine was introduced.

### Step 5. Update Roadmap Documents

Updated:

- loadmap2 `README.md`;
- loadmap2 `recovery_plan.md`;
- loadmap2 `current_issue_phase_map.md`;
- coverage table only if stale text/counts remain.

The updates must mark Phase32 as closed only if the final report and sign-off
support it.

### Step 6. Verify Documentation And Reports

Ran:

```bash
python3 tests/test_eval_signoff.py
python3 tests/test_eval_report.py
python3 -m py_compile scripts/eval_report.py scripts/eval_signoff.py
git diff --check
```

No runtime code changes were made. If runtime code changes had been made, the
additional required commands would have been:

```bash
cargo fmt --check
cargo test
cargo build --release
```

### Step 7. Close Phase32

Created `implementation_report.md` with:

- row disposition counts;
- final decision;
- proof commands and results;
- final sign-off result;
- unresolved blockers, if any;
- review result.

## Target Files

Primary targets:

- `docs/eval/anvil-migration-complete.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/phase_32/implementation_report.md`

Conditional targets:

- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `docs/philosophy.md`
- `docs/architecture.md`
- `docs/evaluation.md`
- `docs/adr/0002-contract-recovery.md`

## Rollback And Split Rules

- If final sign-off fails with a mapped implementation gap, Phase32 remains
  open and the blocker is split into the smallest same-surface phase.
- If final sign-off fails due to a new distinct responsibility class, extend
  the roadmap after Phase32 and declare `migration_not_complete`.
- If coverage contains any adopted `Partial` or `Missing`, do not update KI-011
  as closed.
- If only documentation wording is stale, fix the wording inside Phase32
  without creating a new runtime phase.

## Review Result

The concrete plan was reviewed against the recurring failure pattern from
Phase1-Phase21: moving forward without proof.

Review changes applied:

- Step 2 requires contradiction detection before final report writing;
- Step 3 records exact failure-to-ledger mapping before any split;
- Step 6 separates docs/report checks from runtime checks;
- Step 7 requires an implementation report rather than relying on the final
  assistant response.
