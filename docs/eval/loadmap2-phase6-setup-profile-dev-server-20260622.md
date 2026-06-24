# Loadmap2 Phase 6 Setup/Profile/Dev-server Jobs

Date: 2026-06-22

Scope:

- setup job lifecycle record
- setup artifact validation evidence for Node, Cargo, and Python verifier
  commands
- common profile output schema across profiles
- dev-server smoke recovery classification fields
- eval/report extraction for setup lifecycle fields

Runtime-effective changes:

- Setup and manifest blockers now render a `SetupJobLifecycle` record with
  setup job kind, setup target, manifest kind/path, validation status,
  readiness, command authority, setup command, attempt keys, manifest
  fingerprint, stale reason, setup result, failure signature, verifier command,
  verifier rerun result, rerun authority, runtime job outcome, and explicit
  stop reason.
- Runtime verifier evidence now validates setup manifests for `npm run build`,
  `npm run dev`, Cargo build/check/test/run commands, and pytest commands when
  a Python manifest is present. Invalid manifests route to manifest repair
  evidence instead of generic source repair.
- Profiles now render a common output schema with root hints, classified
  artifacts, setup artifacts, scaffold artifacts, route/integration artifacts,
  verifier commands, protected paths, behavior obligations, verification
  failures, and recovery candidate hints.
- Dev-server smoke reports now include active job, owner, repair action,
  loop-control action, tool policy, dispatch status, and explicit stop reason
  in addition to requested-port smoke state.
- Eval extraction now records setup lifecycle fields such as
  `setup_job_kind`, `setup_manifest_kind`, `setup_artifact_validation_status`,
  `setup_readiness`, `setup_command_authority`, and
  `setup_failure_signature`.

Boundaries:

- The lifecycle record is side-effect free and does not execute setup.
- Rust/Python dependency installation remains unsupported; Phase 6 only adds
  manifest/setup evidence for those ecosystems.
- Profiles expose facts and candidate hints only. The shared dispatch gate
  still owns final active-job and action selection.
- Dev-server smoke remains bounded local evidence and does not keep a
  background server or run dependency setup.
- No hidden retry budget or provider/model-specific policy was added.

Verification target:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --release`
- `python3 tests/test_eval_report.py`
- `bash scripts/eval_smoke.sh`
- focused dry-run for `eval/cases/focused/control-recovery`

Local verification on 2026-06-22:

- `cargo fmt --check`: passed
- `cargo test`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `cargo build --release`: passed
- `python3 tests/test_eval_report.py`: passed
- `bash scripts/eval_smoke.sh`: passed
- focused dry-run:
  `bash scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/loadmap2-phase6-dry-run --binary target/release/commandagent --dry-run`
  wrote `eval/runs/loadmap2-phase6-dry-run/20260622T113401`
- focused dry-run report:
  `python3 scripts/eval_report.py eval/runs/loadmap2-phase6-dry-run/20260622T113401 --cases-dir eval/cases/focused/control-recovery`
  completed. Dry-run does not execute the model/runtime, so all 16 focused
  assertions were reported as `skipped_dry_run` and success was `0/16` with
  missing deliverables, as expected for offline wiring verification.
