# Loadmap2 Phase 13 Profile/Language Parity

Date: 2026-06-22

## Scope

Phase 13 adds profile/language adapter parity as contract data. It does not
add a profile workflow engine.

Implemented contract records:

- `profile_project_kind`
- `profile_manifest_artifacts`
- `profile_entrypoints`
- `profile_integration_artifacts`
- `profile_completion_evidence`
- `profile_failure_mapping`
- `profile_adapter_families`
- `profile_capability_status`

The fields are rendered by the common profile output schema and summarized by
eval reports under `Profile Parity`.

## Boundary

Profiles can expose domain facts, failure mapping hints, and adapter-family
names. Recovery orchestration still owns active-job dispatch, target admission,
repair action selection, setup authority, and bounded stop behavior.

## Verification

Focused checks for this slice:

```bash
cargo test --quiet profile_fact_summary_renders_phase13_parity_fields_for_all_profiles
cargo test --quiet exposes_adapter_family_registry_for_profile_parity
python3 tests/test_eval_report.py
```

Full local signoff should still run the repository baseline:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

Focused dry-run executed:

```text
eval/runs/loadmap2-phase13-dry-run/20260622T161944
```

`scripts/eval_report.py` rendered the new `Profile Parity` section. The dry-run
root reports `Profile Parity: none` because dry-run workspaces do not invoke
the runtime profile fact producer. Runtime projection is covered by the Rust
profile-output parity tests, and report rendering is covered by
`tests/test_eval_report.py`.
