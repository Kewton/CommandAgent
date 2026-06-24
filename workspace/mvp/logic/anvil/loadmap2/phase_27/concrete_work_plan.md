# Phase27 Concrete Work Plan

Date: 2026-06-23 JST

## Step 0: Baseline And Scope Confirmation

Inputs:

- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- Phase22 through Phase26 packages

Actions:

1. Confirm KI-006 still owns C21-C32.
2. Confirm Phase22-C01 through Phase26-C20 are not reopened.
3. Capture current commit and dirty state.
4. Record any unrelated dirty files and keep them out of Phase27 commits.

Exit condition:

- Phase27 work is scoped to C21-C32 only.

## Step 1: Source Alignment And Row Ledger

Actions:

1. Reconcile C21-C32 against Anvil source files in
   `source_alignment_matrix.md`.
2. For each row, define adopted behavior, intentionally omitted behavior,
   CommandAgent target modules, and proof method.
3. Fill `row_closure_matrix.md` with current status `Partial`, adoption
   decision, owner layer, missing contract, target modules, required proof,
   closure condition, and initial disposition.
4. Fill `blocking_ledger.md` with one or more blockers per row.
5. Keep all blockers same-surface. If a blocker belongs to Phase28 or Phase29,
   record it as a boundary, not as Phase27 work.

Exit condition:

- Every C21-C32 row has at least one blocker and one proof command.

## Step 2: C21 Target Admission

Actions:

1. Inspect `target_admission.rs`, `artifact_graph.rs`,
   `artifact_ownership.rs`, `artifact_ledger.rs`, and
   `recovery_orchestration.rs`.
2. Define a target admission input object that consumes active job, action
   envelope, artifact role, ownership, scope, source of truth, freshness,
   current excerpt, and exhaustion state.
3. Add admission/rejection paths for route, source, test, docs, setup, and
   evidence-binding targets.
4. Add focused target matrix cases.

Do not:

- allow profile-owned target selection;
- select a target by path order when authority is ambiguous.

## Step 3: C22 Target Priority

Actions:

1. Define target priority components as data, not as prompt prose.
2. Rank admitted targets by failure kind, source of truth, role, focused-edit
   signal, evidence freshness, and progress history.
3. Stop with structured evidence on ambiguous same-priority admitted targets.
4. Add target-priority tests and focused prioritization fixture.

Do not:

- use model confidence, filename order, or hidden memory to break ties.

## Step 4: C23 Repair Job Lifecycle

Actions:

1. Inspect `repair_job.rs`, `runtime/repair_loop.rs`, and recovery task
   rendering.
2. Define lifecycle transitions for selected, running, verifier_rerun,
   completed, failed, no_progress, and explicit_stop.
3. Record verifier rerun result and safe-stop report without changing the
   verifier command.
4. Add repair-job tests and focused lifecycle fixture.

Do not:

- continue hidden future phases after a failed lifecycle transition.

## Step 5: C24 Attempt Ledger

Actions:

1. Define attempt outcome record fields.
2. Record target, role, cluster, before/after signatures, changed files,
   verifier outcome, profile family, and attempt outcome.
3. Add outcome tests for passed, noop, malformed, unsafe, duplicate,
   no-progress, improved-still-failing, worsened, and explicit stop.
4. Add eval report fields and focused attempt matrix.

Do not:

- treat a changed file as progress unless verifier or patch validation proves
  it is acceptable.

## Step 6: C25 No-progress Strategy

Actions:

1. Consume attempt ledger exhaustion facts from C24.
2. Select bounded strategy branches for switch target, switch role, evidence
   binding, scaffold/materialization, contract-conflict deferral, or explicit
   stop.
3. Add a contract-conflict branch that points to Phase28/C33 and does not
   perform source repair fallback.
4. Add no-progress tests and focused no-progress matrix.

Do not:

- increase retry budgets;
- resolve C33 source-of-truth conflicts in Phase27.

## Step 7: C26 Verifier Diagnostic Assessment

Actions:

1. Inspect `verifier_diagnostic.rs`, `semantic_failure.rs`, and
   `recovery_contract.rs`.
2. Add language/common diagnostic assessment fields for Rust, Python, Next.js,
   command-not-found, missing dependency, port-in-use, weak source grep, and
   generated/self-referential tests.
3. Preserve unknown diagnostics as observable counts.
4. Add verifier-diagnostic tests and focused verifier fixture.

Do not:

- ask a model to classify diagnostics when deterministic evidence is missing.

## Step 8: C27 Verifier Orchestration

Actions:

1. Inspect `verify.rs`, repair loop, and evidence binding flow.
2. Add verifier rerun event fields and attempt limit reporting.
3. Bind rerun authority to the original verifier and evidence scope.
4. Add safe-stop report when verifier rerun cannot proceed or repeats the same
   failure class.
