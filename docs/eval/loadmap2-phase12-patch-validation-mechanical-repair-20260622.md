# Loadmap2 Phase 12 Patch Validation And Mechanical Repair

Date: 2026-06-22

## Scope

Phase 12 connects patch validation, bounded mechanical repair hints, rollback
admission data, and eval projection to the recovery-control stack.

Implemented runtime boundaries:

- common `PatchProposal` and `PatchValidationReport` data
- deterministic patch validation for generated/cache/dependency/protected and
  out-of-scope paths
- repair-loop rejection of unsafe patch validation reports before progress is
  attributed
- bounded mechanical repair hint output for Rust, Python, and Node/Next
  diagnostics
- rollback admission fields for verifier-proven worsening and missing safe
  rollback data
- eval report sections for patch validation, mechanical adapters, and rollback
  admission

## Verification

Local checks:

```bash
cargo fmt --check
cargo test --quiet
cargo clippy --all-targets -- -D warnings
cargo build --release
python3 tests/test_eval_report.py
bash scripts/eval_smoke.sh
```

Result: passed.

Focused deterministic eval:

```bash
bash scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase12-dry-run \
  --dry-run
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase12-dry-run/20260622T155044 \
  --cases-dir eval/cases/focused/control-recovery
```

Run root:

```text
eval/runs/loadmap2-phase12-dry-run/20260622T155044
```

Dry-run result: `0/16` success by design because no LLM or runtime execution is
performed. The report includes the new `Patch Validation`,
`Mechanical Repair Adapters`, and `Rollback Admission` sections and the summary
TSV includes the corresponding fields.

## Interpretation

This phase makes patch validation and mechanical adapter state observable and
runtime-effective without adding hidden retries or provider-specific behavior.
Mechanical adapters still produce bounded hints/proposals; they do not mutate
files independently. Rollback remains conservative and is rejected unless
verifier-proven worsening and safe rollback data are both available.

## Remaining Limits

- Mechanical adapters do not execute direct source rewrites.
- Rollback execution is still gated by safe snapshot availability; Phase 12 only
  exposes admission status and rejection reasons.
- Focused eval coverage is deterministic dry-run coverage for field projection;
  model-quality E2E remains a separate sign-off.
