# Phase25 Concrete Work Plan

Date: 2026-06-23 JST

## Execution Order

### Step 1: Baseline And Guardrails

Read and record:

- current CommandAgent commit and dirty state;
- C11-C12 rows in `docs/eval/legacy-control-stack-coverage-20260621.md`;
- current active-job/dispatch modules:
  - `active_job.rs`
  - `recovery_orchestration.rs`
  - `recovery_policy.rs`
  - `recovery_task.rs`
  - `recovery_contract.rs`
  - `repair_brief.rs`
  - `repair_action_plan.rs`
  - `target_admission.rs`
  - `evidence.rs`
  - `runtime/repair_loop.rs`
- current eval/report fields for owner/action/dispatch/tie-break/candidate
  evidence.

Output:

- update `blocking_ledger.md` actual baseline fields if they changed;
- no runtime code changes yet.

### Step 2: C11 Candidate And Lifecycle Model

Target modules:

- `src/agent/step_runner/active_job.rs`
- `src/agent/step_runner/recovery_orchestration.rs`
- `src/agent/step_runner/recovery_contract.rs`
- `src/agent/step_runner/evidence.rs`
- `scripts/eval_report.py`
- `scripts/eval_agent_slice.sh`
- `tests/test_eval_report.py`

Implementation direction:

1. Treat active-job facts as a deterministic contract decision, not a label.
2. Ensure candidate records carry owner, job, action, source layer,
   source-of-truth, target hint, artifact role, priority, tool policy, loop
   control action, rerun authority, and reason.
3. Add or prove lifecycle states: candidate, selected, not applicable,
   no-owner, ambiguous tie, explicit stop, and conflict stop.
4. Expose lifecycle and dispatch state in evidence/eval without deriving the
   final owner from prose reason text.
5. Keep C33 conflict resolution out of scope; Phase25 may only produce a
   structured conflict-stop handoff.

Proof:

```bash
cargo test active_job
cargo test recovery_orchestration
python3 tests/test_eval_report.py
```

### Step 3: C11 Arbitration Rules

Target modules:

- `src/agent/step_runner/recovery_orchestration.rs`
- `src/agent/step_runner/active_job.rs`
- `src/agent/step_runner/recovery_contract.rs`

Implementation direction:

1. Define stable priority ordering from shared contract data.
2. Select the highest-priority candidate when it is unique.
3. Merge deterministic metadata only for compatible same-owner candidates.
4. Stop with `ambiguous_tie` when competing owners have equal authority and no
   deterministic tie-break is allowed.
5. Stop with `no_owner` when no eligible candidate exists.
6. Stop with `contract_conflict` only as a Phase28 handoff, not as final
   conflict resolution.
7. Record candidate list and tie-break reason for every non-trivial decision.

Proof:

```bash
cargo test recovery_orchestration
cargo test active_job
```

### Step 4: C12 Candidate Producers To Dispatch Gate

Target modules:

- `src/agent/step_runner/recovery_policy.rs`
- `src/agent/step_runner/recovery_orchestration.rs`
- `src/agent/step_runner/profiles.rs`
- `src/agent/step_runner/profile_artifact.rs`
- `src/agent/step_runner/evidence_binding.rs`
- `src/agent/step_runner/recovery_contract.rs`
- `src/agent/step_runner/evidence.rs`

Implementation direction:

1. Inventory current candidate-producing branches for profile failures,
   setup/dependency, manifest/config, route/import, source diagnostics, docs,
   evidence binding, verifier contract, and tool protocol failures.
2. Convert every branch that currently sets only prose or active-job labels
   into a typed `ActiveJobCandidate` path.
3. Preserve profile boundaries: profiles may emit candidate hints but must not
   choose the final dispatch.
4. Ensure setup candidates remain policy-visible and verifier-owned; do not
   make normal repair run dependency setup implicitly.
5. Ensure every candidate carries a source layer and source-of-truth so eval
   reports can distinguish profile, verifier, setup, tool, and evidence
   origins.

Proof:

```bash
cargo test recovery_policy
cargo test recovery_orchestration
cargo test recovery_contract
```

### Step 5: C12 Dispatch Consumption Before Prompt Rendering

Target modules:

- `src/agent/step_runner/recovery_task.rs`
- `src/agent/step_runner/repair_brief.rs`
- `src/agent/step_runner/repair_action_plan.rs`
- `src/agent/step_runner/runtime/repair_loop.rs`
- `src/agent/step_runner/recovery_orchestration.rs`
- `src/agent/step_runner/target_admission.rs`

