# Loadmap2 Phase 7 Target Admission

Date: 2026-06-22

## Summary

Phase 7 extends target admission and prioritization before repair prompt
rendering. The implementation keeps the existing minimal loop unchanged and
adds only deterministic contract data around the Recovery Orchestration
boundary.

## Added Contract Data

- target source of truth
- ownership source
- workspace scope
- target evidence freshness
- focused edit status
- current excerpt availability
- priority components
- target conflict reason

Candidate sources now include failure evidence, verifier diagnostics, selected
profile routes, required artifacts, setup manifests, tool read/write/edit
records, setup/scaffold deltas, completion evidence, evidence bindings,
workspace observations, and artifact-graph relations.

## Admission Behavior

Target admission rejects generated/cache paths, out-of-scope paths,
candidate-only paths, role mismatches, stale target evidence, focused edits
without a current excerpt, exhausted targets/roles/clusters, and file targets
for jobs that are not allowed to edit files.

When multiple admitted targets share the same best priority and point to
different paths, recovery stops with structured target-conflict evidence
instead of using path ordering as a hidden fallback.

## Checks

The focused unit coverage is:

- `cargo test target_admission --lib`
- `cargo test recovery_orchestration --lib`

Local verification for this change:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `cargo build --release`
- `python3 tests/test_eval_report.py`
- `bash scripts/eval_smoke.sh`
- `bash scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/loadmap2-phase7-dry-run --dry-run`
- `python3 scripts/eval_report.py eval/runs/loadmap2-phase7-dry-run/20260622T123406 --cases-dir eval/cases/focused/control-recovery`
- `python3 scripts/eval_report.py eval/runs/loadmap2-phase7-dry-run/20260622T123406 --cases-dir eval/cases/focused/control-recovery --recheck`

Focused dry-run produced `eval/runs/loadmap2-phase7-dry-run/20260622T123406`.
As expected for dry-run, case success is `0/16` and focused assertions are
`skipped_dry_run`; the purpose was to verify case/report wiring and the new
target-admission summary columns.
