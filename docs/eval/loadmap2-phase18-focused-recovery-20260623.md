# Loadmap2 Phase18 Focused Recovery

Date: 2026-06-23 JST

## Scope

Phase18 closed the focused sign-off rows from Phase17:

- P17-F001 / S001: `focused-docs-literal-mismatch`
- P17-F002 / S002: `focused-nextjs-dependency-setup`
- P17-F003 / S003, S005: `focused-nextjs-endpoint-smoke`
- P17-F004 / S004: `focused-nextjs-route-integration`

Large local LLM rows remain assigned to Phase19.

## Changes

- Accepted safe dotfile expected paths such as `.gitignore` while keeping generated/cache dot paths blocked.
- Allowed read-only negated grep checks for docs while keeping source grep verifier checks blocked.
- Added provider transport fallback for native tool-call parse failures before falling back to XML tool calls.
- Reclassified verifier command targets for manifest/config files so package verifier failures repair `package.json`, not route source.
- Rejected over-constrained package verifier literals such as `grep -q '"3011"' package.json` and `grep -q '"next dev"' package.json`.
- Normalized focused recheck assertions for recheck lifecycle/completion-source and non-ok contract-layer projection.

## Targeted Proof Roots

| Row | Root | Result |
| --- | --- | --- |
| P17-F001 | `eval/runs/loadmap2-phase18-targeted-docs-v4/20260623T000427` | Focused assertions passed; recheck passed. |
| P17-F002 | `eval/runs/loadmap2-phase18-targeted-nextjs-dependency-v5/20260622T234925` | Focused assertions passed; recheck passed. |
| P17-F003 | `eval/runs/loadmap2-phase18-targeted-nextjs-endpoint-v2/20260622T235427` | Focused assertions passed; no raw `rc:*`; recheck passed. |
| P17-F004 | `eval/runs/loadmap2-phase18-targeted-nextjs-route-v2/20260622T235832` | Focused assertions passed; recheck passed. |

## Full Focused Proof

Command:

```bash
bash scripts/eval_agent_slice.sh \
  --cases-dir eval/cases/focused/control-recovery \
  --out eval/runs/loadmap2-phase18-focused-local-llm \
  --runs 1 \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --binary target/release/commandagent \
  --timeout-secs 900
```

Root:

```text
eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638
```

Result:

- normal report: `Focused Assertions - passed: 27`
- recheck report: `Focused Assertions - passed_recheck: 27`
- unknown/raw failure coverage defects: none

## Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

- status: fail
- focused findings S001-S005: none
- remaining findings: Phase19 large rows only

## Local Verification

Passed:

- `cargo fmt --check`
- `cargo test`
- `python3 tests/test_eval_report.py`
- `python3 tests/test_eval_signoff.py`
- `cargo build --release`
- `bash scripts/eval_smoke.sh`

## Remaining Work

Phase19 still owns the large-row findings:

- missing evidence binding / completion evidence on failed large rows
- large Next.js modify generic source fallback
- missing target on target-applicable large rows
