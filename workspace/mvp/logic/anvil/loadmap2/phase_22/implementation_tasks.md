# Phase22 Implementation Tasks

Date: 2026-06-23 JST

## Task Status Legend

- `[ ]` not started
- `[~]` started / blocked on proof
- `[x]` complete

## Preparation

- [x] Confirm the current branch, dirty state, and CommandAgent commit before
      implementation.
- [x] Re-read `AGENTS.md`, `docs/philosophy.md`, `docs/architecture.md`, and
      `docs/adr/0002-contract-recovery.md`.
- [x] Confirm Anvil baseline against `anvil_source_baseline.md`.
- [x] Confirm C01-C03 still map to the same coverage rows before runtime
      changes.

## C01: Task Contract Core

- [x] Add or complete task contract lifecycle state.
      - Candidate values: created, admitted, projected, linted,
        evidence_rendered, completed, rejected.
      - Keep lifecycle observational; it must not drive hidden work.
- [x] Add or complete constraint projection.
      - Include profile, plan style, required artifacts, prohibited actions
        where deterministic, and source authority.
      - Do not invent semantic constraints from model prose.
- [x] Add expected completion evidence projection.
      - Include required artifacts, verifier expectations, and behavior
        obligation evidence names.
      - Feed existing completion/eval reporting paths.
- [x] Decide and implement bounded persistence.
      - Prefer step/phase/session-visible persistence already present in
        plan/evidence/session data.
      - If cross-command persistence is intentionally limited, document the
        boundary and prove it.
- [x] Add unit tests for lifecycle, constraints, expected completion evidence,
      and persistence boundary.

## C02: Request Inference And Admission

- [x] Add deterministic request signal extraction.
      - Use goal, profile, intent, required artifacts, and public plan fields.
      - Avoid LLM reclassification and provider/model-specific branches.
- [x] Add task-kind admission decision.
      - Admit clear task kind.
      - Mark ambiguous or conflicting input as partial/conflict with evidence.
      - Preserve existing user-specified `intent` authority when present.
- [x] Connect admission status to plan lint/correction evidence.
      - Missing or conflicting required task-kind facts must produce structured
        correction evidence.
- [x] Add unit tests for new, modify, docs, data, investigation, and ambiguous
      request signals.
- [x] Add or update focused `task-contract-admission` proof expectations.

## C03: Objective And Behavior Contract Projection

- [x] Add behavior-delta obligation projection from deterministic input.
      - Source from profile obligations, required artifacts, user-visible
        route/dev-port/build/docs/data requirements, and expected paths.
      - Keep obligation kinds common across profiles where possible.
- [x] Connect obligations to plan lint owners.
      - Missing owner step for setup/manifest/route/docs/data/test obligations
        should produce structured correction evidence.
- [x] Connect obligations to evidence/completion reporting.
      - Eval reports must expose obligation code, owner, status, target paths,
        and missing owner where applicable.
- [x] Add unit tests for Next.js manifest/route/build/dev-port obligations.
- [x] Add unit tests for docs literal and data/schema obligation projection
      where existing profiles support it.
- [x] Add or update focused `behavior-obligation-projection` proof
      expectations.

## Documentation

- [x] Update `docs/architecture.md` if task contract fields or boundaries
      change.
- [x] Update `docs/ultra-plan-run.md` if public plan input fields or planner
      expectations change.
- [x] Update `docs/evaluation.md` if eval fields or sign-off interpretation
      change.
- [x] Add Phase22 eval report under `docs/eval/`.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
      proof supports C01-C03 status changes.

## Evaluation

- [x] Run `cargo fmt --check`.
- [x] Run targeted Rust tests for:
      - `task_contract`
      - `plan_lint`
      - `plan_input`
      - eval report field rendering
- [x] Run `cargo test`.
- [x] Run focused eval cases:
      - `focused-task-contract-admission`
      - `focused-behavior-obligation-projection`
- [x] Run broad sign-off after behavior changes.

## Review Checklist

- [x] Every C01-C03 task has an owner layer and proof command.
- [x] No task closes by docs alone or CI alone.
- [x] No provider/model-specific runtime branch is introduced.
- [x] No hidden retry or hidden repair loop is introduced.
- [x] `source_alignment_matrix.md` and `reconciliation.md` agree with the
      coverage table.
- [x] Any split-forward row is narrower, same-surface, evidence-backed, and
      assigned to a downstream phase.

## Review Result

Review findings applied:

- Added explicit C01 persistence-boundary task so "cross-command persistence"
  cannot be silently ignored.
- Added C02 ambiguous/conflicting admission tests so request inference is not
  just positive-case classification.
- Added docs/data rollout tasks to prevent Next.js-only behavior projection.
- Required broad sign-off only after targeted proof, avoiding broad eval as a
  substitute for row closure.

## Implementation Result

Phase22 is closed as `closed_proven` for C01-C03.

Proof:

- `cargo fmt --check`: passed
- `cargo test task_contract`: passed
- `cargo test plan_lint`: passed
- `python3 tests/test_eval_report.py`: passed
- `python3 tests/test_eval_signoff.py`: passed
- `cargo test`: passed
- `cargo build --release`: passed
- focused fixture root:
  `eval/runs/loadmap2-phase22-focused-fixtures/20260623T102658`
  - success: 2/2
  - focused assertions: `passed_recheck`
- broad sign-off: passed using the Phase22 focused-fixture root plus existing
  smoke/focused/large roots

No Phase22 row was split forward.
