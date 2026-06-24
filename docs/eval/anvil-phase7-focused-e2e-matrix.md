# Phase 7 Focused E2E Matrix

Date: 2026-06-21

## Scope

Phase 7 adds a focused control-recovery E2E matrix for the migrated control
paths from Phases 1 through 6.

Implemented surfaces:

- recursive focused case discovery
- eval-only `expected_*` field parsing
- focused assertion status in `summary.tsv` and `meta.json`
- focused assertion sections in `scripts/eval_report.py`
- a 16-case `eval/cases/focused/control-recovery` tree grouped by contract
  layer

## Required Case Coverage

| Required case | Case id |
| --- | --- |
| plan parser block scalar `>-` | `focused-plan-parser-block-scalar-chomp` |
| tool protocol missing required field | `focused-tool-protocol-missing-write-path` |
| missing artifact completion | `focused-missing-artifact-completion` |
| route integration repair | `focused-nextjs-route-integration` |
| Next.js dependency setup | `focused-nextjs-dependency-setup` |
| Next.js manifest repair after Tailwind drift | `focused-nextjs-tailwind-manifest-drift` |
| Next.js dev-server port conflict | `focused-nextjs-dev-server-port-conflict` |
| Next.js endpoint smoke | `focused-nextjs-endpoint-smoke` |
| Python missing test artifact | `focused-python-missing-test-artifact` |
| Python import binding | `focused-python-import-binding` |
| Rust Cargo verifier binding | `focused-rust-cargo-verifier-binding` |
| docs literal mismatch | `focused-docs-literal-mismatch` |
| data schema completion | `focused-data-schema-completion` |
| generated test weakening rejection | `focused-generated-test-weakening-rejection` |
| no-progress target switch | `focused-no-progress-target-switch` |
| contract conflict explicit stop | `focused-contract-conflict-explicit-stop` |

## Design Notes

The expected fields are intentionally eval-only. They are compared after the
run against observed runtime/report fields and are not passed into prompts,
plan generation, repair packets, or runtime decision logic.

This keeps Phase 7 aligned with the contract architecture:

```text
runtime evidence -> summary/meta -> focused assertion
```

not:

```text
focused assertion -> runtime behavior
```

## Verification Plan

Run:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
python3 tests/test_eval_report.py
scripts/check_branding.sh
scripts/eval_agent_slice.sh --dry-run --cases-dir eval/cases/focused/control-recovery --runs 1
scripts/eval_report.py <focused-dry-run-root> --cases-dir eval/cases/focused/control-recovery
scripts/eval_report.py <focused-dry-run-root> --cases-dir eval/cases/focused/control-recovery --recheck
```

Dry-run assertion status is expected to be `skipped_dry_run`.

## Verification Result

Executed during this phase:

- `python3 tests/test_eval_report.py`: passed, 11 tests
- `python3 -m py_compile scripts/eval_case_schema.py scripts/eval_report.py`:
  passed
- `bash -n scripts/eval_agent_slice.sh`: passed
- `scripts/eval_agent_slice.sh --dry-run --cases-dir eval/cases/focused/control-recovery --out /tmp/commandagent-phase7-focused-dry-run --runs 1`:
  passed
- focused dry-run root:
  `/private/tmp/commandagent-phase7-focused-dry-run/20260621T210349`
- `scripts/eval_report.py <focused-dry-run-root> --cases-dir eval/cases/focused/control-recovery`:
  passed; `Focused Assertions` reported `skipped_dry_run: 16`
- `scripts/eval_report.py <focused-dry-run-root> --cases-dir eval/cases/focused/control-recovery --recheck`:
  passed; dry-run recheck kept focused assertions as `skipped_dry_run`
- `scripts/check_branding.sh`: passed
- `cargo fmt --check`: passed
- `cargo test`: passed, 603 unit tests plus integration/doc tests
- `cargo build --release`: passed
- `cargo clippy --all-targets -- -D warnings`: passed
- `scripts/eval_smoke.sh`: passed

The focused dry-run intentionally reports runtime success `0/16` because no
model/runtime work is executed and expected artifacts are absent. The Phase 7
assertion layer reports `skipped_dry_run: 16`, which is the expected wiring
result for dry-run mode.

## Remaining Interpretation Limits

The focused matrix now has a place for every required case. A few cases are
positive-path probes for a recovery surface rather than deterministic
reproductions of malformed model behavior. In particular, malformed tool-call
output is still best tested with synthetic report/unit fixtures unless a
stable fake-model provider is added later.
