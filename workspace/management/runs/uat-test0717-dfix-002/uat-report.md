# fix × data second measurement — uat-test0717-dfix-002

Date: 2026-07-17
Revision: `a89db526f4b0f0d66c5dc55cb17d36d8c72d1935` (`develop`)
Contract: `docs/fix-intent-contract.md` v0 fixed
Measured changes: FIX-6a `903f340`, FIX-6b `a89db52`
Measurement workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_dfix_002`

## Result

**Overall predeclared gate: PASS.** P0-a/P0-b/P0-c and P1-a/P1-b all passed. FIX-6a closed the deterministic live wording gap: the exact pipe goal emitted `fix_reproducer_suggested` in 3/3 runs with `basis=goal_failure_kind:pipeline_execution`, and all three executors selected the exact suggested `python3 -B pipeline/main.py` R. The path fallback was not selected because the calibrated failure wording and canonical path were both present, and failure-kind vocabulary has deterministic precedence.

FIX-6b was not exercised. No generated R failed before evaluating its subject, so there was no `reproducer_defect`, `before-attempt-*` evidence, or reconstruction prompt. The escaped-newline `SyntaxError` seen in dfix-001 Run 6 did not recur, but this campaign therefore does not provide a live positive exercise of the classifier/rebuild branch.

All six runs terminated honestly as `failed / failed(after_not_executed)`. F1 was executed and passed in 6/6; F2 and F3 were not executed; no run claimed full. The measured full rate is 0/6, and the combined dfix-001 + dfix-002 rate is 0/12. Full rate, admission, band declaration, and D-2 closure remain reviewer decisions outside this commit.

## Acceptance scenarios

| Scenario | Expected result | Observed result | Status |
| --- | --- | --- | --- |
| FIX-6a exact pipe wording | `pipeline_probe` suggestion in 3/3 pipe runs, with vocabulary/path basis distinguishable | 3/3 suggestion; all used `goal_failure_kind:pipeline_execution`; exact adoption 3/3 | **PASS** |
| FIX-6b defective R handling | If an R self-destructs, classify and rebuild before F1; otherwise record unexercised | No defective R occurred; no defect evidence or rebuild; dfix-001 syntax failure did not recur | **PASS (not exercised)** |
| Fixed contract honesty | 6/6 classified terminals; assurance agrees with F1–F3 | 6/6 failed after F1, with F2/F3 absent and no false full | **PASS** |
| Data F3 freeze | Same manifest-derived five checks in 6/6; no shrink | Same ordered IDs and lineages in 6/6; shrink 0 | **PASS** |

The manual CLI scenario used the exact release command shape specified by the task. Each outer command was launched once from a fresh independent copy. There were no outer retries.

## Preflight

| Check | Result |
| --- | --- |
| Revision | `HEAD=origin/develop=a89db52`; required ancestor present |
| Clean tree | PASS after temporarily isolating the pre-existing untracked `uat-test0715-ff1-001/`; aggregate file hash `70c3eed2…` matched after restoration |
| `cargo test` | PASS; unit 1421 passed / 15 ignored, adjudication byte 6/6, fix conformance 9/9, corpus and data conformance green |
| Release build/install | PASS; build and installed SHA-256 `80eff03e43b66d4ce61f7c1b3b6156d8087e16cb6b2ce42938853e7f16a625cb` |
| Version | `commandagent 0.1.0 a89db52 2026-07-17T13:35:25Z`; no `+dirty` |
| Host environment | `NODE_ENV=production`; all six runs recorded contamination and `host_env_normalized(strategy=unset_inherited)` |
| Models | `qwen3.6:27b-coding-nvfp4`, `qwen3.6:35b-a3b-coding-nvfp4`, and `gemma4:31b` present |
| Interaction probe | Not applicable: data-only campaign |

Structured details are in `analysis/preflight.json`.

## Baseline provenance

No broken source or CSV was synthesized. All sources contain the historical `data/sales.csv` with SHA-256 `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`; no supplement was needed.

| Set | Historical source | Principal source SHA | Procurement R and observed failure | Qualification |
| --- | --- | --- | --- | --- |
| pipe/A | `uat-test0714-m4-001/data_agg_qwen27_plan_qwen35_exec_preset_profile_001` | `pipeline/main.py` `49443221…` | `python3 -B pipeline/main.py` → exit 1, `ValueError` at line 53 | Exact match to retained pipeline/B-2d records |
| pipe/B | `uat-test0715-data-005/data5_qwen35_none_002` | `pipeline/main.py` `b27e8aaf…` | exit 1, `TypeError: list.append()` at line 164 | Deterministic retained artifact; source UAT stopped earlier under a different terminal class |
| schema/A | `uat-test0713-data-001/data_agg_qwen27_plan_gemma31_exec_preset_profile_001` | `results.json` `a0e3a1df…` | product `data_results_schema` → missing `reconciliation` | Exact pre-contract invented-schema artifact |
| schema/B | same historical source, independent fresh copy | same | same | One unique retained invented-schema source exists; baseline diversity is one |

The initial schema helper call was sandbox-blocked before reading the baseline and was not counted as R. The permission-corrected product check then returned the expected exit 1 in A and B. See `artifacts/source-checks/`, `artifacts/source-records/`, and `analysis/source-provenance.json`.

## Command shape and run discipline

```text
commandagent --yes --intent fix --context-budget 65536 \
  --model <executor> --provider ollama \
  --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama \
  --plan-preset none --ultra-plan-run --profile data "<goal>"
