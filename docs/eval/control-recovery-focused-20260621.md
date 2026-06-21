# Control Recovery Focused Eval

Date: 2026-06-21

Commit at run start: `429cbbb`

Dirty flag: yes. This report covers the uncommitted legacy-control adoption
working tree.

Binary: `target/release/commandagent`

Provider/model: `ollama` / `qwen3.6:35b-a3b-coding-nvfp4`

Runs: 1 per focused case

## Scope

This focused eval covers the visible, bounded control surfaces adopted from the
legacy control stack:

- contract evidence and recovery task rendering
- completion evidence and evidence binding
- deliverable obligations and freshness rules
- active job / recovery owner / target admission facts
- semantic failure report and repair action plan data
- repair job state, no-progress strategy, and safe rollback admission policy
- Next.js profile plan contracts for dependency setup, route integration, app
  layout, honest build scripts, and import aliases
- directory-only step materialization for generated plans

## Commands

```bash
cargo fmt --check
cargo test
cargo build --release
scripts/eval_smoke.sh
scripts/check_branding.sh
scripts/eval_agent_slice.sh --dry-run --cases-dir eval/cases/focused/control-recovery --out eval/runs/control-focused-dry-run-final --runs 1
scripts/eval_report.py eval/runs/control-focused-dry-run-final/20260621T122241
scripts/eval_report.py eval/runs/control-focused-dry-run-final/20260621T122241 --recheck
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/control-focused-local-llm-final6 --runs 1 --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_report.py eval/runs/control-focused-local-llm-final6/20260621T122253
scripts/eval_report.py eval/runs/control-focused-local-llm-final6/20260621T122253 --recheck
```

## Results

- `cargo fmt --check`: passed
- `cargo test`: passed, 571 unit tests plus integration/doc tests
- `cargo build --release`: passed
- `scripts/eval_smoke.sh`: passed
- `scripts/check_branding.sh`: passed
- focused dry-run recheck: 8/8
- focused local LLM E2E: 8/8
- focused local LLM E2E recheck: 8/8

Focused local LLM root:

- `eval/runs/control-focused-local-llm-final6/20260621T122253`

## Focused Cases

| Case | Result |
| --- | --- |
| `focused-data-schema-completion` | pass |
| `focused-docs-literal-mismatch` | pass |
| `focused-nextjs-dependency-setup` | pass |
| `focused-nextjs-route-integration` | pass |
| `focused-python-import-binding` | pass |
| `focused-python-missing-test-artifact` | pass |
| `focused-rust-cargo-verifier-binding` | pass |
| `focused-tool-protocol-correction` | pass |

## Notes

The final run did not need recovery jobs for these focused cases. Earlier local
LLM runs exposed deterministic contract gaps in Next.js app layout projection,
`@components/*` alias handling, package `private: true` false-positive linting,
TypeScript toolchain literal materialization, and directory-only setup steps.
Those gaps are now covered by unit tests and focused eval cases.
