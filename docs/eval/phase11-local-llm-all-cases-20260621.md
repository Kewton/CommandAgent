# Phase 11 Local LLM All Cases Eval

Date: 2026-06-21

Commit at run start: `429cbbb`

Dirty flag: yes. This run used the Phase 11 working tree before commit.

Binary: `target/release/commandagent`

Provider/model: `ollama` / `qwen3.6:35b-a3b-coding-nvfp4`

Runs: 1 per case

## Commands

```bash
scripts/eval_agent_slice.sh --out eval/runs/phase11-local-llm-smoke --runs 1 --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_report.py eval/runs/phase11-local-llm-smoke/20260621T104953
scripts/eval_report.py eval/runs/phase11-local-llm-smoke/20260621T104953 --recheck

scripts/eval_large_tasks.sh --runs 1 --out eval/runs/phase11-local-llm-large --provider ollama --model qwen3.6:35b-a3b-coding-nvfp4 --binary target/release/commandagent --timeout-secs 900
scripts/eval_report.py eval/runs/phase11-local-llm-large/20260621T105034
scripts/eval_report.py eval/runs/phase11-local-llm-large/20260621T105034 --recheck
```

Run roots:

- `eval/runs/phase11-local-llm-smoke/20260621T104953`
- `eval/runs/phase11-local-llm-large/20260621T105034`

## Summary

Normal eval result:

- smoke: 2/3
- large: 0/6
- total: 2/9

Recheck result:

- smoke: 2/3
- large: 0/6
- total: 2/9

## Case Results

| Case | Result | Category | Contract layer | Reason |
| --- | --- | --- | --- | --- |
| `smoke-docs-readme` | pass | ok | ok | ok |
| `smoke-python-script` | pass | ok | ok | ok |
| `smoke-rust-cli` | fail | quality | eval_success_contract | `semantic_mismatch:src/main.rs:CommandAgent` |
| `large-fastapi-app-modify` | fail | planning | planning_contract | normal: `missing:tests/test_app.py`; recheck: `rc:1` |
| `large-fastapi-app-new` | fail | verifier | verification_contract | `rc:1` |
| `large-nextjs-app-modify` | fail | profile | profile_contract | normal: `profile_verification:nextjs_integration_artifact_missing`; recheck: `rc:1` |
| `large-nextjs-app-new` | fail | verifier | verification_contract | normal stderr shows bounded plan correction exhausted for `nextjs_typescript_toolchain_plan_contract`; recheck: `rc:1` |
| `large-rust-app-modify` | fail | verifier | verification_contract | initial turn tried to access missing `src/lib.rs`; repair packet saved |
| `large-rust-app-new` | fail | verifier | verification_contract | `verify-error-behavior` failed; repair packet saved |

## Observations

- The run completed all repository eval cases with the local LLM.
- The smoke failures are not runtime crashes; `smoke-rust-cli` failed the
  semantic success contract after `rc=0`.
- Large cases still do not converge. The observed failure classes are planning,
  profile verification, and verifier failures.
- Repair packets were produced for most large failures. The Next.js new-app
  failure stopped earlier at bounded plan correction for the TypeScript
  toolchain contract.
