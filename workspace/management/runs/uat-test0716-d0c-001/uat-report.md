# D-0c 非悪化ゲート計測レポート（uat-test0716-d0c-001）

## 結論

**PASS。G1〜G5をすべて満たし、D-0をクローズする。** D-0b後の
`df833ab`をrelease buildして固定し、Next.js 6本とdata 6本を指定順に
各1回だけlive実行した。12/12が`run_stop`まで分類済みの理由を伴って
正直終端し、panic・分類不能・理由なき中断は0件だった。

fullは2/12（Quiz / gemma31、aggregation / gemma31 / profile）。Next.js
fullにはcontract-modeのbrowser interaction evidenceが、data fullには
実行済みpipeline probeとE1〜E4全PASSのevidenceが実在した。Quizは
1/2 fullで事前宣言したG4下限を満たす。その他の非fullはSpace、Breakout、
timeseriesおよびdataの既存能力バンド内で、assuranceのインフレ・デフレは
0件だった。

裁定eventについて、`ultra_final_acceptance`は81 keys、
`tui_command_stop`は43 keys、`run_stop`は54 keysで、歴代12本とのkey集合、
key signature、型集合が全て一致した。D-0b直前`6f261c0`とD-0b完了
`df833ab`のsource event名集合も148対148、追加0・削除0である。したがって
revert条件は発火せず、`7f26ad0..df833ab`のrevertは提案しない。

| Gate | 判定 | 計測事実 |
|---|---:|---|
| G1 正直終端 | **PASS** | `run_start` / `tui_command_stop` / `run_stop`が各12件。completed 2、具体理由付きfailed 10。panic・分類不能・理由なきinterrupt 0 |
| G2 偽成功ゼロ | **PASS** | full 2件のevidence実在を確認。false-full 0 |
| G3 assurance契約 | **PASS** | Next.js従来意味論、data契約§4に全12件が整合。インフレ0、デフレ0 |
| G4 バンド整合 | **PASS** | Quiz 1/2 full。Space / Breakout / timeseriesの非fullは正常帯、aggregationも既存混成帯 |
| G5 event形互換 | **PASS** | source event追加/削除0。裁定3 eventのkey/type/signature完全一致。byte fixture 6/6 green |
| 総合 | **PASS** | **D-0 close**。revert提案不要 |

機械可読の判定値は
[`gate-summary.json`](artifacts/analysis/gate-summary.json)、run別値は
[`run-matrix.json`](artifacts/analysis/run-matrix.json)、event全量比較は
[`event-comparison.json`](artifacts/analysis/event-comparison.json)に保存した。

## 対象と固定条件

- 実行日: 2026-07-16（Asia/Tokyo）
- repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- branch / HEAD: `develop` / `df833ab2bb3943e4b145653e3ccab4c157b48966`
  （`Record D-0b adjudication extraction`）
- D-0b範囲: `7f26ad0..df833ab`
- pre-D-0b event比較基点: `6f261c0`
- workspace: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0716_d0c_001`
- report: `workspace/management/runs/uat-test0716-d0c-001/`
- planner: Ollama `qwen3.6:27b-coding-nvfp4`
- executor: Ollama `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b`
- context budget: `65536`
- prompt layout: `legacy`（既定値、全runの`run_start`で確認）
- data input SHA-256:
  `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`
  （生成元、実行前6本、実行後6本で一致）
- 外側の`commandagent` invocation: 12回。各run 1回、再試行0回
- 同一run内のplanner corrective retry、bounded repair / replanは製品所定の
  経路であり、外側runの再試行には数えない
- 12本の`time_profile.total_ms`合計: 7,465,417 ms（124.424分）

### Goal原文

- Space:
  `あなたが考える最高に面白くかっこいいスペースインベーダーゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。`
- Breakout:
  `あなたが考える最高に面白くかっこいいブロック崩しゲームを3011ポートで起動可能なnext.jsアプリとして開発してください。`
- Quiz:
  `シンプルで美しいクイズアプリ（3問・スコア表示・リトライ可能）を3011ポートで起動可能なnext.jsアプリとして開発してください。`
- aggregation:
  `data/sales.csv を読み込み、月次×地域の売上集計と全体合計を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`
- timeseries:
  `data/sales.csv を読み込み、月次の売上合計・前月比（%）・3ヶ月移動平均を計算し、無効な行は理由別に除外して件数を明記した上で、要約レポートを作成してください。`

### コマンド形

全runを次の形で実行し、`<profile>`、`<preset>`、`<executor>`、`<goal>`だけを
事前行列どおり置換した。

```text
commandagent --model <executor> --provider ollama \
  --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama \
  --context-budget 65536 --ultra-plan-run --profile <profile> \
  --plan-preset <preset> --yes '<goal>'
