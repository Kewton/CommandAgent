# Loadmap2 Phase31 Large Proof

Date: 2026-06-23 JST

## Summary

Phase31 closes `P20-LEDGER-001` / `P17-L001` as `closed_proven`.

The prior Phase16 large root was timeboxed and therefore proved attribution
only. Phase31 produced a fresh large root with eval timeout disabled explicitly
through `--no-timeout`, rechecked that root, and ran broad sign-off against the
fresh root.

## Code And Eval Changes

- Added eval-only `--no-timeout` support to `scripts/eval_agent_slice.sh`.
- Recorded `timeout_mode` and `effective_timeout_secs` in eval summaries.
- Updated recheck to read workspace repair packets so row-level diagnostic,
  target, and evidence fields are reconstructed from deterministic evidence.
- Updated docs to describe `--no-timeout` as proof-run reporting policy only.

These changes do not alter CommandAgent runtime, provider behavior, retry
policy, or verifier semantics.

## Proof Root

Fresh large proof root:

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

Root metadata:

- commit: `169da8b53cfcaea451c9828492ac93d906ceb6b2`
- dirty: true, because Phase31 docs/eval script edits were in progress
- provider/model: `ollama` / `qwen3.6:27b-coding-nvfp4`
- timeout mode: `none`
- effective timeout seconds: blank/null

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
- `provider_transport:eval_timeout`: 0 rows
- unknown/raw failure coverage defects: none
- all failed large rows have owner/action/target/evidence/attempt outcome

Large row outcomes after recheck:

| case | success | diagnostic | terminal state | target | evidence |
| --- | --- | --- | --- | --- | --- |
| `large-fastapi-app-modify` | false | `fastapi_response_mismatch` | `verifier_command_failed` | `app/main.py` | bound/failed |
| `large-fastapi-app-new` | false | `tool_args_missing_required_field` | `verifier_command_failed` | `tests/test_app.py` | bound/failed |
| `large-nextjs-app-modify` | false | `nextjs_integration_artifact_missing` | `profile_contract_failed` | `components/AnalyticsPanel.tsx` | bound/failed |
| `large-nextjs-app-new` | false | `read_only_step_mutation` | `step_policy_failed` | `app/page.tsx` | bound/failed |
| `large-rust-app-modify` | false | `tool_args_missing_required_field` | `verifier_command_failed` | `src/main.rs` | bound/failed |
| `large-rust-app-new` | true | `ok` | `ok` | n/a | bound/passed |

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

## Decision

`P17-L001` is `closed_proven`.

The proof does not claim all large tasks succeed. It proves the remaining
Phase31 blocker: large rows are no longer blocked by eval timeout, and the
remaining large failures are deterministic, owned, target-bound, and accepted
by broad sign-off.