5. Add focused verifier-rerun fixture.

Do not:

- replace the original verifier with a weaker command.

## Step 9: C28 Verifier Command Policy

Actions:

1. Inspect `verifier_selection.rs`, `integrity_guard.rs`, and
   `plan_lint/verifiers.rs`.
2. Add generated-test preflight and expectation-audit checks.
3. Reject self-referential verifier commands, unsupported contract assertions,
   and test weakening before completion evidence is claimed.
4. Add verifier-selection/integrity tests and focused verifier-policy fixture.

Do not:

- make generated tests authoritative without source-of-truth evidence.

## Step 10: C29 Artifact Completion Job

Actions:

1. Inspect `artifact_completion.rs`, `evidence_authority.rs`, and
   `deliverable_obligation.rs`.
2. Bind completion jobs to owned in-scope ledger entries and freshness rules.
3. Keep missing deliverable, missing evidence, failed evidence, and stale
   evidence distinct in runtime/eval fields.
4. Add focused artifact-completion fixture.

Do not:

- claim completion from candidate-only reads or stale evidence.

## Step 11: C30 Focused Edit Recovery

Actions:

1. Inspect read/edit/write ledger signals and current excerpt handling.
2. Require current excerpt availability for focused edit admission.
3. Reject stale, changed-only, out-of-scope, and exhausted targets before
   prompt rendering.
4. Add focused edit fixture.

Do not:

- run hidden file reads to rescue focused edit after admission.

## Step 12: C31 Mechanical Fallback Admission

Actions:

1. Inspect `mechanical_repair.rs` and repair action plan fields.
2. Require owner/action/target/verifier authority before a mechanical fallback
   can be rendered.
3. Ensure fallback output is a bounded patch proposal or instruction that must
   pass C32 validation.
4. Add mechanical-repair tests and focused fallback fixture.

Do not:

- directly mutate files from mechanical fallback without patch validation.

## Step 13: C32 Patch Validation And Rollback Proof

Actions:

1. Inspect `integrity_guard.rs`, `mechanical_repair.rs`, repair loop, and
   rollback admission fields.
2. Add patch validation outcome matrix for accepted, unsafe, noop, duplicate,
   test weakening, protected/generated/cache/raw, and rollback admission.
3. Ensure progress is claimed only after deterministic patch validation and
   original verifier rerun.
4. Add focused patch validation matrix.

Do not:

- execute hidden rollback or call a worsened patch successful.

## Step 14: Focused Eval Package

Suggested focused fixture directory:

```text
eval/cases/focused/control-recovery/target-verifier-patch/
```

Minimum fixture families:

- target admission and rejection matrix;
- target priority and ambiguous tie stop;
- repair lifecycle and verifier rerun;
- attempt ledger outcomes;
- no-progress strategy and contract-conflict deferral;
- verifier diagnostics and weak verifier policy;
- artifact completion job;
- focused edit current excerpt and stale target;
- mechanical fallback admission;
- patch validation and rollback proof.

Suggested execution:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/target-verifier-patch \
  --out eval/runs/loadmap2-phase27-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase27-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/target-verifier-patch \
  --recheck
```

## Step 15: Documentation And Coverage

Update only after row proof:

- `docs/architecture.md`
- `docs/adr/0002-contract-recovery.md`
- `docs/evaluation.md`
- `docs/profiles.md`, if profile inputs change
- `docs/known-limitations.md`, if needed
- `docs/eval/loadmap2-phase27-target-verifier-patch-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`

Coverage status rule:

- C21-C32 can become `Implemented` only after unit, focused, report, and
  sign-off proof.
- Any row below proof remains `Partial` and must be split with owner,
  downstream phase, failed proof, and closure condition.

## Step 16: Verification

Run:

```bash
cargo fmt --check
cargo test target_admission
cargo test repair_job
cargo test verifier_diagnostic
cargo test verifier_selection
cargo test integrity_guard
cargo test artifact_completion
cargo test evidence_authority
cargo test mechanical_repair
cargo test repair_action_plan
cargo test recovery_orchestration
cargo test repair_loop
python3 tests/test_eval_report.py
cargo test
cargo build --release
```

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=<phase27-focused-root> \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

If sign-off fails:

- map each finding to C21-C32 or a later same-surface phase;
- keep Phase27 open unless the finding is split with owner, downstream phase,
  proof command, and failed proof evidence.

## Review Result

Review findings applied:

- Split implementation into row-owned steps before any runtime change.
- Added target/verifier/lifecycle/completion/focused-edit/mechanical/patch
  focused fixture requirements instead of relying on broad eval.
- Kept Phase28 conflict resolution and Phase29 runtime-support expansion out
  of scope.
- Required common contracts and eval fields before profile-specific expansion.
- Added no-go rules for hidden retries, verifier weakening, implicit setup,
  hidden patch execution, and workflow engines.
