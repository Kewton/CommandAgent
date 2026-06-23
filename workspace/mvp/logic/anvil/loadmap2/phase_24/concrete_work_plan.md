# Phase24 Concrete Work Plan

Date: 2026-06-23 JST

## Execution Order

### Step 1: Baseline And Guardrails

Read and record:

- current CommandAgent commit and dirty state;
- C07-C10 rows in `docs/eval/legacy-control-stack-coverage-20260621.md`;
- current ledger/evidence/binding/obligation modules:
  - `artifact_ledger.rs`
  - `completion_evidence.rs`
  - `evidence_producer.rs`
  - `evidence_authority.rs`
  - `evidence_binding.rs`
  - `deliverable_obligation.rs`
  - `task_contract.rs`
  - `artifact_completion.rs`
  - `recovery_contract.rs`
  - `runtime/repair_loop.rs`
- current eval/report fields for ledger, completion evidence, evidence binding,
  freshness, and deliverable obligations.

Output:

- update `blocking_ledger.md` actual baseline fields if they changed;
- no runtime code changes yet.

### Step 2: C07 Artifact Ledger Producer Closure

Target modules:

- `src/agent/step_runner/artifact_ledger.rs`
- `src/agent/step_runner/artifact_graph.rs`
- `src/agent/step_runner/artifact_ownership.rs`
- `src/agent/step_runner/workspace_scope.rs`
- `src/agent/step_runner/evidence_authority.rs`
- `src/agent/step_runner/recovery_contract.rs`
- `scripts/eval_report.py`
- `scripts/eval_agent_slice.sh`
- `tests/test_eval_report.py`

Implementation direction:

1. Treat `ArtifactLedgerSummary` as the single source for artifact
   observation records.
2. Ensure ledger producers cover graph, read/write/edit, verifier mentions,
   workspace observations, setup deltas, scaffold deltas, and pass-side
   authority inputs.
3. Make ledger entries distinguish observed vs changed vs created vs required
   vs verifier-mentioned facts.
4. Ensure ledger entries carry role, lifecycle, ownership, source of truth,
   in-scope, generated/cache, and dependency/build output status.
5. Ensure eval report fields expose ledger signal presence without fabricating
   rows from terminal-state defaults.

Proof:

```bash
cargo test artifact_ledger
cargo test evidence_authority
python3 tests/test_eval_report.py
```

### Step 3: C08 Completion Evidence Producer Closure

Target modules:

- `src/agent/step_runner/completion_evidence.rs`
- `src/agent/step_runner/evidence_producer.rs`
- `src/agent/step_runner/evidence_authority.rs`
- `src/agent/step_runner/artifact_completion.rs`
- `src/agent/step_runner/profile_artifact.rs`
- `scripts/eval_report.py`
- `scripts/eval_agent_slice.sh`
- `tests/test_eval_report.py`

Implementation direction:

1. Treat completion evidence producers as pure converters from observed facts.
2. Add or complete evidence for verifier pass/fail, command observation, file
   layout, docs section, structured data/schema, report completeness, and
   profile-wide completion facts.
3. Ensure missing/failed/stale/unbound/passed evidence map to distinct
   `CompletionAuthorityResult` states.
4. Avoid hidden commands: producers may not run build/test/schema/doc checks.
   They may only consume deterministic observations already available.
5. Ensure eval rows expose completion evidence kind/status/source.

Proof:

```bash
cargo test completion_evidence
cargo test evidence_producer
cargo test evidence_authority
cargo test artifact_completion
python3 tests/test_eval_report.py
```

### Step 4: C09 Evidence Binding Producer Closure

Target modules:

- `src/agent/step_runner/evidence_binding.rs`
- `src/agent/step_runner/evidence_producer.rs`
- `src/agent/step_runner/evidence_authority.rs`
- `src/agent/step_runner/recovery_contract.rs`
- `src/agent/step_runner/recovery_orchestration.rs`
- `scripts/eval_report.py`
- `scripts/eval_agent_slice.sh`
- `tests/test_eval_report.py`

Implementation direction:

1. Add or complete producer functions for manifest identity, import/route
   symbol, executable handle, test script, docs section, schema column,
   citation, and file layout binding.
2. Ensure failed/missing/unbound binding becomes contract evidence with
   violated contract, target, expected binding, repair target, and required
   literals.
3. Ensure bound binding remains completion evidence and does not create a
   repair job.
4. Ensure completion authority prioritizes binding failures distinctly from
   missing deliverables and failed verifier evidence.
5. Ensure eval reports include binding kind/status/source when available.

Proof:

```bash
cargo test evidence_binding
cargo test evidence_producer
cargo test evidence_authority
python3 tests/test_eval_report.py
```

### Step 5: C10 Deliverable Obligation And Freshness Closure

Target modules:

