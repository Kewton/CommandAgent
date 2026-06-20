# Logic 005 Contract Stack

Date: 2026-06-21
Base commit: `a8a4121`
Working tree: dirty during implementation

## Change

This slice broadens the visible contract stack with the Anvil mechanisms that
fit CommandAgent's minimal-loop architecture:

- public plan input normalization for YAML/JSON step plans
- artifact graph roles for setup/config/source/test/docs/generated/cache paths
- active recovery orchestration fields on contract evidence
- artifact completion jobs with target-only repair intent
- verifier selection and evidence binding for blocked/dependency/setup cases
- target admission and prioritization for recovery targets
- recovery task rendering for active job, repair action, tool policy, and
  attempt ledger fields

The runtime still has one execution engine. These mechanisms produce bounded
contract data and repair instructions; they do not add hidden retry loops,
provider/model-specific behavioral policy, or sidecar routing.

## Additional Fixes From Focused Eval

The follow-up smoke run exposed two narrow gaps:

- a step could successfully create its expected artifact and then fail because
  the same turn issued a blocked compound Bash command
- a plan could list final required artifacts while no mutation step owned one
  of those artifacts

The implementation now:

- probes completion after blocked Bash only when all expected paths exist and
  the original verifier passes
- rejects unowned required artifacts during plan lint when a mutation plan is
  present, and sends structured plan-correction evidence
- keeps ultra phase plans from treating final required artifacts as per-phase
  ownership requirements; final artifacts remain enforced at the final boundary

## Verification

Local checks:

```text
cargo fmt --check: pass
cargo test: pass
cargo clippy --all-targets -- -D warnings: pass
cargo build --release: pass
python3 tests/test_eval_report.py: pass
```

Focused local LLM eval:

```text
eval/runs/logic-005-smoke/20260621T004010
provider/model: ollama / qwen3.6:35b-a3b-coding-nvfp4
success: 2/3
ok: smoke-python-script, smoke-rust-cli
remaining failure: smoke-docs-readme semantic_mismatch README.md:usage
failure category: quality
contract layer: eval_success_contract
```

The remaining docs failure is not a missing-artifact, plan-decomposition, tool
policy, or recovery-orchestration failure. The generated README mentions
CommandAgent and contains a usage-like sentence, but not the literal term
`usage` required by the eval success contract. Treating that as pass would
require changing the eval semantic matcher or making the case prompt expose the
literal success criterion; that is intentionally left as a separate eval
quality-contract decision.
