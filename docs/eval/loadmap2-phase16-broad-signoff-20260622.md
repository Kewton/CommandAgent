# Loadmap2 Phase 16 Broad Sign-off

Date: 2026-06-22
Base commit: `9b588c3`
Dirty during eval: yes
Binary: `target/release/commandagent`
Primary provider/model: `ollama:qwen3.6:27b-coding-nvfp4`

## Scope

Phase 16 added an eval-only broad sign-off checker and ran the required smoke,
focused, fixture, and large local LLM roots. The checker reads existing
`summary.tsv` / `recheck_summary.tsv` files only. It does not run
CommandAgent, mutate workspaces, retry cases, or change runtime behavior.

## Code And Docs Changes

- Added `scripts/eval_signoff.py`.
- Added `tests/test_eval_signoff.py`.
- Updated eval docs for broad sign-off gates.
- Updated eval timeout recording so `scripts/eval_agent_slice.sh` records
  `provider_transport:eval_timeout` instead of losing the root on timeout.
- Updated report mapping so profile dependency/version conflicts map to
  `manifest_repair` in new eval rows.

## Deterministic Checks

Executed before broad eval:

```text
cargo fmt --check: pass
cargo test: pass
cargo build --release: pass
python3 tests/test_eval_report.py: pass
python3 tests/test_eval_signoff.py: pass
bash scripts/eval_smoke.sh: pass
```

## Eval Roots

Focused deterministic fixtures:

```text
eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659
normal report: generated
recheck report: generated
result: 2/16 success, focused assertions 16 passed
```

Smoke local LLM:

```text
eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759
normal report: generated
recheck report: generated
result: 3/3 success
```

Focused local LLM:

```text
eval/runs/loadmap2-phase16-focused-local-llm/20260622T173940
normal report: generated
recheck report: generated
result: 9/27 success
focused assertions: 23 passed, 4 failed
coverage defects: focused-nextjs-endpoint-smoke raw rc:1
```

Large local LLM:

```text
initial 1200s run: started but produced no root before manual stop
time-boxed evidence root: eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
normal report: generated
recheck report: generated
result: 0/6 success
terminal states: provider_transport_failed 5, profile_contract_failed 1
```

The time-boxed large root used `--timeout-secs 120` after the original 1200s
attempt did not leave a root. It is evidence of broad-run blocker behavior, not
a release-quality large pass.

## Sign-off Checker

Command:

```bash
python3 scripts/eval_signoff.py --require-recheck \
  --root smoke=eval/runs/loadmap2-phase16-smoke-local-llm/20260622T173759 \
  --root focused=eval/runs/loadmap2-phase16-focused-local-llm/20260622T173940 \
  --root focused-fixture=eval/runs/loadmap2-phase16-focused-fixtures/20260622T173659 \
  --root large=eval/runs/loadmap2-phase16-large-local-llm-timebox/20260622T182149
```

Result:

```text
status: fail
```

## Blocking Findings

Focused local LLM blockers:

| case | blocker |
| --- | --- |
| focused-docs-literal-mismatch | expected source repair, observed explicit stop / step policy failure |
| focused-nextjs-dependency-setup | expected completed setup, observed explicit stop |
| focused-nextjs-endpoint-smoke | expected ok, observed plan lint failure and raw `rc:1` diagnostic in the recorded root |
| focused-nextjs-route-integration | expected ok, observed manifest repair / plan lint failure |

Large local LLM blockers:

| case group | blocker |
| --- | --- |
| 5 large cases | `provider_transport:eval_timeout` under the 120s evidence run |
| large-nextjs-app-modify | profile dependency version conflict mapped to source repair in the recorded root |
| all failed large rows | missing evidence binding and completion evidence fields in the recorded root |

## Interpretation

Phase 16 did not pass broad migration sign-off. The useful result is that the
failure is now explicit:

- smoke is clean;
- fixture proof still validates the focused deterministic rows in the original
  summary;
- focused real-LLM rows still expose assertion mismatches that need targeted
  follow-up;
- large local LLM execution is currently blocked by local model timeout /
  provider-transport behavior rather than by a clean owned task-quality stop;
- the eval harness now preserves timeout evidence instead of dropping the root.

## Follow-up

1. Rerun the focused failing cases after the `plan_lint.invalid_expected_path`
   extraction change so the endpoint smoke row no longer appears as raw `rc:1`.
2. Rerun large with a practical release timeout once local model throughput is
   stable, or use a faster approved local coding model for broad sign-off.
3. Confirm new large rows map Next.js dependency/version conflicts to
   `manifest_repair` instead of `source_implementation_repair`.
4. Decide whether timeout rows should carry explicit
   `evidence_binding_status=not_applicable` /
   `completion_evidence_status=not_applicable` in future reports.

## Decision

Phase 16 implementation is complete as an eval/report capability, but broad
local LLM migration sign-off remains blocked. The blocker is recorded and
actionable; it is not hidden as success.
