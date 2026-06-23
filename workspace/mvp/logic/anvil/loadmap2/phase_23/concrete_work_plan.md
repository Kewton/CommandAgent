# Phase23 Concrete Work Plan

Date: 2026-06-23 JST

## Execution Order

### Step 1: Baseline And Guardrails

Read and record:

- current CommandAgent commit and dirty state;
- C04-C06 rows in `docs/eval/legacy-control-stack-coverage-20260621.md`;
- current role/scope/ownership modules:
  - `profile_artifact.rs`
  - `artifact_graph.rs`
  - `workspace_scope.rs`
  - `workspace_snapshot.rs`
  - `artifact_ownership.rs`
  - `target_admission.rs`
  - `artifact_completion.rs`
  - `recovery_contract.rs`
  - `runtime/repair_loop.rs`
- current eval/report fields for role/scope/ownership.

Output:

- update `blocking_ledger.md` actual baseline fields if they changed;
- no runtime code changes yet.

### Step 2: C04 Artifact Role SSOT

Target modules:

- `src/agent/step_runner/artifact_graph.rs`
- `src/agent/step_runner/profile_artifact.rs`
- `src/agent/step_runner/plan_lint/mod.rs`
- `src/agent/step_runner/target_admission.rs`
- `src/agent/step_runner/artifact_completion.rs`
- `scripts/eval_report.py`
- `scripts/eval_agent_slice.sh`

Implementation direction:

1. Identify the canonical role source for each profile/path class.
2. Remove or fence off parallel source-like string checks where they can drift
   from `ArtifactRole`.
3. Ensure generated, dependency/cache, and build output roles are visible to
   target and completion consumers.
4. Keep profile-specific facts as classification facts only; profiles must not
   select recovery workflows.

Proof:

```bash
cargo test profile_artifact
cargo test artifact_graph
cargo test target_admission
cargo test artifact_completion
```

### Step 3: C05 Workspace Scope Admission

Target modules:

- `src/agent/step_runner/workspace_snapshot.rs`
- `src/agent/step_runner/workspace_scope.rs`
- `src/agent/step_runner/artifact_graph.rs`
- `src/agent/step_runner/artifact_ownership.rs`
- `src/agent/step_runner/target_admission.rs`
- `src/agent/step_runner/recovery_contract.rs`
- `src/agent/step_runner/runtime/repair_loop.rs`

Implementation direction:

1. Confirm `WorkspaceSnapshot` and `WorkspaceScope` are the single source for
   scope kind and claimable roots.
2. Add or complete deterministic admission rules for:
   - greenfield;
   - single project root;
   - explicit root;
   - ambiguous parent;
   - excluded dependency/cache/build/generated paths.
3. Make target and ownership consumers use scope facts rather than
   reconstructing root decisions.
4. Render/report scope kind and roots where recovery/eval decisions need them.

Proof:

```bash
cargo test workspace_scope
cargo test workspace_snapshot
cargo test artifact_ownership
cargo test target_admission
```

### Step 4: C06 Ownership Consumer Closure

Target modules:

- `src/agent/step_runner/artifact_ownership.rs`
- `src/agent/step_runner/target_admission.rs`
- `src/agent/step_runner/artifact_completion.rs`
- `src/agent/step_runner/evidence_authority.rs`
- `src/agent/step_runner/recovery_orchestration.rs`
- `src/agent/step_runner/runtime/repair_loop.rs`
- `scripts/eval_report.py`

Implementation direction:

1. Make ownership decision fields sufficient for all consumers:
   - ownership status;
   - reason/subreason;
   - source of truth;
   - role;
   - workspace scope summary;
   - candidate origin;
   - repair admissibility.
2. Feed ownership into target admission and completion eligibility.
3. Feed repeated-target exclusion/no-progress evidence only where deterministic
   attempt facts already exist.
4. Add eval/report fields only if needed to prove existing decisions.

Proof:

```bash
cargo test artifact_ownership
cargo test target_admission
cargo test artifact_completion
cargo test evidence_authority
```

### Step 5: Focused Fixture And Report Proof

Target files:

