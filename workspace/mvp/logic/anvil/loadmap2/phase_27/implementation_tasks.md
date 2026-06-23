# Phase27 Implementation Tasks

Date: 2026-06-23 JST

## Task Status Legend

- `[ ]` not started
- `[~]` started / blocked on proof
- `[x]` complete

## Preparation

- [x] Confirm the current branch, commit, and dirty state before
      implementation.
- [x] Re-read `AGENTS.md`, `docs/philosophy.md`, `docs/architecture.md`,
      `docs/adr/0002-contract-recovery.md`, `docs/evaluation.md`, and
      `docs/profiles.md`.
- [x] Confirm Anvil baseline against
      `workspace/mvp/logic/anvil/loadmap2/anvil_source_baseline.md`.
- [x] Confirm C21-C32 still map to `P20-COV-003` and KI-006.
- [x] Inspect Phase22-Phase26 outputs and avoid reopening closed rows unless a
      narrow regression is proven.
- [x] Inventory current modules:
      - `target_admission.rs`
      - `artifact_graph.rs`
      - `artifact_ownership.rs`
      - `artifact_ledger.rs`
      - `repair_job.rs`
      - `runtime/repair_loop.rs`
      - `recovery_task.rs`
      - `semantic_failure.rs`
      - `verifier_diagnostic.rs`
      - `verifier_selection.rs`
      - `integrity_guard.rs`
      - `artifact_completion.rs`
      - `evidence_authority.rs`
      - `deliverable_obligation.rs`
      - `mechanical_repair.rs`
      - `repair_action_plan.rs`
      - `verify.rs`
- [x] Inventory current eval fields and focused fixtures for target
      admission, verifier diagnostics, repair lifecycle, no-progress,
      completion, focused edit, mechanical fallback, and patch validation.

## C21: Repair Target Decision / Admission

- [x] Inventory current target candidate/admission fields and rejection
      reasons.
- [x] Define common target admission inputs: active job, action envelope,
      artifact role, ownership, scope, source of truth, freshness, current
      excerpt, exhausted target/role/cluster, and disallowed target families.
- [x] Add route/source/test/docs/setup/evidence-binding target admission tests.
- [x] Add rejection tests for generated/cache/raw/out-of-scope/stale/missing
      excerpt/role-mismatch/disallowed targets.
- [x] Add focused target matrix.

## C22: Repair Target Prioritization

- [x] Inventory current target priority and tie-break behavior.
- [x] Define priority components for failure kind, authority, role, focused
      edit signal, evidence freshness, progress history, and source of truth.
- [x] Add deterministic ordering tests.
- [x] Add ambiguous same-priority stop tests.
- [x] Add focused prioritization fixture.

## C23: Repair Job State Machine

- [x] Inventory repair job lifecycle and verifier rerun result fields.
- [x] Define repair job lifecycle states and transition events.
- [x] Ensure verifier rerun outcome is recorded without replacing the original
      verifier.
- [x] Add safe-stop report fields for failed/no-progress/exhausted states.
- [x] Add repair job lifecycle tests and focused lifecycle fixture.

## C24: Repair Attempt Ledger

- [x] Inventory current attempt outcome and signature fields.
- [x] Define attempt outcome records for passed, noop, malformed, unsafe,
      duplicate, no-progress, improved-still-failing, worsened, and explicit
      stop.
- [x] Record target, role, cluster, changed files, before/after signatures,
      verifier result, and profile family.
- [x] Add eval report fields and tests for attempt outcome matrix.
- [x] Add focused attempt-ledger fixture matrix.

## C25: No-progress Recovery

- [x] Inventory no-progress strategy and exhausted target/role/cluster fields.
- [x] Define strategy branches:
      - switch target;
      - switch role;
      - evidence binding repair;
      - contract-conflict deferral to Phase28/C33;
      - scaffold/materialization repair;
      - explicit stop.
- [x] Add no-progress branch tests without increasing retry budgets.
- [x] Add contract-conflict deferral tests that do not claim C33 resolution.
- [x] Add focused no-progress matrix.

## C26: Verifier Diagnostic Assessment

- [x] Inventory current verifier diagnostic codes, payload fields, and weak
      verifier reasons.
- [x] Add or complete Rust, Python, Next.js, and common diagnostic
      classification fields.
- [x] Add weak target filters for source-grep, self-reference, generated-test,
      unsupported assertion, missing source excerpt, and ambiguous source of
      truth.
- [x] Ensure unknown diagnostics remain observable and counted.
- [x] Add verifier-diagnostic tests and focused verifier fixture.

## C27: Verifier Orchestration

- [x] Inventory verify step, repair loop, and rerun authority flow.
- [x] Add explicit verifier rerun events and outcome fields.
- [x] Add failure-attempt limits by job/failure class where deterministic.
- [x] Bind verifier rerun to evidence scope and original verifier authority.
- [x] Add verifier safe-stop report tests and focused verifier-rerun fixture.

## C28: Verifier Command Policy

