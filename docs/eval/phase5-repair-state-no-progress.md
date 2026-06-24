# Phase 5 Repair State And No-Progress Report

Date: 2026-06-21

## Scope

Phase 5 implements bounded repair job state for the legacy-control loadmap.
The change records what a repair turn attempted after deterministic failure
evidence selected an active job, repair action, target, role, and failure
cluster.

This is not a retry-budget increase and does not add hidden continuation. The
minimal loop still receives one bounded repair task at a time, and the original
verifier, profile check, or guard remains the success authority.

## Implemented

- `RepairAttemptRecord` now carries attempt number, step id, active job,
  recovery owner, repair action, selected failure cluster, verifier command,
  before/after signatures, selected target, target role, changed files,
  outcome, and outcome reason.
- `RepairJobState` records bounded attempt ledgers, current signatures,
  exhausted targets, exhausted roles, exhausted clusters, no-progress strategy,
  and explicit stop reason.
- `repair_step_with_state` records attempt outcomes after malformed repair
  turns, unsafe patch-validation failures, verifier pass, and verifier failure
  reruns.
- Target admission receives exhausted failure clusters in addition to existing
  exhausted target and role facts.
- `ContractEvidence`, `RecoveryTaskContract`, orchestration evidence, and eval
  report fields expose repair attempt count, attempt outcome, reason,
  signatures, exhausted target/role/cluster, no-progress strategy, and repair
  state status.

## Non-Goals

- No hidden recovery loop.
- No provider/model-specific repair policy.
- No setup/profile/scaffold runtime job expansion beyond consuming existing
  active-job and evidence fields.
- No verifier weakening or success faking.
- No automatic rollback unless a future verifier-proven rollback mechanism
  supplies safe rollback data.

## Verification

Local verification on the dirty working tree before commit:

- `cargo fmt --check`: pass
- `cargo test`: pass, 598 unit tests plus integration tests
- `cargo build --release`: pass
- `python3 tests/test_eval_report.py`: pass, 9 tests
- `scripts/check_branding.sh`: pass
- `scripts/eval_agent_slice.sh --dry-run --cases-dir eval/cases/smoke --runs 1 --out /private/tmp/commandagent-phase5-eval-dry-run`: pass

Dry-run eval root:

- `/private/tmp/commandagent-phase5-eval-dry-run/20260621T191257`

The generated `summary.tsv` includes the new fields
`repair_attempt_count`, `attempt_outcome_reason`, `before_signature`,
`after_signature`, `exhausted_targets`, `exhausted_roles`,
`exhausted_clusters`, `no_progress_strategy`, and `repair_state_status`.
