# fix × data first measurement — uat-test0717-dfix-001

Date: 2026-07-17
Revision: `7f15729f69c470d6b9d592c48aedc70239e5235f` (`develop`)
Contract: `docs/fix-intent-contract.md` v0 fixed
Measurement workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_dfix_001`

## Result

**Overall: FAIL.** P0-a/P0-b/P0-c and P1-b passed, but P1-a failed. The schema goal emitted the data-profile `data_results_schema` suggestion in 3/3 runs; the exact pipe goal emitted no `fix_reproducer_suggested` event in 0/3 runs. The wording `実行がエラーで失敗` does not contain the currently recognized contiguous token `実行エラー`, so the first live pipe wording fell through the D-2a map.

All six runs terminated honestly as `failed / failed(after_not_executed)`. F1 was executed and recorded as passed in 6/6, F2 and F3 were not executed, and no run claimed full. The measured full rate is therefore 0/6; this rate is record-only.

One additional audit issue is material: Run 6's model-built schema R failed with a Python `SyntaxError` caused by escaped newlines before it evaluated `results.json`. The baseline gate recorded `before_fails=passed`; assurance nevertheless remained conservatively failed, so this was not a false full.

## Preflight

| Check | Result |
| --- | --- |
| Revision | `HEAD=7f15729`; required revision present; `origin/develop` matched before new records |
| Clean tree | PASS after temporarily isolating the pre-existing untracked `uat-test0715-ff1-001/`; its file hashes matched after restoration |
| `cargo test` | PASS; unit 1413 passed / 15 ignored, adjudication byte 6/6, fix conformance 9/9, corpus and data conformance green |
| Release build/install | PASS; build and installed SHA-256 `9f0ce7a416ee8d19ab1b22773bed1ab18a022628b8b5871c07bb3feabc537e5b` |
| Version | `commandagent 0.1.0 7f15729 2026-07-17T10:49:21Z`; no `+dirty` |
| Host environment | `NODE_ENV=production` recorded; each run emitted both contamination observation and `host_env_normalized(strategy=unset_inherited)` |
| Models | `qwen3.6:27b-coding-nvfp4`, `qwen3.6:35b-a3b-coding-nvfp4`, and `gemma4:31b` present |
| Interaction probe | Not applicable: data-only campaign |

The first release candidate was rejected before any UAT run because restoring the pre-existing untracked UAT path made its version report `+dirty`. The accepted binary was rebuilt from the isolated clean tree and then installed. Structured detail is in `analysis/preflight.json`.

## Baseline provenance

No broken source or CSV was synthesized. All sources already contained `data/sales.csv` with SHA-256 `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`; no supplement was needed.

| Set | Historical source | Principal source SHA | Procurement R and observed failure | Qualification |
| --- | --- | --- | --- | --- |
| pipe/A | `uat-test0714-m4-001/data_agg_qwen27_plan_qwen35_exec_preset_profile_001` | `pipeline/main.py` `49443221…` | `python3 -B pipeline/main.py` → exit 1, `ValueError` at line 53 | Exact match to retained `pipeline-run.json` and B-2d record |
| pipe/B | `uat-test0715-data-005/data5_qwen35_none_002` | `pipeline/main.py` `b27e8aaf…` | exit 1, `TypeError: list.append()` at line 164 | Real retained artifact, but source UAT stopped earlier as `artifact_follow_through_exhausted`; terminal-class concordance is not established |
| schema/A | `uat-test0713-data-001/data_agg_qwen27_plan_gemma31_exec_preset_profile_001` | `results.json` `a0e3a1df…` | product `data_results_schema` → `missing required key reconciliation` | Exact pre-contract invented schema |
| schema/B | same historical source, fresh independent copy | same | same | Only one unique retained invented-schema result was found; A/B baseline diversity is one |

The catalog helper initially failed at its own Cargo workspace boundary. That setup failure was preserved separately and was not counted as R. After isolating the helper workspace, both schema checks returned product exit 1 with the expected missing-key reason. See `artifacts/source-checks/`, `artifacts/source-records/`, and `analysis/source-provenance.json`.

## Run matrix

Each outer run was invoked once. There were no retries.

| # | Run / event run id | Family / set / executor | Verdict / assurance | Terminal class | Wall time |
| ---: | --- | --- | --- | --- | ---: |
| 1 | `dfix1_pipe_qwen35_001`<br>`019f6fb6-41de-72d1-bb20-6401a6885f69` | pipe / A / qwen35 | failed / failed (`after_not_executed`) | isolate-cause requested absent workspace path `docs/data-profile-contract.md` | 489 s |
| 2 | `dfix1_pipe_gemma31_001`<br>`019f6fbe-34d1-7520-bf79-0a40831d7520` | pipe / B / gemma31 | failed / failed (`after_not_executed`) | repair reached `pipeline/main.py`, then bounded read-only exhaustion | 1154 s |
| 3 | `dfix1_pipe_qwen35_002`<br>`019f6fd0-900d-7c30-bfa0-077bd548d258` | pipe / B / qwen35 | failed / failed (`after_not_executed`) | cause-isolation generated an implement step, edited the file without fixing R, then read-only exhaustion | 187 s |
| 4 | `dfix1_schema_qwen35_001`<br>`019f6fd3-e210-7801-9b53-85ef5284d109` | schema / A / qwen35 | failed / failed (`after_not_executed`) | isolate-cause requested absent `output/inspection.json` | 548 s |
| 5 | `dfix1_schema_gemma31_001`<br>`019f6fdc-c2ad-7471-ba59-22e80c501eb4` | schema / B / gemma31 | failed / failed (`after_not_executed`) | reached repair, then requested absent `output/inspection.json` | 948 s |
| 6 | `dfix1_schema_qwen35_002`<br>`019f6feb-dfef-7591-b166-e88e8947ff28` | schema / A / qwen35 | failed / failed (`after_not_executed`) | irrelevant `SyntaxError` R accepted as F1, then isolate-cause requested absent inspection | 585 s |

Distribution: pipe 0 full / 3 failed; schema 0 full / 3 failed. qwen35 0/4 full; gemma31 0/2 full.

## F evidence audit

Every run wrote both `fix-*-before.json` and `fix-*-adjudication.json`.

| Run | F1 before_fails | F2 after_passes | F3 no_regression |
| ---: | --- | --- | --- |
| 1 | pass: stage before, expected failure, epoch 1; `ValueError` from `pipeline/main.py` | not executed | bound 5, not executed |
| 2 | pass: before/failure/epoch 1; `TypeError` at line 164 | not executed | bound 5, not executed |
| 3 | pass: before/failure/epoch 1; same `TypeError` | not executed | bound 5, not executed |
| 4 | pass: before/failure/epoch 1; relevant custom schema assertion raised `AssertionError` | not executed | bound 5, not executed |
| 5 | pass: before/failure/epoch 1; relevant schema/reconciliation assertion raised `Schema mismatch` | not executed | bound 5, not executed |
| 6 | engine pass: before/failure/epoch 1; **UAT relevance fail** because escaped `\n` produced `SyntaxError` before schema evaluation | not executed | bound 5, not executed |

The frozen F3 set was byte-order-identical in 6/6 adjudication records:

1. `pipeline_probe`
2. `data_reconciliation`
3. `data_claims_binding`
4. `data_rerun_consistency`
5. `data_results_schema`

All five regression lineage hashes were also identical across all runs. No binding shrink occurred. Because F2 was never reached, `regressions=[]` in all six records and F3 honestly remained `not_executed`.

## Intent and R suggestion audit

`intent_resolved {value=fix, origin=cli, source=fix}` appeared exactly once in 6/6 runs.

| Family | Suggestion event | Model-selected R | Adoption audit |
| --- | --- | --- | --- |
| pipe, Runs 1–3 | **0/3**; expected `pipeline_probe` | `python pipeline/main.py` | independently relevant 3/3, but no profile suggestion existed to adopt |
| schema, Runs 4–6 | 3/3; `goal_profile_contract:data_results_schema` → catalog marker | custom `python -c` assertions | exact adoption 0/3; semantically relevant 2/3; Run 6 irrelevant syntax failure |

P1-a therefore fails. Across emitted suggestions, exact adoption was 0/3 and semantic adoption was 2/3. Across all six model-built R values, relevance was 5/6.

The pipe miss is deterministic: the live goal says `実行がエラーで失敗`, while the profile map recognizes terms including `実行エラー`, `traceback`, and `exit非ゼロ`. This is an observed vocabulary coverage gap, not an inference from executor randomness.

## Pipeline traceback wiring

For pipe Runs 1–3, F1 produced a `pipeline_error_extraction(status=extracted)` event and the generated Phase 2 plan contains the runtime-derived diagnostic block.

| Run | Extracted diagnostic | Phase 2 prompt | Target resolution |
| ---: | --- | --- | --- |
| 1 | `ValueError`, `pipeline/main.py:53`, `parse_amount` | present | initial `pipeline/main.py / traceback_mapped`; stopped before write pressure |
| 2 | `TypeError`, `pipeline/main.py:164`, `run` | present | write pressure `pipeline/main.py / traceback_mapped` |
| 3 | `TypeError`, line 164 initially; line 170 after unsuccessful edit | present | initial `traceback_mapped`; later verify-repair pressure used `required_path` after cause-isolation mutation |

The raw plans are under each run's `.anvil/plans/`; Run 2's retained write-pressure record is `.anvil/repairs/repair-read-only-stagnation-019f6fcf-d217-7562-a2b4-aecdace0009e.md`.

## Spoof-resistance observations

- `baseline_not_reproduced`: not exercised; the engine recorded F1 failure in 6/6.
- Lineage mismatch, regression shrink rejection, and epoch reversal: not exercised because no after/F3 evidence existed.
- False full: zero. No run exceeded failed assurance.
- Run 6 exposes a narrower negative case: a failing but irrelevant/syntactically invalid R can satisfy the current F1 outcome gate. It did not inflate assurance in this campaign because F2/F3 were absent.
- Run 3 wrote during the high-level cause-isolation phase, while its adjudication record retained `fix_written=false`; assurance stayed failed, but phase-role enforcement and write observation diverged.

## New fix × data failure classes

1. Pipe R suggestion vocabulary miss for the exact UAT wording (3/3).
2. Profile contract documentation path leaked into a workspace-local inspect step (Run 1).
3. Cause-isolation plan admitted an implement step and mutation (Run 3).
4. Historical schema baseline legitimately lacked `output/inspection.json`, but generated inspect/repair steps treated it as an unconditional readable prerequisite (Runs 4–6; terminal in 3/3).
5. Custom schema R syntax failure was accepted as before_fails (Run 6).
6. Planner regeneration dominated wall time: retries were concentrated in cause-isolation and repair planning, before useful writes.

## Gate table

| Gate | Result | Evidence |
| --- | --- | --- |
| P0-a honest terminal | **PASS** | 6/6 `run_stop`, all classified failed; panic 0 |
| P0-b assurance contract | **PASS** | 6/6 failed(`after_not_executed`), matching F1 pass and absent F2/F3 |
| P0-c false success | **PASS** | full 0, false-full 0 |
| P1-a data R suggestion | **FAIL** | schema 3/3, pipe 0/3 |
| P1-b data F3 binding | **PASS** | exact five bindings and stable lineages in 6/6; shrink 0 |

The campaign does not establish fix × data admission. It does establish that the shared fix adjudicator stayed conservative and that the data F3 set was wired correctly.

## D-2 cost record

| Phase | Elapsed | Measurement boundary |
| --- | ---: | --- |
| Acquisition | 756 s (12m36s) | first executable candidate audit through four verified baseline sets; preflight excluded |
| Execution campaign | 4156 s (69m16s) | campaign execution clock, including between-run inspection; sum of six run walls is 3911 s (65m11s) |
| Reporting | 395 s (6m35s) | evidence audit, report, archive, and integrity checks |

This cost is the D-2 measurement input. It is materially above a pure type-A wiring smoke because live planner regeneration and six local-model runs dominate execution time.

## Artifact index

- `analysis/preflight.json`: preflight facts.
- `analysis/source-provenance.json`: source paths, hashes, baseline R results, and qualification caveats.
- `analysis/run-matrix.json`: machine-readable run distribution.
- `analysis/fix-evidence-audit.json`: F1–F3, intent, suggestion, and traceback audit.
- `analysis/gate-results.json`: predeclared gate results.
- `artifacts/<run>/`: full small workspace snapshot including `.anvil`, evidence, source/output, console, timestamps, command shape, and outer exit.
- `artifacts/source-checks/`: procurement R stdout/stderr/exit; helper setup failure retained separately.
- `artifacts/source-records/`: copied historical source records and source SHA manifests.
- `artifacts/timing/`: phase clocks.
- `artifact-manifest.sha256`: SHA-256 inventory of archived artifacts.

No source, test, documentation, ledger, or band file was changed by this campaign.