```

Next.jsは全て`profile=nextjs / preset=profile`。dataは`profile=data`で、
Run 7 / 8 / 10が`preset=profile`、Run 9 / 11 / 12が`preset=none`である。

## Preflight

開始時に存在した別タスクの未追跡資料
`workspace/management/runs/uat-test0715-ff1-001/`は内容を変更せず対象パスだけを
一時stashし、clean確認後にpreflightと計測を行った。全run退避後に同じ資料を
復元し、stashはdrop済みである。本コミットには含めない。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 一時隔離後に空。tracked差分なし |
| HEAD / origin | `df833ab`そのもの、`origin/develop`と一致、`df833ab..HEAD`は空 |
| workspace / report新規性 | いずれも開始前に非存在 |
| disk | 339 GiB available |
| Ollama models | planner 1種、executor 2種すべて存在 |
| 権限付き`cargo test` | exit 0。lib 1346 passed / 13 ignored、byte fixture 6/6、conformance 18 passed / 1 ignored、corpus 1/1、data conformance 10/10、guardrail 7/7、doc tests 2/2を含め全green |
| `cargo build --release` | exit 0 |
| install | `target/release/commandagent`を`/Users/maenokota/.local/bin/commandagent`へinstall |
| target / install SHA-256 | 両方`d8ac1355fffcad948b5d56fbb524207e9ba9b4566db04c92127e4c36e332cc51` |
| `commandagent --version` | `commandagent 0.1.0 df833ab 2026-07-16T10:32:06Z`、`+dirty`なし |
| `--setup-interaction-probe` | `probe ready: playwright 1.61.1 (managed_interaction_probe)` |
| 3011 listener | Next.js各runの開始前・終了後とも残留なし |

全値の原文転記は[`preflight.json`](artifacts/analysis/preflight.json)に保存した。

## Run行列

`terminal / final`は最後の`run_stop.status / final_acceptance_status`、
`runtime / release`も同eventの投影値である。時間は対応する
`tui_command_stop.time_profile_total_ms`。

| # | run / run UUID | executor / preset | exit | terminal / final | assurance | runtime / release | 主要終端 | 時間 |
|---:|---|---|---:|---|---|---|---|---:|
| 1 | `d0c_01_space_qwen35`<br>`019f6a7e-d0f2-7352-81f5-1708585df3f7` | qwen35 / profile | 1 | failed / not_checked | partial (`acceptance_not_full_success`) | not_checked / not_applicable | Phase 2で`src/app/game-invaders.tsx`不在 | 65.490 s |
| 2 | `d0c_02_space_gemma31`<br>`019f6a80-481a-7621-846e-530d3f9484f5` | gemma31 / profile | 1 | failed / incomplete | partial (`missing_required_evidence:restart_or_recoverable_state_evidence`) | failed / failed | interaction state change不足、bounded repair枯渇 | 763.764 s |
| 3 | `d0c_03_breakout_qwen35`<br>`019f6a8c-b456-70c0-8eea-a6ffb0a7350a` | qwen35 / profile | 1 | failed / incomplete | partial (`missing_required_evidence:stateful_update_evidence`) | failed / failed | final repairがread-only stagnation | 454.989 s |
| 4 | `d0c_04_breakout_gemma31`<br>`019f6a94-93ce-7fb1-8b5a-0d5b27e62456` | gemma31 / profile | 1 | failed / incomplete | partial (`missing_required_evidence:restart_or_recoverable_state_evidence`) | failed / failed | interaction state change不足、bounded repair枯渇 | 1,295.241 s |
| 5 | `d0c_05_quiz_qwen35`<br>`019f6aa9-23cb-70f0-a9ef-acdbfca4d4a8` | qwen35 / profile | 1 | failed / incomplete | partial (`contract_instrumentation_missing:primary`) | pass / failed | contract repairがhook snapshot regressionで枯渇 | 454.086 s |
| 6 | `d0c_06_quiz_gemma31`<br>`019f6ab0-db70-7b32-bb7b-a83a79797546` | gemma31 / profile | 0 | completed / **full_success** | **full** | pass / pass | completed | 473.602 s |
| 7 | `d0c_07_aggregation_qwen35_profile`<br>`019f6ab8-e1f2-7eb1-9e2b-95a73b8f56d0` | qwen35 / profile | 1 | failed / not_checked | static (`data_profile_probe_not_run`) | not_checked / not_applicable | aggregation artifact recovery枯渇 | 814.346 s |
| 8 | `d0c_08_aggregation_gemma31_profile`<br>`019f6ac5-cf9d-7a50-9122-0651910ebe5e` | gemma31 / profile | 0 | completed / **full_success** | **full** | pass / pass | completed、E1〜E4全PASS | 1,135.201 s |
| 9 | `d0c_09_aggregation_qwen35_none`<br>`019f6ad7-c9f0-7fb0-99e7-90cde68edf32` | qwen35 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | not_checked / not_applicable | workspace外`docs/data-profile-contract.md`参照不在 | 459.259 s |
| 10 | `d0c_10_timeseries_qwen35_profile`<br>`019f6adf-326e-7060-9627-1472f8185214` | qwen35 / profile | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | not_checked / not_applicable | inspectionの`distinct_values.date`不足 | 674.173 s |
| 11 | `d0c_11_timeseries_qwen35_none`<br>`019f6ae9-dcca-7563-bba5-a3b9c23231c7` | qwen35 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | not_checked / not_applicable | Phase 1 artifact recovery枯渇 | 553.557 s |
| 12 | `d0c_12_timeseries_gemma31_none`<br>`019f6af3-1d99-7d21-a196-a9c8a1b887c6` | gemma31 / none | 1 | failed / not_checked | failed (`data_profile_script_not_generated`) | not_checked / not_applicable | verify command不在、corrective planning 3回後にscaffold失敗 | 321.709 s |

各runに`run_start`、`tui_command_stop`、`run_stop`が1件ずつ存在する。失敗10本の
完全な`stop_reason`、repair prompt、recovery UltraPlan YAMLは各artifactの
`.anvil/`に保存した。

## G1: 正直終端

- `run_start`: 12/12
- `tui_command_stop`: 12/12
- `run_stop`: 12/12
- `run_stop.status`: completed 2、failed 10。それ以外0
- failed 10本: 全件で非空の具体的`stop_reason`と検証済みrecovery handoffを記録
- panic文字列を持つevent / stop reason: 0
- unclassified / unexplained interrupted: 0
- completion / release failureをcompleted/fullへ投影したrun: 0

したがってG1は**PASS**。

## G2: full evidenceと偽成功監査

### Next.js full: Run 6

| 項目 | 値 |
|---|---|
| final acceptance / assurance | `full_success` / `full` |
| runtime / release | `pass` / `pass` |
| browser readiness | `passed`、evidence file実在 |
| browser interaction | `passed`、`probe_mode=contract`、`contract_hook_status=usable` |
| action hook | `primary` |
| state dimensions | `questionIndex`, `score` |
| evidence | [`browser-interaction.json`](artifacts/d0c_06_quiz_gemma31/.anvil/evidence/browser-interaction.json)、[`browser-readiness.json`](artifacts/d0c_06_quiz_gemma31/.anvil/evidence/browser-readiness.json) |

Run 5は一時的に`interaction_verified_heuristic_only`へ到達したが、
`contract_instrumentation_missing:primary`としてpartial/incompleteを維持した。
heuristic-onlyからfullへの昇格は0件である。

### data full: Run 8

| Evidence | 判定 | 一次資料 |
|---|---:|---|
| pipeline probe | PASS、exit 0、実行済み | [`pipeline-run.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/pipeline-run.json) |
| E1 reconciliation | PASS、`60 = 58 + 2` | [`reconciliation.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/reconciliation.json) |
| E2 claims binding | PASS、全数値claim照合済み | [`claims-binding.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/claims-binding.json) |
| E3 rerun consistency | PASS、baselineとrerun一致 | [`rerun-consistency.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/rerun-consistency.json) |
| E4 schema assertions | PASS | [`results-schema.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/results-schema.json)、[`inspection-schema.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/inspection-schema.json) |
| 集約 | `status=full`、全6 checks true、reasons空 | [`data-assurance.json`](artifacts/d0c_08_aggregation_gemma31_profile/evidence/data-assurance.json) |

full 2件の全てにprofile所定の実行済みevidenceが存在し、evidence欠落fullは0。
したがってG2は**PASS**。

## G3: assurance契約監査

### Next.js

- Run 6だけがruntime、release、browser readiness、contract interactionを全て
  passして`full`を獲得した。
- Phase途中で停止したRun 1はknown profileの従来投影どおり
  `partial / acceptance_not_full_success`。
- Run 2〜5はruntime / interaction / contract gateの具体的失敗を保持し、
  全て`partial`。failed gateからのfull獲得は0。

### data契約§4

| 契約段階 | 対象run | 実測投影 | 判定 |
|---|---|---|---:|
| pipeline probe + E1〜E4全PASS | Run 8 | full | PASS |
| pipeline生成済み、probe未実行 | Run 7 | static | PASS |
| pipeline未生成 | Run 9〜12 | failed | PASS |
| pipeline実行成功＋E1/E3 pass、E2/E4未達 | 該当なし | partial 0 | N/A |

fullを未実行gateから獲得した例、実行済み全PASSを保守側へ落とした例はいずれも
0である。G3は**PASS**。

## G4: 能力バンド

- Quiz: Run 6がfull、Run 5がpartialで、**1/2 full**。事前下限
  「2本中1本以上」を満たす。
- Space: 0/2 full。歴代能力バンドで失敗が正常な族であり、両runとも正直な
  Phase / interaction理由を保持。
- Breakout: 0/2 full。state / restart evidence不足を厳格に拒否しており、
  偽成功ではない。
- aggregation: 1/3 full、static 1、failed 1。歴代のexecutor / preset分散内。
- timeseries: 0/3 full。Phase B sealで受容済みの0-full帯と整合。

G4は**PASS**。

## G5: event vocabularyとschema互換

### 比較方法

liveの比較対象はD-0c 12本。歴代12本は、Next.jsの
`uat-test0715-ff1-001` 6本、aggregationの`uat-test0715-data-007`から
qwen35/profile・gemma31/profile・qwen35/noneの3本、timeseriesの
`uat-test0716-data-009`からqwen35/profile・qwen35/none・gemma31/noneの
3本を採用した。

live model出力による分岐発火数は一致条件にしない。schema互換は次の三段で
検査した。

1. pre-D-0b `6f261c0`とpost-D-0b `df833ab`の`src/**/*.rs`にあるevent名
   literal集合を比較。
2. 12本統合で裁定authorityを投影する3 eventのkey集合、eventごとの
   key signature集合、JSON型集合を比較。
3. preflightの6本のdeterministic byte fixtureを行単位でreplay。

| 対象 | D-0c | 歴代 / pre-D-0b | 結果 |
|---|---:|---:|---:|
| source event名集合 | 148 | 148 | added 0 / removed 0 |
| `ultra_final_acceptance` | 11件、各81 keys | 11件、各81 keys | key / signature / type全一致 |
| `tui_command_stop` | 12件、各43 keys | 12件、各43 keys | key / signature / type全一致 |
| `run_stop` | 12件、各54 keys | 12件、各54 keys | key / signature / type全一致 |
| adjudication byte fixture | 6/6 PASS | 固定bytes | 一致 |

#### 観測分岐差とschema差の切り分け

統合live観測では片側だけで発火したeventが10名称ある。ただし、D-0c側だけの
4名称は全て`6f261c0`に既にemitterがあり、歴代側だけの6名称は全て
`df833ab`にもemitterが残る。したがって新出 / 消失ではなく、model出力と
停止phaseによる既存分岐のcoverage差である。

| 観測 | event名 | source確認 | schema判定 |
|---|---|---|---:|
| D-0c側のみ | `hook_snapshot_feedback`, `route_unbound_recovery`, `tool_policy_error`, `write_required_off_target_write_allowed` | 全4名称がpre-D-0b `6f261c0`に存在 | 新出0 |
| 歴代側のみ | `bash_path_confinement_rejected`, `context_truncation_suspected`, `final_acceptance_repair_no_source_change`, `path_fallback_evaluated`, `planner_quality_retry_degraded`, `tool_args_path_salvaged` | 全6名称がpost-D-0b `df833ab`に存続 | 消失0 |

共有eventのうち`loop_stop`、`recovery_prompt_saved`、`tool_execute`、
`planner_error`等で観測した任意field組合せの差も、既存variantの発火差であり、
D-0bによるfield追加・削除ではない。裁定結果を外部へ投影する3 eventは上表の
とおり全signatureが完全一致している。詳細は
[`source-event-vocabulary.json`](artifacts/analysis/source-event-vocabulary.json)と
[`event-comparison.json`](artifacts/analysis/event-comparison.json)に保存した。

### 12本統合event語彙表

`current-path` / `reference-path`は既存分岐の片側観測を表し、schemaの新出 / 消失を
意味しない。

| event | D-0c | historical 12 | observation |
|---|---:|---:|---|
| `artifact_stagnation_feedback` | 17 | 18 | both |
| `bash_path_confinement_rejected` | 0 | 1 | reference-path |
| `browser_probe` | 10 | 9 | both |
| `compile_snapshot_saved` | 49 | 61 | both |
| `completion_contract_bound` | 53 | 57 | both |
| `completion_verify` | 27 | 20 | both |
| `context_truncation_suspected` | 0 | 3 | reference-path |
| `contract_attribute_repair_guidance` | 7 | 2 | both |
| `contract_observation_incomplete` | 4 | 4 | both |
| `dependency_build_lifecycle` | 66 | 79 | both |
| `depth_profile` | 11 | 11 | both |
| `deterministic_step_plan_used` | 11 | 12 | both |
| `empty_response_escalation` | 3 | 5 | both |
| `empty_response_recovered` | 2 | 4 | both |
| `escalation_carryover` | 15 | 10 | both |
| `final_acceptance_cycle_complete` | 5 | 3 | both |
| `final_acceptance_cycle_summary` | 2 | 3 | both |
| `final_acceptance_deterministic_remedies` | 11 | 11 | both |
| `final_acceptance_repair_complete` | 5 | 5 | both |
| `final_acceptance_repair_exhausted` | 2 | 1 | both |
| `final_acceptance_repair_failed` | 2 | 2 | both |
| `final_acceptance_repair_no_source_change` | 0 | 2 | reference-path |
| `final_acceptance_repair_start` | 7 | 7 | both |
| `hook_snapshot_feedback` | 1 | 0 | current-path |
| `hook_snapshot_saved` | 18 | 24 | both |
| `host_env_contamination` | 12 | 12 | both |
| `inspect_command_normalized` | 15 | 13 | both |
| `loop_stop` | 61 | 68 | both |
| `path_fallback_evaluated` | 0 | 1 | reference-path |
| `phase_verification_result` | 34 | 41 | both |
| `plan_preset_resolved` | 12 | 12 | both |
| `planner_error` | 4 | 3 | both |
| `planner_plan_sanitized` | 23 | 26 | both |
| `planner_quality_issue` | 87 | 119 | both |
| `planner_quality_retry` | 1 | 1 | both |
| `planner_quality_retry_degraded` | 0 | 2 | reference-path |
| `planner_quality_warning` | 3 | 3 | both |
| `planner_raw_output_shape` | 26 | 30 | both |
| `planner_verify_command_normalized` | 3 | 5 | both |
| `preset_step_converted` | 38 | 50 | both |
| `preset_ultra_plan_used` | 9 | 9 | both |
| `probe_preflight` | 10 | 9 | both |
| `profile_behavior_probe` | 1 | 2 | both |
| `provider_turn_duration` | 175 | 221 | both |
| `read_only_stagnation_feedback` | 18 | 21 | both |
| `read_only_tool_rejected` | 6 | 9 | both |
| `recovery_prompt_saved` | 14 | 13 | both |
| `repair_regeneration` | 2 | 1 | both |
| `route_unbound_recovery` | 1 | 0 | current-path |
| `run_start` | 12 | 12 | both |
| `run_stop` | 12 | 12 | both |
| `runtime_bash_policy` | 61 | 81 | both |
| `state_binding_diagnosis` | 10 | 8 | both |
| `step_obligation_scope` | 65 | 71 | both |
| `step_prompt_contract` | 98 | 121 | both |
| `step_short_circuited` | 54 | 63 | both |
| `step_verify_failure` | 2 | 2 | both |
| `step_verify_repair` | 9 | 3 | both |
| `time_profile` | 12 | 12 | both |
| `tool_args_path_normalized` | 7 | 6 | both |
| `tool_args_path_salvaged` | 0 | 1 | reference-path |
| `tool_call_raw` | 179 | 245 | both |
| `tool_execute` | 171 | 229 | both |
| `tool_policy_error` | 1 | 0 | current-path |
| `tool_validation_error` | 2 | 2 | both |
| `tui_command_stop` | 12 | 12 | both |
| `ultra_context_initialized` | 12 | 12 | both |
| `ultra_final_acceptance` | 11 | 11 | both |
| `ultra_final_acceptance_failed` | 4 | 5 | both |
| `ultra_partial_artifact_summary` | 10 | 7 | both |
| `ultra_phase_complete` | 28 | 33 | both |
| `ultra_phase_context_attached` | 34 | 37 | both |
| `ultra_phase_context_updated` | 33 | 37 | both |
| `ultra_phase_execute_complete` | 28 | 33 | both |
| `ultra_phase_failed` | 6 | 4 | both |
| `ultra_phase_plan_validated` | 33 | 37 | both |
| `ultra_phase_profile_check` | 28 | 33 | both |
| `ultra_phase_scaffold_complete` | 33 | 37 | both |
| `ultra_phase_start` | 34 | 37 | both |
| `ultra_plan_complete` | 2 | 5 | both |
| `ultra_plan_generation_attempt` | 3 | 3 | both |
| `ultra_plan_generation_metadata_normalized` | 3 | 3 | both |
| `ultra_plan_generation_succeeded` | 3 | 3 | both |
| `ultra_plan_raw_output_shape` | 3 | 3 | both |
| `verify_canonicalized` | 21 | 19 | both |
| `verify_command_normalized_at_runtime` | 12 | 22 | both |
| `verify_default_bound` | 7 | 8 | both |
| `verify_repair_progress` | 1 | 1 | both |
| `verify_repair_turn` | 3 | 3 | both |
| `workspace_cd_stripped` | 1 | 3 | both |
| `write_required_off_target_write_allowed` | 1 | 0 | current-path |

G5は**PASS**。

## Manual UAT記録

### Target

- 対象: D-0b裁定骨格抽出 `7f26ad0..df833ab`
- revision / build: `df833ab`、release SHA-256 `d8ac…cc51`

### Preconditions

- macOS / zsh、Ollamaローカルprovider、指定3 model利用可能
- managed Playwright 1.61.1 ready
- 各run専用の新規workspace、Next.jsは3011、dataは同一SHAのCSV
- `NODE_ENV=production`はhost contamination eventとして記録され、verifier childは
  cleaned environmentで実行

### Steps / Expected result

1. preflightを全greenにする。
2. 固定12行を各1回実行する。
3. 各runのterminal、assurance、evidence、event schemaを採取する。
4. G1〜G5が全PASSであることを確認する。

期待値は、成功数を作ることではなく、能力バンド内での正直終端、fullの
earned evidence、assurance非膨張、event互換である。全て満たした。

### Regression / safety

- run再試行なし。失敗を通すためのgate緩和なし。
- 既存`.anvil/` namespace、historical evidence、source、tests、設計docsを変更なし。
- docs変更は依頼で明示されたmechanism ledgerのD-0c 1行だけ。
- 失敗runのworkspaceとrecovery資料を削除・修正せず退避。

## Artifacts

[`artifacts/`](artifacts/)には12 runそれぞれの`.anvil/`、source、data、
pipeline、output、evidence、manifest / lockfileを保存した。再生成可能な
`node_modules/`と`.next/`、コミット禁止のraw `*.log`（1件）を除外した。
同じ3除外条件でのlocal measurement workspaceからの
`rsync --checksum --dry-run`差分は0、events fileは12、総量は約3.2 MiB。

| # | artifact |
|---:|---|
| 1 | [`d0c_01_space_qwen35/`](artifacts/d0c_01_space_qwen35/) |
| 2 | [`d0c_02_space_gemma31/`](artifacts/d0c_02_space_gemma31/) |
| 3 | [`d0c_03_breakout_qwen35/`](artifacts/d0c_03_breakout_qwen35/) |
| 4 | [`d0c_04_breakout_gemma31/`](artifacts/d0c_04_breakout_gemma31/) |
| 5 | [`d0c_05_quiz_qwen35/`](artifacts/d0c_05_quiz_qwen35/) |
| 6 | [`d0c_06_quiz_gemma31/`](artifacts/d0c_06_quiz_gemma31/) |
| 7 | [`d0c_07_aggregation_qwen35_profile/`](artifacts/d0c_07_aggregation_qwen35_profile/) |
| 8 | [`d0c_08_aggregation_gemma31_profile/`](artifacts/d0c_08_aggregation_gemma31_profile/) |
| 9 | [`d0c_09_aggregation_qwen35_none/`](artifacts/d0c_09_aggregation_qwen35_none/) |
| 10 | [`d0c_10_timeseries_qwen35_profile/`](artifacts/d0c_10_timeseries_qwen35_profile/) |
| 11 | [`d0c_11_timeseries_qwen35_none/`](artifacts/d0c_11_timeseries_qwen35_none/) |
| 12 | [`d0c_12_timeseries_gemma31_none/`](artifacts/d0c_12_timeseries_gemma31_none/) |

分析artifact:

- [`preflight.json`](artifacts/analysis/preflight.json)
- [`run-matrix.json`](artifacts/analysis/run-matrix.json)
- [`gate-summary.json`](artifacts/analysis/gate-summary.json)
- [`event-comparison.json`](artifacts/analysis/event-comparison.json)
- [`source-event-vocabulary.json`](artifacts/analysis/source-event-vocabulary.json)

## 提出判定

G1〜G5全PASSのため、本レポートとartifactsおよびmechanism ledgerのD-0c 1行を
1コミットにまとめる。パリティ不成立時のrevert停止条件は発火していない。
