# Loadmap2 Phase 1 Verifier Diagnostics - 2026-06-22

## Scope

Implemented the first execution slice for `loadmap2` Phase 8: semantic failure
report and verifier diagnostics.

The change adds deterministic verifier diagnostic payloads and propagates them
through contract evidence, recovery tasks, semantic failure clusters, and eval
reports. It does not add retry budget, hidden continuation, or provider/model
specific policy.

## Local Verification

```text
cargo fmt --check
cargo test
cargo build --release
python3 tests/test_eval_report.py
```

All checks passed locally.

Focused eval dry-run:

```text
root: /private/tmp/commandagent-loadmap2-phase1-focused-dry/20260622T000639
cases: eval/cases/focused/control-recovery
runs: 1
mode: --dry-run
result: runner/report schema completed; assertions skipped by dry-run policy
```

The dry-run report rendered the new sections:

- Diagnostic Codes
- Selected Failure Clusters
- Semantic Failure Kinds
- Preferred Repair Roles
- Weak Verifier Reasons
- Admitted Cluster Targets

## Focused LLM Eval Attempt

An attempted real focused eval used:

```text
root: /tmp/commandagent-loadmap2-phase1-focused/20260621T235330
provider/model: ollama/qwen3.6:27b-coding-nvfp4
cases: eval/cases/focused/control-recovery
runs: 1
```

It was stopped after progressing through eight focused cases because a generated
Next.js dev server remained running on port `3011` inside the eval workspace
and the full case set did not complete in bounded interactive time. The partial
root is retained as local evidence, but it is not a pass/fail sign-off root.

This is not treated as a verifier diagnostics regression. It is a focused-eval
execution hygiene issue around dev-server process cleanup / timeout handling.

## Result

The phase is verified at unit, build, eval-report, and focused dry-run levels.
The full real LLM focused case set still needs a stable eval harness run after
the dev-server cleanup behavior is separately triaged.
