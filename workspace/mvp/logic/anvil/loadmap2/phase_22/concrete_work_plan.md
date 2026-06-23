# Phase22 Concrete Work Plan

Date: 2026-06-23 JST

## Execution Order

### Step 1: Baseline And Guardrails

Read and record:

- current CommandAgent commit and dirty state;
- C01-C03 rows in `docs/eval/legacy-control-stack-coverage-20260621.md`;
- current `TaskContract` behavior in
  `src/agent/step_runner/task_contract.rs`;
- current lint usage in `src/agent/step_runner/plan_lint/mod.rs`;
- current focused cases in `eval/cases/focused/control-recovery/planning/`.

Output:

- update `blocking_ledger.md` actual baseline fields if they changed;
- no runtime code changes yet.

### Step 2: C01 Contract Core Data

Target modules:

- `src/agent/step_runner/task_contract.rs`
- `src/agent/step_runner/plan_prompt.rs`
- `src/agent/step_runner/evidence.rs`
- `scripts/eval_report.py`
- `tests/test_eval_report.py`

Implementation direction:

1. Extend `TaskContract` with lifecycle, deterministic constraints, and
   expected completion evidence.
2. Render the fields through existing prompt/evidence/eval paths.
3. Add tests proving values are deterministic from goal/profile/intent and
   required artifacts.
4. Record a bounded persistence decision:
   - if persisted through existing plan/session/evidence structures, test it;
   - if not persisted across commands, document the explicit boundary.

Proof:

- targeted `cargo test task_contract`
- eval report tests for new fields

### Step 3: C02 Request Signals And Admission

Target modules:

- `src/agent/step_runner/task_contract.rs`
- `src/agent/step_runner/plan_input.rs`
- `src/agent/step_runner/plan_lint/mod.rs`
- `eval/cases/focused/control-recovery/planning/task-contract-admission.yaml`

Implementation direction:

1. Add a deterministic request signal structure derived from public inputs.
2. Normalize task kind/admission decisions:
   - explicit intent wins;
   - clear goal/profile/path signals can admit;
   - conflicting signals produce `partial` or `conflict` with evidence.
3. Add lint/correction evidence for missing or conflicting admission when it
   affects plan ownership.
4. Update focused fixture expectations only when new fields are produced.

Proof:

- unit tests for new/modify/docs/data/investigation/ambiguous signals
- focused `task-contract-admission` case

### Step 4: C03 Behavior Obligation Projection

Target modules:

- `src/agent/step_runner/task_contract.rs`
- `src/agent/step_runner/deliverable_obligation.rs`
- `src/agent/step_runner/profiles.rs`
- `src/agent/step_runner/plan_lint/mod.rs`
- `scripts/eval_report.py`
- `eval/cases/focused/control-recovery/planning/behavior-obligation-projection.yaml`

Implementation direction:

1. Add deterministic behavior-delta obligation sources for:
   - Next.js manifest/dependencies/build/dev-port/route;
   - docs literal or docs deliverable;
   - data/schema deliverable if existing profile data supports it.
2. Enforce missing owner steps through plan lint only when the obligation can
   be determined without semantic guessing.
3. Propagate obligation code, owner, path, status, and missing owner evidence
   into eval reports.
4. Do not make profiles execute or select recovery workflows.

Proof:

- targeted task contract and plan lint tests
- focused `behavior-obligation-projection` case
- eval report test for projected fields

### Step 5: Documentation And Coverage

Update only after tests prove behavior:

- `docs/architecture.md`
- `docs/ultra-plan-run.md`
- `docs/evaluation.md`
- `docs/eval/loadmap2-phase22-task-contract-admission-20260623.md`
- coverage table row status for C01-C03 if each row reaches proof

Coverage status rule:

- C01-C03 can become `Implemented` only if unit, focused, and report proof
  pass.
- If a row remains below proof, leave coverage status as `Partial` and record
  the narrower blocker in `blocking_ledger.md`.

### Step 6: Verification

Run:

```bash
cargo fmt --check
cargo test task_contract
cargo test plan_lint
cargo test
python3 tests/test_eval_report.py
```

Focused eval:

```bash
# Exact command should follow eval/README.md and existing focused-run scripts.
# Required cases:
# - focused-task-contract-admission
# - focused-behavior-obligation-projection
```

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

If behavior changes require new roots, record them in the Phase22 eval report
and use those roots for final sign-off.

Final Phase22 root used:

```text
eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658
```

Broad sign-off was rerun with that focused-fixture root and returned
`status: pass`.

## Rollback / Split Rules

- If a change requires active job arbitration, move it out of Phase22 and into
  Phase25 or later with a split-forward row.
- If a change requires artifact role, workspace scope, or ownership changes,
  move it to Phase23.
- If a change requires completion evidence or evidence binding producer work,
  move it to Phase24 unless Phase22 only renders existing data.
- If a row cannot be proven after two targeted attempts, add a design review
  note to `blocking_ledger.md` before continuing.

## Review Result

Review findings applied:

- Split the concrete work by coverage row so implementation cannot close C01
  by satisfying only C02/C03 tests.
- Added explicit target modules and proof commands per row.
- Added rollback/split rules to prevent Phase22 from swallowing Phase23-25
  responsibilities.
- Kept focused eval as proof for model-facing planner behavior, with broad
  sign-off as regression evidence only.

## Final Result

All concrete work steps completed. C01-C03 are closed as `closed_proven` and
promoted to `Implemented` in the coverage table.
