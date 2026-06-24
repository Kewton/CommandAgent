# Current Local LLM All Cases Eval

Date: 2026-06-21

Commit at run start: `429cbbb2221513fea0acd1e597747db6b4c87221`

Dirty flag: yes. This report covers the current uncommitted working tree.

Binary: `target/release/commandagent`

Provider/model: `ollama` / `qwen3.6:35b-a3b-coding-nvfp4`

Runs: 1 per case

## Commands

The first attempt was blocked by sandboxed access to the local Ollama port.
The recorded results below are from the rerun with local Ollama access.

```bash
cargo build --release
scripts/eval_agent_slice.sh --cases-dir eval/cases/smoke --out eval/runs/current-local-llm-smoke-rerun --runs 1 --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_agent_slice.sh --cases-dir eval/cases/focused/control-recovery --out eval/runs/current-local-llm-focused-control-rerun --runs 1 --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_large_tasks.sh --runs 1 --out eval/runs/current-local-llm-large-rerun --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_report.py eval/runs/current-local-llm-smoke-rerun/20260621T135927
scripts/eval_report.py eval/runs/current-local-llm-smoke-rerun/20260621T135927 --recheck
scripts/eval_report.py eval/runs/current-local-llm-focused-control-rerun/20260621T135957
scripts/eval_report.py eval/runs/current-local-llm-focused-control-rerun/20260621T135957 --recheck
scripts/eval_report.py eval/runs/current-local-llm-large-rerun/20260621T140307
scripts/eval_report.py eval/runs/current-local-llm-large-rerun/20260621T140307 --recheck
```

## Run Roots

- smoke: `eval/runs/current-local-llm-smoke-rerun/20260621T135927`
- focused control-recovery: `eval/runs/current-local-llm-focused-control-rerun/20260621T135957`
- large: `eval/runs/current-local-llm-large-rerun/20260621T140307`

## Summary

Normal eval result:

- smoke: 2/3
- focused control-recovery: 7/8
- large: 0/6
- total: 9/17

Recheck result:

- smoke: 2/3
- focused control-recovery: 7/8
- large: 0/6
- total: 9/17

## Case Results

| Case | Result | Category | Contract layer | Reason |
| --- | --- | --- | --- | --- |
| `smoke-docs-readme` | fail | quality | eval_success_contract | `semantic_mismatch:README.md:usage` |
| `smoke-python-script` | pass | ok | ok | ok |
| `smoke-rust-cli` | pass | ok | ok | ok |
| `focused-data-schema-completion` | pass | ok | ok | ok |
| `focused-docs-literal-mismatch` | pass | ok | ok | ok |
| `focused-nextjs-dependency-setup` | fail | verifier | verification_contract | `rc:1` |
| `focused-nextjs-route-integration` | pass | ok | ok | ok |
| `focused-python-import-binding` | pass | ok | ok | ok |
| `focused-python-missing-test-artifact` | pass | ok | ok | ok |
| `focused-rust-cargo-verifier-binding` | pass | ok | ok | ok |
| `focused-tool-protocol-correction` | pass | ok | ok | ok |
| `large-fastapi-app-modify` | fail | planning | planning_contract | `missing:tests/test_app.py`; recheck: `rc:1` |
| `large-fastapi-app-new` | fail | verifier | verification_contract | `rc:1` |
| `large-nextjs-app-modify` | fail | profile | profile_contract | `profile_verification:nextjs_integration_artifact_missing`; recheck: `rc:1` |
| `large-nextjs-app-new` | fail | planning | planning_contract | `missing:package.json,app/page.tsx`; recheck: `rc:1` |
| `large-rust-app-modify` | fail | verifier | verification_contract | `rc:1` |
| `large-rust-app-new` | fail | verifier | verification_contract | `rc:1` |

## Notes

- The current all-cases local LLM run is not green.
- The focused control-recovery set regressed from the previous 8/8 recorded run
  to 7/8 in this run because `focused-nextjs-dependency-setup` failed its
  verifier.
- Large cases remain 0/6.
- The evaluation runner now records recovery owner/action fields for failures,
  but many failed cases still stop with `attempt_outcome=not_attempted` and
  `evidence_binding_status=unknown`.