- [x] Inventory verifier selection, plan-lint verifier checks, and integrity
      guards.
- [x] Add generated-test preflight and expectation-audit fields.
- [x] Reject self-referential verifiers and unsupported assertions before
      completion evidence is claimed.
- [x] Add tests for source-grep weakening, generated tests, self-reference,
      unsupported assertions, and expectation drift.
- [x] Add focused verifier-policy fixture.

## C29: Artifact Completion Job

- [x] Inventory artifact completion, evidence authority, deliverable
      obligations, and freshness fields.
- [x] Bind artifact completion to owned/in-scope ledger entries.
- [x] Keep missing deliverable, missing evidence, failed evidence, and stale
      evidence distinct.
- [x] Add completion job lifecycle and eval fields.
- [x] Add focused artifact-completion fixture.

## C30: Focused Edit Recovery

- [x] Inventory read/edit/write ledger signals and current excerpt handling.
- [x] Require current excerpt availability before focused edit admission.
- [x] Reject stale targets, changed-only targets, out-of-scope targets, and
      targets exhausted by C24/C25.
- [x] Add focused edit admission/rejection tests.
- [x] Add focused edit fixture.

## C31: Forced Small Edit / Deterministic Fallback

- [x] Inventory mechanical repair adapter and deterministic fallback output.
- [x] Define admission gate: owner/action/target/verifier authority must be
      present before fallback is rendered.
- [x] Ensure mechanical fallback proposes a bounded patch or instruction
      without mutating outside patch validation authority.
- [x] Add mechanical-repair tests for admitted and rejected fallback.
- [x] Add focused mechanical fallback fixture.

## C32: Patch Executor / Validation

- [x] Inventory patch validation, integrity guard, rollback admission, and
      repair loop integration.
- [x] Define patch validation outcomes for accepted, rejected unsafe,
      rejected noop, rejected duplicate, rejected test weakening, rejected
      protected/generated/cache/raw, and rollback admitted/rejected.
- [x] Ensure progress is claimed only after deterministic patch validation and
      original verifier rerun.
- [x] Add patch validation and rollback proof tests.
- [x] Add focused patch validation matrix.

## Documentation

- [x] Update `docs/architecture.md` if target admission, verifier
      orchestration, repair lifecycle, patch validation, or rollback admission
      boundaries change.
- [x] Update `docs/adr/0002-contract-recovery.md` if Phase27 admits a new
      recovery lifecycle, focused edit, mechanical fallback, or patch
      validation contract.
- [x] Update `docs/evaluation.md` for any eval field or sign-off changes.
- [x] Update `docs/profiles.md` only if profile facts consumed by Phase27
      change.
- [x] Update `docs/known-limitations.md` if rows split forward or carry an
      explicit limitation.
- [x] Add Phase27 eval report under `docs/eval/`.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
      row proof supports status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`,
      `recovery_plan.md`, and `README.md` if KI-006 status changes.

## Evaluation

- [x] Run `cargo fmt --check`.
- [x] Run targeted Rust tests:
      - `cargo test target_admission`
      - `cargo test repair_job`
      - `cargo test verifier_diagnostic`
      - `cargo test verifier_selection`
      - `cargo test integrity_guard`
      - `cargo test artifact_completion`
      - `cargo test evidence_authority`
      - `cargo test mechanical_repair`
      - `cargo test repair_action_plan`
      - `cargo test recovery_orchestration`
      - `cargo test repair_loop`
- [x] Run `python3 tests/test_eval_report.py`.
- [x] Run focused Phase27 fixture root with recheck.
- [x] Run broad sign-off with Phase27 focused root.
- [x] Run `cargo test`.
- [x] Run `cargo build --release`.

## Review Checklist

- [x] Every C21-C32 task has an owner layer and proof command.
- [x] No row is closed by docs alone, CI alone, broad sign-off alone, or field
      existence alone.
- [x] Target admission consumes Phase23/24/25/26 facts instead of recreating
      them.
- [x] Verifier orchestration reruns the original verifier and does not weaken
      success checks.
- [x] No-progress behavior remains bounded and visible.
- [x] Mechanical fallback cannot mutate without target and patch validation
      authority.
- [x] Patch validation rejects unsafe/noop/duplicate/test-weakening changes.
- [x] Profiles produce facts only; they do not arbitrate workflow behavior.
- [x] No provider/model-specific runtime branch is introduced.
- [x] No hidden retry, hidden continuation, or hidden repair loop is
      introduced.
- [x] No Phase28 contract conflict resolution is claimed in Phase27.

## Review Result

Review findings applied:

- Split Phase27 into twelve row-owned workstreams so broad repair lifecycle
  work cannot hide target, verifier, completion, focused edit, or patch gaps.
- Added explicit non-goals for Phase28 and Phase29 to keep the scope stable.
- Required common target/verifier/patch contracts before profile-specific
  expansion.
- Required rejection and safe-stop proof as first-class paths, not only
  selected repair success.
- Required patch validation proof before any repair attempt can claim progress.