- `eval/cases/focused/control-recovery/planning/artifact-role-scope-ownership.yaml`
  if a new focused fixture is needed
- `scripts/eval_agent_slice.sh`
- `scripts/eval_case_schema.py`
- `scripts/eval_report.py`
- `tests/test_eval_report.py`

Implementation direction:

1. Prefer existing focused fixtures if they already expose role/scope/ownership
   fields.
2. Add one focused fixture if no existing case proves the Phase23 fields.
3. Record a focused proof root in every closure path.
4. Keep focused assertions narrow:
   - role taxonomy visible;
   - workspace scope kind visible;
   - ownership decision/source visible;
   - generated/cache/build outputs not admitted as owned targets.

Proof:

```bash
python3 tests/test_eval_report.py
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/planning --out eval/runs/loadmap2-phase23-focused-fixtures --runs 1 --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase23-focused-fixtures/<root> --recheck
```

### Step 6: Documentation And Coverage

Update only after tests prove behavior:

- `docs/architecture.md`
- `docs/evaluation.md`
- `docs/profiles.md`, if profile classification behavior changes
- `docs/ultra-plan-run.md`, if planner-facing contract guidance changes
- `docs/eval/loadmap2-phase23-artifact-scope-ownership-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`

Coverage status rule:

- C04-C06 can become `Implemented` only if unit, focused, and report proof
  pass.
- If a row remains below proof, leave coverage status as `Partial` and record
  the narrower blocker in `blocking_ledger.md`.

### Step 7: Verification

Run:

```bash
cargo fmt --check
cargo test profile_artifact
cargo test artifact_graph
cargo test workspace_scope
cargo test workspace_snapshot
cargo test artifact_ownership
cargo test target_admission
cargo test artifact_completion
cargo test evidence_authority
python3 tests/test_eval_report.py
cargo test
cargo build --release
```

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=<Phase23 focused proof root> \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

If Phase23 reuses an existing focused fixture instead of adding a new one,
document exactly which assertions prove C04-C06 and use that root for sign-off.

## Rollback / Split Rules

- If a change requires artifact ledger producer closure, split to Phase24.
- If a change requires completion evidence or evidence binding producer
  closure beyond ownership consumption, split to Phase24.
- If a change requires active-job arbitration, split to Phase25.
- If a change requires setup/profile semantic repair, split to Phase26.
- If a change requires target prioritization beyond ownership admission,
  split to Phase27.
- If a row cannot be proven after two targeted attempts, add a design review
  note to `blocking_ledger.md` before continuing.

## Execution Result

Phase23 completed without a split-forward row.

Executed proof:

```text
cargo test profile_artifact: pass
cargo test artifact_graph: pass
cargo test workspace_scope: pass
cargo test workspace_snapshot: pass
cargo test artifact_ownership: pass
cargo test target_admission: pass
cargo test artifact_completion: pass
cargo test evidence_authority: pass
python3 tests/test_eval_report.py: pass
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery/planning --out eval/runs/loadmap2-phase23-focused-fixtures --runs 1 --proof-mode deterministic_fixture: pass
python3 scripts/eval_report.py eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 --cases-dir eval/cases/focused/control-recovery/planning --recheck: focused assertions passed_recheck
cargo fmt --check: pass
cargo test: pass
cargo build --release: pass
python3 scripts/eval_signoff.py ... focused-fixture=eval/runs/loadmap2-phase23-focused-fixtures/20260623T111023 ...: pass
```

Coverage result:

| row | result |
| --- | --- |
| C04 | `Implemented` / `closed_proven` |
| C05 | `Implemented` / `closed_proven` |
| C06 | `Implemented` / `closed_proven` |

## Review Result

Review findings applied:

- Split the concrete plan by C04, C05, and C06 so role, scope, and ownership
  cannot be closed by a single broad proof.
- Added exact target modules and proof commands for each row.
- Made focused proof mandatory for closure while allowing reuse of an existing
  focused case when it explicitly proves C04-C06 fields.
- Added rollback/split rules to prevent Phase23 from absorbing Phase24-27
  responsibilities.
- Kept broad sign-off as regression evidence only, not row-level proof.
