# Phase25 Implementation Tasks

Date: 2026-06-23 JST

## Task Status Legend

- `[ ]` not started
- `[~]` started / blocked on proof
- `[x]` complete

## Preparation

- [x] Confirm the current branch, commit, and dirty state before
      implementation.
- [x] Re-read `AGENTS.md`, `docs/philosophy.md`, `docs/architecture.md`,
      `docs/adr/0002-contract-recovery.md`, and `docs/evaluation.md`.
- [x] Confirm Anvil baseline against `anvil_source_baseline.md`.
- [x] Confirm C11-C12 still map to the same coverage rows before runtime
      changes.
- [x] Inspect current active-job and dispatch modules:
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
- [x] Inspect current eval fields:
      - `active_job`
      - `recovery_owner`
      - `loop_control_action`
      - `dispatch_status`
      - `dispatch_reason`
      - `candidate_jobs`
      - `tie_break_reason`
      - `explicit_stop_reason`

## C11: Active-Job Arbitration Lifecycle

- [x] Inventory all current active-job candidates and derived active-job
      fallbacks.
- [x] Define the accepted lifecycle states for Phase25:
      - candidate
      - selected
      - not_applicable
      - no_owner
      - ambiguous_tie
      - explicit_stop
      - conflict_stop
- [x] Ensure candidate records carry owner, job, action, source layer,
      source-of-truth, target hint, artifact role, priority, tool policy,
      loop control action, rerun authority, and reason.
- [x] Add deterministic ordering and tie-break behavior:
      - stable priority ordering;
      - deterministic same-owner metadata merge where safe;
      - ambiguous stop for competing same-priority owners;
      - explicit no-owner stop when no candidate is eligible.
- [x] Ensure conflict-stop behavior records enough evidence for Phase28
      without claiming C33 conflict resolution.
- [x] Add tests for lifecycle, priority, tie-break, no-owner, same-owner merge,
      competing-owner stop, and conflict-stop cases.
- [x] Ensure no active-job behavior executes repair or setup directly.

## C12: Recovery Owner / Dispatch Gate

- [x] Inventory all candidate producers:
      - profile verification policy;
      - setup/dependency evidence;
      - manifest/config evidence;
      - route/import integration evidence;
      - source diagnostic evidence;
      - docs literal evidence;
      - evidence-binding failure evidence;
      - verifier contract evidence;
      - tool protocol failure evidence.
- [x] Connect candidate producers to one dispatch gate before repair prompt
      rendering.
- [x] Ensure dispatch produces exactly one of:
      - selected bounded repair task;
      - verifier-owned setup action;
      - tool protocol correction;
      - dev-server smoke action;
      - explicit structured stop.
- [x] Ensure `recovery_task` and `repair_brief` render the selected
      owner/action/target/tool policy and do not recompute them from prose.
- [x] Ensure dispatch output includes disallowed actions and rerun authority.
- [x] Ensure candidate list, dispatch reason, selected owner/action, and
      tie-break reason are visible in contract evidence and eval reports.
- [x] Add tests proving profile/setup/verifier/evidence/tool candidates reach
      dispatch and that competing candidates do not run multiple repairs.

## Focused Fixtures

- [x] Add or update focused cases for active-job owner/action proof:
      - setup dispatch;
      - manifest dispatch;
      - route integration dispatch;
      - source diagnostic dispatch;
      - docs dispatch;
      - evidence-binding dispatch;
      - verifier-contract dispatch;
      - tool-protocol dispatch;
      - no-owner explicit stop;
      - ambiguous tie explicit stop.
- [x] Ensure focused expected fields assert dispatch fields, not only final
      reason strings.
- [x] Reuse existing focused cases only when they assert C11/C12 fields
      directly.
- [x] Record focused proof root and row-to-case mapping in `reconciliation.md`.

## Documentation

- [x] Update `docs/architecture.md` if active-job or dispatch boundaries
      change.
- [x] Update `docs/adr/0002-contract-recovery.md` if dispatch semantics or
      explicit-stop policy changes.
- [x] Update `docs/evaluation.md` if eval fields or sign-off interpretation
      changes.
- [x] Update `docs/profiles.md` if profile candidate-hint responsibilities
      change.
- [x] Update `docs/ultra-plan-run.md` only if planner-facing recovery fields
      change.
- [x] Add Phase25 eval report under `docs/eval/`.
- [x] Update `docs/eval/legacy-control-stack-coverage-20260621.md` only after
      C11-C12 proof supports status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/current_issue_phase_map.md`
      if KI-004 status changes.
- [x] Update `workspace/mvp/logic/anvil/loadmap2/recovery_plan.md` or
      `README.md` only if exit gates or authority rules change.

## Evaluation

- [x] Run `cargo fmt --check`.
- [x] Run targeted Rust tests for:
      - `active_job`
      - `recovery_orchestration`
      - `recovery_policy`
      - `recovery_task`
      - `recovery_contract`
      - `repair_brief`
      - `target_admission`
      - `runtime` dispatch consumption where changed
- [x] Run `python3 tests/test_eval_report.py` if eval report fields change.
- [x] Run focused eval proof for C11-C12 dispatch fields.
- [x] Run `cargo test`.
- [x] Run `cargo build --release`.
- [x] Run broad sign-off after behavior changes.

## Review Checklist

- [x] Every C11-C12 task has an owner layer and proof command.
- [x] No task closes by docs alone, CI alone, broad sign-off alone, or field
      existence alone.
- [x] Owner/action selection happens before repair prompt rendering.
- [x] Candidate producers do not execute actions or become workflow engines.
- [x] Profiles produce candidate hints only; they do not arbitrate dispatch.
- [x] No provider/model-specific runtime branch is introduced.
- [x] No hidden retry, hidden continuation, or hidden repair loop is
      introduced.
- [x] No implicit dependency setup is introduced in normal repair.
- [x] No C33 contract-conflict resolution is claimed in Phase25.
- [x] `source_alignment_matrix.md` and `reconciliation.md` agree with the
      coverage table.
- [x] Any split-forward row is narrower, same-surface, evidence-backed, and
      assigned to a downstream phase.

## Review Result

Review findings applied:

- Split C11 work into lifecycle/arbitration and C12 work into dispatch
  producer/consumer paths.
- Required owner/action proof before prompt rendering, preventing a plan that
  only adds eval labels.
- Added explicit no-owner, ambiguous tie, and conflict-stop cases to avoid
  generic source-repair fallback.
- Kept Phase26/27/28 work out of scope while preserving evidence handoff to
  those later phases.
- Required focused dispatch assertions for each owner/action family likely to
  appear in current eval failures.

## Implementation Result

All Phase25 tasks are complete.

Proof:

- `cargo test active_job`
- `cargo test recovery_orchestration`
- `cargo test recovery_task`
- `python3 tests/test_eval_report.py`
- focused fixture root `eval/runs/loadmap2-phase25-focused-fixtures/20260623T132110`
- focused assertions `passed: 10`
- recheck assertions `passed_recheck: 10`
- broad sign-off `status: pass`

C11 and C12 are closed without a split-forward blocker. Phase25 did not claim
Phase26 recovery-task semantics, Phase27 target/verifier lifecycle behavior,
or Phase28 contract-conflict resolution.
