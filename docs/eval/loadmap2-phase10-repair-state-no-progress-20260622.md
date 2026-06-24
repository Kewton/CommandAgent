# Loadmap2 Phase 10: Repair State And No-progress Recovery

Date: 2026-06-22

## Scope

Phase 10 makes repair attempt state runtime-effective. A repair attempt now
records a bounded ledger with target, role, cluster, action, changed files,
before/after signatures, verifier authority, outcome, and reason. Repeated
ineffective outcomes can exhaust target, role, and cluster facts before the
next admission decision.

This phase does not add retries, hidden continuation, provider/model-specific
policy, or deterministic source-fix adapters. Tool/protocol and later
mechanical repair phases may consume the evidence, but Phase 10 only classifies,
records, routes, or stops.

## Implemented

- Added history-aware repair attempt classification:
  - `passed`
  - `noop`
  - `malformed`
  - `unsafe`
  - `duplicate`
  - `no_progress`
  - `improved_still_failing`
  - `worsened`
  - `explicit_stop`
- Added `switch_target` no-progress strategy before role switching when another
  admitted target is available.
- Propagated exhausted targets and roles as first-class `ContractEvidence`
  fields, in addition to existing repair-job-state lines.
- Added structured `safe_stop_payload` fields to `ContractEvidence`,
  `RecoveryTaskContract`, eval extraction, and reports.
- Recorded repair-plan admission rejection as an `explicit_stop` repair
  attempt when it consumes the repair slot.
- Updated migration coverage and design documentation.

## Verification

Local checks run:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
python3 tests/test_eval_report.py
bash scripts/eval_smoke.sh
bash scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase10-dry-run \
  --dry-run
```

Focused dry-run root:

```bash
eval/runs/loadmap2-phase10-dry-run/20260622T143503
```

## Residual Work

Phase 10 still needs focused live eval roots when a local model is selected for
runtime behavior proof. Broader profile-family live evidence is needed before
the legacy coverage table can mark the related migrated-control rows as fully
implemented.
