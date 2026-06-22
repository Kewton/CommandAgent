# Phase 21 Concrete Work Plan

## Step 0: Establish Baseline

Run:

```bash
git status --short
git rev-parse --short HEAD
git branch --show-current
```

Read:

```text
workspace/mvp/logic/anvil/loadmap2/recovery_plan.md
workspace/mvp/logic/anvil/loadmap2/phase_20/coverage_closure.md
workspace/mvp/logic/anvil/loadmap2/phase_20/continuation_ledger.md
docs/eval/loadmap2-phase20-final-migration-decision-20260623.md
docs/eval/legacy-control-stack-coverage-20260621.md
```

Expected baseline:

- Phase20 final decision is `migration_not_complete`.
- Phase20 broad sign-off is pass.
- P20-COV-001 is open and maps to C01-C12.
- The working tree should be clean before implementation starts.

## Step 1: Create Row Closure Matrix

Create:

```text
workspace/mvp/logic/anvil/loadmap2/phase_21/row_closure_matrix.md
workspace/mvp/logic/anvil/loadmap2/phase_21/blocking_ledger.md
workspace/mvp/logic/anvil/loadmap2/phase_21/reconciliation.md
```

Use this table shape:

| field | required content |
| --- | --- |
| id | C01-C12 |
| source mechanism | exact Phase20 coverage row name |
| owner | one layer/module family |
| parity target | concrete MVP behavior needed for closure |
| implementation target | files/modules likely to change |
| proof | unit test, focused eval, broad sign-off |
| status | `open`, `closed_proven`, `excluded_with_rationale`, `split_forward` |
| notes | reason and remaining risk |

Initial expected rows:

- C01-C03: task contract and behavior obligation projection.
- C04-C06: artifact role, workspace scope, artifact ownership.
- C07-C10: ledger, completion evidence, evidence binding, deliverable audit.
- C11-C12: active job and dispatch gate.

Do not edit runtime code until this matrix exists.

`blocking_ledger.md` must split P20-COV-001 into row-level blockers. Each row
needs owner, failed/incomplete contract, suspected module, proof command,
closure condition, and status.

`reconciliation.md` must map:

```text
P20-COV-001
  -> Phase21 blocker
  -> coverage row C01-C12
  -> implementation task
  -> proof command
  -> final sign-off rerun
```

If a row cannot be mapped through this chain, keep it out of implementation
until the mapping is corrected.

## Step 2: Inspect Existing Implementation Boundaries

Use `rg` to locate current owners:

```bash
rg -n "TaskContract|BehaviorObligation|ArtifactRole|WorkspaceScope|ArtifactOwnership|ArtifactLedger|CompletionEvidence|EvidenceBinding|DeliverableObligation|ActiveJob|Dispatch" src tests scripts docs
```

For each row, decide:

1. Is the implementation already present but unproven?
2. Is a small shared contract field missing?
3. Is model-facing behavior missing and therefore needs focused eval?
4. Is the row too broad and should be split forward?

Record the answer in `row_closure_matrix.md`.

## Step 3: Close C01-C03

Target:

```text
C01 Task contract core
C02 Task contract inference and admission
C03 Objective and behavior contract projection
```

Work pattern:

1. Add/tighten typed task contract fields only when missing.
2. Ensure the fields render into plan prompts, correction evidence, or eval
   reports as the closure target requires.
3. Add unit tests for deterministic projection.
4. Add focused eval only if generated plans or repairs must change behavior.

Example proof:

```bash
cargo test task_contract
cargo test plan_lint
python3 tests/test_eval_report.py
```

Update the row matrix after each row.

## Step 4: Close C04-C06

Target:

```text
C04 Artifact role taxonomy
C05 Task workspace scope
C06 Artifact ownership
```

Work pattern:

1. Use a common role/scope/ownership SSOT before profile-specific exceptions.
2. Keep generated, cache, dependency, and build output paths rejected.
3. Add representative tests for Next.js, Rust, Python, docs, and data where a
   common classifier is claimed.
4. Ensure target admission consumes the same ownership facts.

Example proof:

```bash
cargo test profile_artifact
cargo test workspace_scope
cargo test artifact_ownership
```

## Step 5: Close C07-C10

Target:

```text
C07 Artifact ledger
C08 Completion evidence
C09 Evidence binding
C10 Deliverable obligation audit
```

