# Phase31 Implementation Report

Date: 2026-06-23 JST

Status: completed / closed_proven

## Scope

Phase31 closed `P20-LEDGER-001` / `P17-L001`.

The selected closure path was fresh large proof root only. No external
limitation packet was used.

## Implemented Changes

- Added eval-only `--no-timeout` support in `scripts/eval_agent_slice.sh`.
- Added timeout proof columns to eval summary/recheck output:
  - `timeout_mode`
  - `effective_timeout_secs`
- Updated `scripts/eval_report.py` recheck to read stdout/stderr and
  `.commandagent/repairs/*.md` from the run workspace.
- Updated evidence parsing so indented contract evidence such as
  `  - target_path: app/main.py` is captured.
- Added report projection for deterministic failed large rows so sign-off can
  distinguish owned failures from missing evidence.
- Updated `docs/evaluation.md` and `eval/README.md` to document `--no-timeout`
  as explicit proof-run reporting policy.

No runtime, provider, profile, or hidden retry behavior changed.

## Fresh Large Proof Root

```text
eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624
```

Command:

```bash
scripts/eval_large_tasks.sh \
  --runs 1 \
  --out eval/runs/loadmap2-phase31-large-non-timeboxed \
  --binary target/release/commandagent \
  --provider ollama \
  --model qwen3.6:27b-coding-nvfp4 \
  --no-timeout
```

Proof facts:

- provider/model: `ollama` / `qwen3.6:27b-coding-nvfp4`
- binary: `target/release/commandagent`
- commit recorded in run meta:
  `169da8b53cfcaea451c9828492ac93d906ceb6b2`
- `timeout_mode=none`
- `effective_timeout_secs=null`
- `provider_transport:eval_timeout` rows: 0

## Recheck

Command:

```bash
python3 scripts/eval_report.py \
  eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624 \
  --cases-dir eval/cases/large \
  --recheck
```

Result:

- wrote `recheck_summary.tsv`
- success: 1/6
- unknown/raw failure coverage defects: none
- all failed rows have owner, action, target, evidence binding, completion
  evidence, and attempt outcome

## Broad Sign-off

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase18-focused-local-llm/20260623T000638 \
  --root focused-fixture=eval/runs/loadmap2-phase29-runtime-support-fixtures/20260623T161335 \
  --root large=eval/runs/loadmap2-phase31-large-non-timeboxed/20260623T174624
```

Result:

```text
status: pass
```

## Row Closure

| row | disposition | evidence |
| --- | --- | --- |
| P17-L001 | `closed_proven` | Fresh no-timeout large root, large recheck, and broad sign-off pass. |

## Review Result

- Phase31 did not weaken semantic checks.
- Phase31 did not classify timeout as success.
- Phase31 did not add hidden continuation or retry behavior.
- Phase32 remains the only phase allowed to declare migration completion.