```

The pipe and schema goals were byte-for-byte the task wording. Each run used a new copy of only `pipeline/`, `output/`, and `data/`; no Git history was carried in. Product-internal bounded planner regeneration and repair loops are part of one outer run and are reported as execution cost, not as UAT retries.

## Run matrix

| # | Run / event run id | Family / set / executor | Verdict / assurance | Terminal class | Wall time |
| ---: | --- | --- | --- | --- | ---: |
| 1 | `dfix2_pipe_qwen35_001`<br>`019f7051-422b-74c0-83a3-17af3a44e364` | pipe / A / qwen35 | failed / failed (`after_not_executed`) | repair reached the diagnosed target, then read-only write-pressure exhaustion | 952 s |
| 2 | `dfix2_pipe_gemma31_001`<br>`019f7060-a228-7962-956a-f65a93868ef1` | pipe / B / gemma31 | failed / failed (`after_not_executed`) | isolate-cause plan contained implement; executor stayed read-only and exhausted | 634 s |
| 3 | `dfix2_pipe_qwen35_002`<br>`019f706a-f0a2-7881-b5a3-081acc703af2` | pipe / B / qwen35 | failed / failed (`after_not_executed`) | same isolate-cause implement/read-only exhaustion | 399 s |
| 4 | `dfix2_schema_qwen35_001`<br>`019f7071-dd72-7583-8d73-dcef75c12f91` | schema / A / qwen35 | failed / failed (`after_not_executed`) | isolate-cause requested absent `output/inspection.json` | 233 s |
| 5 | `dfix2_schema_gemma31_001`<br>`019f7076-04b2-76b2-8a1f-59812ff29010` | schema / B / gemma31 | failed / failed (`after_not_executed`) | schema-key verify failed after bounded Phase 2 repair | 594 s |
| 6 | `dfix2_schema_qwen35_002`<br>`019f707f-f1ec-7f81-91ea-65479c4e718b` | schema / A / qwen35 | failed / failed (`after_not_executed`) | isolate-cause requested absent `output/inspection.json` | 286 s |

Distribution: pipe 0 full / 3 failed; schema 0 full / 3 failed. qwen35 0/4 full; gemma31 0/2 full. All outer exit codes were 1 and all six emitted a classified `run_stop`.

## F evidence audit

Every run wrote one `fix-*-before.json` and one `fix-*-adjudication.json`. No `after`, regression, or `before-attempt` evidence file was produced.

| Run | F1 before_fails | F2 after_passes | F3 no_regression |
| ---: | --- | --- | --- |
| 1 | pass: before/failure/epoch 1; exact suggested R; `ValueError` at `pipeline/main.py:53` | not executed | five bindings frozen; not executed |
| 2 | pass: before/failure/epoch 1; exact suggested R; `TypeError` at line 164 | not executed | five bindings frozen; not executed |
| 3 | pass: before/failure/epoch 1; exact suggested R; same `TypeError` | not executed | five bindings frozen; not executed |
| 4 | pass: before/failure/epoch 1; custom assertion read `results.json` then raised `AssertionError` | not executed | five bindings frozen; not executed |
| 5 | pass: before/failure/epoch 1; custom assertion read named schema keys then raised `AssertionError` | not executed | five bindings frozen; not executed |
| 6 | pass: before/failure/epoch 1; valid schema assertion, no syntax failure | not executed | five bindings frozen; not executed |

All adjudication bundles retained `fix_written=false`. The six assurance values therefore correctly remained failed with `after_not_executed`; there was no inflation or false full.

## FIX-6a audit — calibrated data R mapping

| Family / runs | Suggested basis and R | Model-selected R | Adoption |
| --- | --- | --- | --- |
| pipe, 1–3 | 3/3 `goal_failure_kind:pipeline_execution`; `pipeline_probe => python3 -B pipeline/main.py` | exact same command in 3/3 | exact 3/3, semantic 3/3 |
| schema, 4–6 | 3/3 `goal_profile_contract:data_results_schema`; catalog marker | custom one-line Python schema assertions | exact 0/3, semantic 3/3 |

P1-a passes: the dfix-001 pipe miss moved from 0/3 to 3/3. This live goal contains both the calibrated phrase `実行がエラーで失敗` and the literal path `pipeline/main.py`; the vocabulary basis wins deterministically, so `goal_path_mention` count is zero in this campaign. The path-only fallback was not a live condition here.

Pipe adoption also improved from “independently relevant without a suggestion” to exact catalog-command adoption 3/3. All three emitted `pipeline_error_extraction(status=extracted)` with `repair_target=pipeline/main.py` and `selection_reason=traceback_mapped`.

## FIX-6b audit — reproducer defect classification

- `failure_classification=reproducer_defect`: 0 events/evidence.
- `evidence/fix-*-before-attempt-*.json`: 0 files.
- F1 reconstruction feedback/prompt: 0.
- Valid subject-evaluating R: 6/6.

The branch is therefore **not exercised**, not failed. The dfix-001 Run 6 escaped-newline `SyntaxError` did not recur; all schema custom R values parsed, read `output/results.json`, and failed the named assertions. A later live gate would still be needed to positively observe defect classification and pre-F1 reconstruction.

## Frozen F3 set

The ordered binding set and all lineage hashes were identical in 6/6:

1. `pipeline_probe` — `regression:539f12e6adea8590`
2. `data_reconciliation` — `regression:aad7a27c9f14260b`
3. `data_claims_binding` — `regression:5ee07e4968be4199`
4. `data_rerun_consistency` — `regression:09f9be875c51316d`
5. `data_results_schema` — `regression:292fa5e5cb8065da`

No binding shrink occurred. Because F2 was never reached, `regressions=[]` in all six bundles and F3 honestly remained `not_executed`.

## Spoof-resistance observations

- `baseline_not_reproduced`: not exercised; relevant F1 failure was recorded in 6/6.
- Lineage mismatch, epoch reversal, and regression-shrink rejection: not exercised because no after/F3 evidence existed and no shrink attempt occurred.
- Reproducer self-defect: not exercised; zero defective R values.
- False full: zero. No run exceeded failed assurance.

## dfix-001 + dfix-002 combined distribution

Formal denominator is 12; there are no environment exclusions.

| Family | Executor | Full | Failed | Denominator | Full rate |
| --- | --- | ---: | ---: | ---: | ---: |
| pipe | qwen35 | 0 | 4 | 4 | 0% |
| pipe | gemma31 | 0 | 2 | 2 | 0% |
| schema | qwen35 | 0 | 4 | 4 | 0% |
| schema | gemma31 | 0 | 2 | 2 | 0% |
| **Total** | — | **0** | **12** | **12** | **0%** |

FIX-6 clears the two specified measurement defects but does not produce a fix × data full in this 12-run window. The dominant remaining path is after valid F1: Phase 2 planner regeneration, role leakage (implement inside isolate-cause), read-only executor stagnation, and an unconditional `output/inspection.json` assumption. These are distribution observations only; this task does not open or implement another fix.

## Gate table

| Gate | Result | Evidence |
| --- | --- | --- |
| P0-a honest terminal | **PASS** | 6/6 `run_stop`, all classified failed; panic 0 |
| P0-b assurance contract | **PASS** | 6/6 failed(`after_not_executed`), matching F1 pass and absent F2/F3 |
| P0-c false success | **PASS** | full 0, false-full 0 |
| P1-a pipe R suggestion | **PASS** | exact goal emitted suggestion 3/3; exact R adoption 3/3 |
| P1-b data F3 binding | **PASS** | exact five bindings and stable lineages in 6/6; shrink 0 |

## D-2 cost record

| Phase | Elapsed | Measurement boundary |
| --- | ---: | --- |
| Acquisition | 280 s (4m40s) | four fresh historical-source copies through R failure/SHA qualification; preflight excluded |
| Execution campaign | 3396 s (56m36s) | Run 1 start through Run 6 archive; sum of six run walls is 3098 s (51m38s) |
| Reporting | 545 s (9m05s) | evidence audit, structured analyses, report, manifest, and integrity checks |
| **Total measured** | **4221 s (70m21s)** | preflight excluded |

## Artifact index

- `analysis/preflight.json`: preflight facts.
- `analysis/source-provenance.json`: historical sources, hashes, R results, and caveats.
- `analysis/run-matrix.json`: machine-readable six-run and combined distributions.
- `analysis/fix-evidence-audit.json`: F1–F3, intent, FIX-6a adoption, FIX-6b non-exercise, and spoof-resistance audit.
- `analysis/gate-results.json`: predeclared gate results.
- `analysis/catalog-check/`: pinned product catalog-check helper used for the schema baseline qualification.
- `artifacts/<run>/`: workspace snapshot, `.anvil` events/plans/recovery evidence, fix evidence, source/output, console, timestamps, command shape, and outer exit.
- `artifacts/source-checks/`: procurement source snapshots and R outputs.
- `artifacts/source-records/`: immutable copies of historical provenance records.
- `artifacts/timing/`: acquisition, execution, and reporting clocks.
- `artifact-manifest.sha256`: SHA-256 inventory of archived artifacts.

No `src/`, `tests/`, `docs/`, ledger, or band file was changed by this campaign.
