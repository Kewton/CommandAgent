# Loadmap2 Phase 11 Tool Protocol Recovery

Date: 2026-06-22

## Scope

Phase 11 implements tool failure recovery and protocol correction as a
runtime-effective contract surface.

The change keeps provider transport, verifier repair, profile repair, and
dependency setup separate. A deterministic tool/prose/provider failure is
normalized into tool-protocol evidence, mapped to one bounded correction
action, run under a narrow tool allowlist, and then judged by the original
expected-path/verifier/profile authority.

## Implementation Summary

- Added a normalized `ToolProtocolFailure` payload for tool-argument schema
  failures, stale edit targets, action-required prose, final-answer contract
  failures with safe targets, invalid paths, and provider-transport parse
  evidence.
- Added `ToolProtocolCorrectionAction` for same-tool required-field correction,
  valid-JSON correction, read-before-edit, repository-evidence tool call,
  provider transport fallback, and explicit stop.
- Projected the selected action into recovery task prompts, contract evidence,
  eval report fields, and minimal-loop allowed tools.
- Added a minimal-loop tool allowlist so a correction turn cannot use tools
  outside the admitted action.
- Added eval/report fields for tool-protocol source, action, failed tool,
  missing field, required fields, correction spent, and correction exhausted.

## Local Verification

Baseline commit before this slice:

```text
3ec4e84668377a70b0dda42c3756f92d5d38bd1d
```

Checks run locally:

```text
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
python3 tests/test_eval_report.py
bash scripts/eval_smoke.sh
bash scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/loadmap2-phase11-dry-run --dry-run
python3 scripts/eval_report.py eval/runs/loadmap2-phase11-dry-run/20260622T151338 --cases-dir eval/cases/focused/control-recovery
```

Results:

- Rust tests: passed, 662 library tests plus integration/doc tests.
- Clippy: passed with `-D warnings`.
- Release build: passed.
- Eval report tests: passed, 19 tests.
- Eval smoke: passed.
- Focused control-recovery dry run: report generated successfully at
  `eval/runs/loadmap2-phase11-dry-run/20260622T151338`.

The focused dry run intentionally does not execute model/runtime cases, so its
report shows `success: 0/16` and `skipped_dry_run: 16`. That is expected for
schema/reporting verification.

## Remaining Follow-Up

- Broaden live E2E coverage for provider parse and stale-edit branches.
- Continue to keep provider-specific behavior out of shared recovery policy.
- Do not mark the legacy tool failure recovery row fully implemented until
  live E2E demonstrates the action branches across representative providers.