Implementation direction:

1. Ensure repair prompt rendering receives the selected dispatch decision as
   input.
2. Render selected owner, active job, action, target hint, artifact role,
   required action, disallowed actions, allowed tool policy, loop control
   action, and rerun authority.
3. Ensure prompt rendering does not recompute owner/action from reason text.
4. Ensure runtime consumes exactly one selected action or explicit stop.
5. Preserve the minimal loop boundary: the runtime executes the selected
   bounded task; it does not become an arbiter.

Proof:

```bash
cargo test recovery_task
cargo test repair_brief
cargo test target_admission
cargo test recovery_orchestration
```

### Step 6: Focused Fixtures And Report Proof

Target files:

- `eval/cases/focused/control-recovery/nextjs/dependency-setup.yaml`
- `eval/cases/focused/control-recovery/nextjs/manifest-repair.yaml`
- `eval/cases/focused/control-recovery/nextjs/route-integration-repair.yaml`
- `eval/cases/focused/control-recovery/docs/docs-literal-mismatch.yaml`
- `eval/cases/focused/control-recovery/completion/evidence-binding-failure.yaml`
- `eval/cases/focused/control-recovery/recovery-policy/contract-conflict-explicit-stop.yaml`
- `eval/cases/focused/control-recovery/tool-protocol/missing-write-path.yaml`
- new Phase25 dispatch fixtures if existing cases do not prove all required
  fields
- `scripts/eval_agent_slice.sh`
- `scripts/eval_report.py`
- `scripts/eval_runtime_job_report.py`
- `tests/test_eval_report.py`

Implementation direction:

1. Reuse existing focused cases only where they assert C11/C12 fields.
2. Add focused fixtures for missing dispatch fields:
   - setup dispatch;
   - manifest dispatch;
   - route integration dispatch;
   - source diagnostic dispatch;
   - docs dispatch;
   - evidence binding dispatch;
   - verifier contract dispatch;
   - tool protocol dispatch;
   - no-owner explicit stop;
   - ambiguous tie explicit stop.
3. Keep focused assertions narrow and deterministic.
4. Record focused proof roots and row-to-root mapping in `reconciliation.md`.

Suggested focused command:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase25-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
```

Then recheck:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase25-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/dispatch \
  --recheck
```

### Step 7: Documentation And Coverage

Update only after tests prove behavior:

- `docs/architecture.md`
- `docs/adr/0002-contract-recovery.md`
- `docs/evaluation.md`
- `docs/profiles.md`
- `docs/ultra-plan-run.md`, only if planner-facing fields change
- `docs/eval/loadmap2-phase25-active-job-dispatch-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`

Coverage status rule:

- C11-C12 can become `Implemented` only if unit, focused, and report proof
  pass.
- If a row remains below proof, leave coverage status as `Partial` and record
  the narrower blocker in `blocking_ledger.md`.

### Step 8: Verification

Run:

```bash
cargo fmt --check
cargo test active_job
cargo test recovery_orchestration
cargo test recovery_policy
cargo test recovery_task
cargo test recovery_contract
cargo test repair_brief
cargo test target_admission
python3 tests/test_eval_report.py
cargo test
cargo build --release
```

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

If any sign-off finding remains:

- map it to C11, C12, or a later same-surface phase;
- keep Phase25 open unless the finding is split with owner, proof command,
  downstream phase, and failed proof evidence.

## Review Result

Review findings applied:

- Added a separate arbitration step before dispatch consumption so C11 does
  not disappear into C12 prompt rendering.
- Required candidate producers to carry source layer/source-of-truth for
  cross-profile rollout and later debugging.
- Required dispatch proof before prompt rendering to address the recurring
  failure where the model receives evidence but chooses the wrong repair task.
- Added explicit commands and focused fixture families for both selected and
  stop states.
- Kept Phase25 from absorbing Phase26 recovery-task semantics, Phase27 target
  lifecycle, and Phase28 conflict resolution.

## Implementation Result

Phase25 is closed.

- C11 status: `closed_proven`
- C12 status: `closed_proven`
- Focused root: `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`
- Focused assertions: `passed: 10`
- Recheck assertions: `passed_recheck: 10`
- Broad sign-off: `status: pass`

No C11-C12 finding is carried forward. Later phases still own recovery task
depth, target prioritization, verifier orchestration, patch validation, and
contract-conflict resolution.
