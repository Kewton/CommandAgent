# Phase24 Implementation Tasks

Date: 2026-06-23 JST

## Task Status Legend

- `[ ]` not started
- `[~]` started / blocked on proof
- `[x]` complete

## Preparation

- [x] Confirm the current branch, dirty state, and CommandAgent commit before
      implementation.
- [x] Re-read `AGENTS.md`, `docs/philosophy.md`, `docs/architecture.md`,
      `docs/adr/0002-contract-recovery.md`, and `docs/evaluation.md`.
- [x] Confirm Anvil baseline against `anvil_source_baseline.md`.
- [x] Confirm C07-C10 still map to the same coverage rows before runtime
      changes.
- [x] Inspect current producer and consumer modules:
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

## C07: Artifact Ledger Producers

- [x] Inventory all current artifact ledger sources:
      - artifact graph nodes
      - `Read`, `Write`, `Edit`, and generic tool records
      - verifier diagnostic mentions
      - workspace observations
      - setup deltas
      - scaffold deltas
- [x] Prove ledger merge behavior for role, lifecycle, ownership,
      source-of-truth, changed/read/created/observed/required flags, and
      diagnostic facts.
- [x] Add or complete producer paths so pass-side completion authority can see
      ledger entries rather than relying on terminal-state defaults.
- [x] Ensure generated/cache/raw-input/out-of-scope facts remain non-completion
      authority even when they are observed.
- [x] Expose ledger fields in eval/report output only from deterministic
      ledger facts.
- [x] Add focused fixture expectations for ledger signals.

## C08: Completion Evidence Producers

- [x] Inventory current completion evidence producers:
      - verifier pass/fail
      - command observation
      - file layout
      - docs section
      - structured data/schema
      - report completeness
      - profile-wide completion facts
- [x] Add or complete deterministic producers for missing docs/data/report and
      profile-wide facts.
- [x] Ensure producers consume already-observed facts and do not run tools,
      select repair jobs, or trigger setup.
- [x] Connect producer output to `evidence_authority` so missing, failed, stale,
      and passed evidence produce distinct terminal/eval fields.
- [x] Add tests proving completion evidence status and source-of-truth for
      pass, fail, missing, unbound, and stale cases.
- [x] Add focused fixture expectations for completion evidence fields.

## C09: Evidence Binding Producers

- [x] Inventory current evidence binding helpers:
      - manifest identity
      - import symbol / route integration
      - executable handle
      - test script
      - required docs section
      - schema column
      - citation
      - file layout
- [x] Add or complete deterministic binding producers for each required binding
      family where observed facts already exist.
- [x] Ensure missing or failed binding becomes `evidence_binding` contract
      evidence with target, expected binding, failed step, repair target, and
      required literals.
- [x] Ensure bound evidence does not create a repair job.
- [x] Add tests proving binding kind/status/source are visible to completion
      authority and eval reporting.
- [x] Add focused fixture expectations for binding fields.

## C10: Deliverable Obligation Audit And Freshness

- [x] Inventory deliverable obligation projection from required artifacts,
      profile obligations, and task contract behavior obligations.
- [x] Prove deliverable kind mapping for source, setup manifest, test, docs,
      structured data, and report artifacts.
- [x] Add or complete freshness rules for:
      - must exist
      - edited this session
      - match current plan
      - have verifier evidence
- [x] Add read-only freshness checks so old observations do not satisfy fresh
      deliverable obligations.
- [x] Ensure obligation and freshness fields are projected into plan/profile
      facts and eval/report output without making profiles workflow engines.
- [x] Add focused fixture expectations for deliverable obligation and freshness
      fields.

## Documentation

- [x] Update `docs/architecture.md` if producer/authority boundaries change.
- [x] Update `docs/evaluation.md` if eval fields or report sections change.
- [x] Update `docs/profiles.md` if profile facts produce new evidence/binding
      hints.
- [x] Update `docs/ultra-plan-run.md` if planner-facing evidence obligations
      change.
- [x] Add Phase24 eval report under `docs/eval/`.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
      C07-C10 proof supports status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
      if KI-003 status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md` or
      `README.md` only if exit gates or authority rules change.

## Evaluation

- [x] Run `cargo fmt --check`.
- [x] Run targeted Rust tests for:
      - `artifact_ledger`
      - `completion_evidence`
      - `evidence_producer`
      - `evidence_authority`
      - `evidence_binding`
      - `deliverable_obligation`
      - `task_contract`
      - `plan_lint`
      - `artifact_completion`
- [x] Run `python3 tests/test_eval_report.py` if eval report fields change.
- [x] Run focused eval proof for C07-C10 producer fields.
- [x] Run `cargo test`.
- [x] Run `cargo build --release`.
- [x] Run broad sign-off after behavior changes.

## Review Checklist

- [x] Every C07-C10 task has an owner layer and proof command.
- [x] No task closes by docs alone, CI alone, or broad sign-off alone.
- [x] No provider/model-specific runtime branch is introduced.
- [x] No hidden retry, hidden evidence runner, or hidden repair loop is
      introduced.
- [x] No profile becomes a workflow engine.
- [x] Completion authority is fed by deterministic producer output where
      concrete facts exist.
- [x] `source_alignment_matrix.md` and `reconciliation.md` agree with the
      coverage table.
- [x] Any split-forward row is narrower, same-surface, evidence-backed, and
      assigned to a downstream phase.

## Review Result

Review findings applied:

- Split ledger, completion evidence, evidence binding, and deliverable
  obligation work into separate row-level task groups so C07 proof cannot mask
  C08-C10 gaps.
- Added producer-oriented tasks for docs/data/report/profile-wide evidence,
  manifest/import/test/docs/schema/citation bindings, and freshness checks.
- Added explicit boundaries against Phase25-27 work so this phase does not
  absorb active-job, setup/profile repair, target prioritization, or repair
  lifecycle responsibilities.
- Required focused proof for producer-visible fields, not only terminal
  success/failure.
- Required documentation updates only after runtime or eval behavior changes
  are proven.

## Implementation Result

All listed tasks are complete for Phase24.

- Focused proof root:
  `eval/runs/loadmap2-phase24-focused-fixtures/20260623T115617`
- Focused assertions: `passed: 6`
- Recheck assertions: `passed_recheck: 6`
- Broad sign-off: `status: pass`
- Coverage rows C07-C10 are updated to `Implemented`.