Work pattern:

1. Verify tool records and verifier observations reconcile into bounded
   artifact/evidence data.
2. Add only deterministic shared producers needed for proof.
3. Project proof-relevant fields into eval reports.
4. Keep freshness and binding checks observable, not hidden repair triggers.

Example proof:

```bash
cargo test artifact_ledger
cargo test completion_evidence
cargo test evidence_binding
cargo test deliverable_obligation
python3 tests/test_eval_report.py
```

## Step 6: Close C11-C12

Target:

```text
C11 Active job arbiter
C12 Recovery owner / dispatch gate
```

Work pattern:

1. Verify candidate generation has owner/job/action/target/tool policy facts.
2. Verify dispatch selects one candidate or emits explicit conflict.
3. Add deterministic tie-break and conflict tests.
4. Avoid adding a hidden scheduler or multi-engine control loop.

Example proof:

```bash
cargo test active_job
cargo test dispatch
cargo test recovery
```

## Step 7: Add Focused Eval Only For Behavior Changes

Add or update focused fixtures only when unit tests cannot prove model-facing
behavior.

Allowed fixture themes:

- task contract projection affects generated plan content;
- role/scope/ownership changes recovery target choice;
- active job dispatch changes repair owner/action;
- evidence binding/completion authority changes explicit stop vs repair.

Do not add broad live eval just to hunt for a green result.

## Step 8: Update Documentation

Create:

```text
docs/eval/loadmap2-phase21-core-contract-ownership-20260623.md
workspace/mvp/logic/anvil/loadmap2/phase_21/implementation_report.md
```

Update as needed:

```text
docs/eval/legacy-control-stack-coverage-20260621.md
docs/architecture.md
docs/evaluation.md
docs/profiles.md
```

Only update the coverage table when a row has proof or a controlled
`split_forward` outcome.

## Step 9: Run Verification

Minimum:

```bash
cargo fmt --check
cargo test
python3 tests/test_eval_report.py
python3 tests/test_eval_signoff.py
bash scripts/eval_smoke.sh
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
scripts/check_branding.sh
git diff --check
```

If focused fixtures are added, also run the targeted fixture and report/recheck
commands recorded in `implementation_report.md`.

## Step 10: Exit Review

Fill the exit review before closing:

| question | expected answer |
| --- | --- |
| Is every C01-C12 row accounted for? | yes |
| Are implemented rows proven by tests/eval? | yes |
| Are split-forward rows row-level and owned? | yes |
| Are Phase21 ledger and reconciliation entries complete? | yes |
| Does final broad sign-off pass? | yes |
| Does branding check pass after tracked docs updates? | yes |
| Are docs and coverage updated only with proof? | yes |
| Did the phase avoid hidden retry/provider-specific policy/profile workflows? | yes |

If any answer is no, Phase21 remains incomplete or closes only with explicit
split-forward blockers.

## Step 11: Commit Readiness

Phase21 execution status:

| step | status |
| --- | --- |
| Step 0 baseline | done |
| Step 1 row closure matrix / ledger / reconciliation | done |
| Step 2 implementation boundary inspection | done |
| Step 3 C01-C03 closure planning | split forward to Phase22 |
| Step 4 C04-C06 closure planning | split forward to Phase23 |
| Step 5 C07-C10 closure planning | split forward to Phase24 |
| Step 6 C11-C12 closure planning | split forward to Phase25 |
| Step 7 focused eval additions | not needed; no model-facing behavior changed |
| Step 8 docs update | done |
| Step 9 verification | done |
| Step 10 exit review | done |

Commit readiness requires staging the ignored workspace outputs with
`git add -f`.

Before commit:

```bash
git status --short
git diff --check
```

Expected files:

- code/tests only for C01-C12 closure;
- `row_closure_matrix.md`;
- `blocking_ledger.md`;
- `reconciliation.md`;
- `implementation_report.md`;
- Phase21 eval report;
- targeted docs/coverage updates.

## Review Result Reflected

The concrete plan was reviewed for the Phase20 failure mode: a phase can look
successful while coverage remains unresolved. Phase21 therefore starts with a
row closure matrix, blocking ledger, and reconciliation map, not code. It also
makes `split_forward` explicit and bounded, so broad rows cannot be silently
counted as complete.