- `src/agent/step_runner/deliverable_obligation.rs`
- `src/agent/step_runner/task_contract.rs`
- `src/agent/step_runner/plan_lint/mod.rs`
- `src/agent/step_runner/evidence_authority.rs`
- `src/agent/step_runner/artifact_completion.rs`
- `scripts/eval_report.py`
- `scripts/eval_agent_slice.sh`
- `tests/test_eval_report.py`

Implementation direction:

1. Ensure required artifacts and profile obligations project to
   `DeliverableObligation` records.
2. Ensure deliverable kinds cover source, setup manifest, test, docs,
   structured data, and report.
3. Ensure freshness rules cover existence, edited-this-session,
   match-current-plan, and verifier-evidence requirements.
4. Add read-only freshness checks so previous observations cannot satisfy
   fresh edit or current-plan requirements.
5. Ensure plan/profile/eval facts expose obligation kind, path, required
   evidence, and freshness status.

Proof:

```bash
cargo test deliverable_obligation
cargo test task_contract
cargo test plan_lint
cargo test evidence_authority
python3 tests/test_eval_report.py
```

### Step 6: Focused Fixtures And Report Proof

Target files:

- existing focused completion cases under
  `eval/cases/focused/control-recovery/completion/`
- new Phase24 fixture files if existing fixtures do not prove all C07-C10
  fields
- `scripts/eval_agent_slice.sh`
- `scripts/eval_report.py`
- `tests/test_eval_report.py`

Implementation direction:

1. Reuse existing completion fixtures only where they explicitly assert
   C07-C10 fields.
2. Add focused fixtures for missing producer fields:
   - ledger producer signals;
   - completion evidence kind/status/source;
   - evidence binding kind/status/target;
   - deliverable obligation and freshness fields.
3. Keep focused assertions narrow and deterministic.
4. Record a focused proof root in every closure path.

Proof:

```bash
scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery/completion \
  --out eval/runs/loadmap2-phase24-focused-fixtures \
  --runs 1 \
  --proof-mode deterministic_fixture
python3 scripts/eval_report.py eval/runs/loadmap2-phase24-focused-fixtures/<root> \
  --cases-dir eval/cases/focused/control-recovery/completion \
  --recheck
```

If fixtures are split across focused directories, record every root and the
specific row it proves.

### Step 7: Documentation And Coverage

Update only after tests prove behavior:

- `docs/architecture.md`
- `docs/evaluation.md`
- `docs/profiles.md`, if profile evidence/binding hints change
- `docs/ultra-plan-run.md`, if planner-facing obligations change
- `docs/eval/loadmap2-phase24-ledger-evidence-binding-20260623.md`
- `docs/eval/legacy-control-stack-coverage-20260621.md`
- `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
- `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md`
- `workspace/mvp/logic/anvil/loadmap2/README.md`

Coverage status rule:

- C07-C10 can become `Implemented` only if unit, focused, and report proof
  pass.
- If a row remains below proof, leave coverage status as `Partial` and record
  the narrower blocker in `blocking_ledger.md`.

### Step 8: Verification

Run:

```bash
cargo fmt --check
cargo test artifact_ledger
cargo test completion_evidence
cargo test evidence_producer
cargo test evidence_authority
cargo test evidence_binding
cargo test deliverable_obligation
cargo test task_contract
cargo test plan_lint
cargo test artifact_completion
python3 tests/test_eval_report.py
cargo test
cargo build --release
```

Broad sign-off:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=<Phase24 focused proof root> \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

## Rollback / Split Rules

- If a change requires active-job selection or dispatch lifecycle, split to
  Phase25.
- If a change requires setup/profile semantic repair or repair brief/action
  envelope, split to Phase26.
- If a change requires target prioritization, verifier repair lifecycle,
  patch validation, or no-progress strategy switching, split to Phase27.
- If a producer requires running a new hidden tool, do not implement it in
  Phase24. Record a binding/evidence limitation instead.
- If a row cannot be proven after two targeted attempts, add a design review
  note to `blocking_ledger.md` before continuing.

## Review Result

Implementation result:

- Steps 1-8 are complete.
- Focused proof root:
  `eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617`
- Focused assertions: `passed: 6`
- Recheck assertions: `passed_recheck: 6`
- Broad sign-off: `status: pass`
- C07-C10 are marked `Implemented` / `closed_proven`.

Review findings applied:

- Split the concrete plan by C07, C08, C09, and C10 so ledger closure cannot
  hide completion, binding, or freshness gaps.
- Added exact target modules and proof commands for each row.
- Made focused proof mandatory for producer-visible fields while allowing
  reuse of existing focused completion fixtures when they assert the required
  fields.
- Added rollback/split rules to prevent Phase24 from absorbing Phase25-27
  responsibilities.
- Kept broad sign-off as regression evidence only, not row-level proof.
