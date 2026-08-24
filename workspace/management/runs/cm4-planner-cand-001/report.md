# 結果サマリ

- **preflight**: 親`4afcfa14d2389125adc326091e29f04ec7b1c2b5`はCI
  `32122826750`、acceptance `32122826674`ともに`completed/success`。
- **計器・封緘**: execution revision `f2072b570b5eddde06215e8025cce859335c7916`、
  binary SHA-256 `b9f9818602d34c1b383a1910bcaf0c8737d596bcf0d792f5b3e0399d330c13fa`。
  golden/schema/adversarial外側manifest SHA-256は従来値のまま全entry照合済み。
- **実測**: E/Fとも12/12完走。修復込みfullはE 8/12=66.7%
  [39.06%, 86.19%]、F 8/12=66.7% [39.06%, 86.19%]。一発fullも
  両arm 7/12=58.3% [31.95%, 80.67%]。
- **時間**: E p50/p95=59.0/74.8秒、F=148.5/540.45秒。E→Fはfull率
  0.0pp [Newcombe 95% CI −33.81pp, +33.81pp]のまま、p50 +89.5秒、
  p95 +465.65秒。
- **費用・identity**: plannerは両armともlocal $0。OpenAI executor費用は
  E $0.02128278、F $0.03342056、計$0.05470334。返却model ID 112件でdrift 0。
- **判定**: 採用はowner裁定事項のため`owner_adjudication_pending`。
  この分母ではhighはmediumに対してfull率を改善せず、時間分布を悪化させた。
- **未実施・逸脱・新規床**: 初回E preflightは`.env` export状態と欠落-key負例の
  衝突で`preflight cargo test failed`、0run・支出0。live preflightでは
  `--skip-suite-tests`を明示記録し、親CI正本と最終clean full suiteを使用する。
  新規model署名`community_planner_empty_response`を登録。製品補修なし。

# CM-4 planner candidate calibration

## 1. 事前宣言

2026-08-18T18:52:59+09:00、provider支出前に以下を固定した。

- 分母: E 12run + F 12run = 24run。各armは封緘golden 3種×4観測。
- E: planner=`qwen3.8:27b-mlx`/Ollama、`think=medium`。
- F: planner=`qwen3.8:27b-mlx`/Ollama、`think=high`。
- executor: 両armとも`gpt-5.6-luna`/OpenAI Responses/native。
- 比較基準A: qwen3.6 plannerのgolden-008/matrix-001 12runを引用し再実行しない。
  Aは旧計器世代なので、A↔E/Fの差には世代交絡がある。E↔Fは同計器の直接比較。
- 閾値・点予測を置かず、Wilson 95% CIとNewcombe差分CIを併記する。
  採用はowner裁定であり、自動採用しない。
- suite TOMLの`think`、suite SHA、binary SHA、execution revision、provider到達性、
  model IDを0run目で照合する。

## 2. A/E/F対比

| arm | planner / think | 一発full [Wilson 95% CI] | 修復込みfull [Wilson 95% CI] | p50 / p95 | cost total |
|---|---|---:|---:|---:|---:|
| A (引用) | qwen3.6:27b / omitted | 9/12=75.0% [46.77, 91.11] | 10/12=83.3% [55.20, 95.30] | 181.5 / 218.50秒 | $0.01611224 |
| E | qwen3.8:27b-mlx / medium | 7/12=58.3% [31.95, 80.67] | 8/12=66.7% [39.06, 86.19] | 59.0 / 74.80秒 | $0.02128278 |
| F | qwen3.8:27b-mlx / high | 7/12=58.3% [31.95, 80.67] | 8/12=66.7% [39.06, 86.19] | 148.5 / 540.45秒 | $0.03342056 |

| comparison | 修復込みfull率差 [Newcombe 95% CI] | 一発full率差 [Newcombe 95% CI] | p50差 | p95差 |
|---|---:|---:|---:|---:|
| A→E | −16.67pp [−46.75, +17.58] | −16.67pp [−47.58, +19.33] | −122.5秒 | −143.7秒 |
| A→F | −16.67pp [−46.75, +17.58] | −16.67pp [−47.58, +19.33] | −33.0秒 | +321.95秒 |
| E→F | 0.00pp [−33.81, +33.81] | 0.00pp [−34.57, +34.57] | +89.5秒 | +465.65秒 |

CIは広い。読みはすべて「このn=12/armでは」に限定する。特にAは計器世代交絡が
あるため、採用判断の直接比較にはE/Fを優先する。

## 3. タスク別

| arm | warikan full / p50 / p95 | mochimono full / p50 / p95 | vote full / p50 / p95 |
|---|---:|---:|---:|
| E | 4/4 / 54.0 / 64.55秒 | 2/4 / 59.0 / 74.90秒 | 2/4 / 61.5 / 72.10秒 |
| F | 2/4 / 335.0 / 636.85秒 | 3/4 / 145.0 / 164.10秒 | 3/4 / 143.0 / 148.85秒 |

## 4. 失敗署名

| arm | stop class | 件数 | 帰属 |
|---|---|---:|---|
| E | `community_computed_unregistered` | 1 | model |
| E | `community_spec_artifact_missing` | 1 | model |
| E | `community_verify_instruction_not_executable` | 2 | model |
| F | `community_esbuild_script_missing` | 1 | model |
| F | `community_planner_empty_response` | 1 | model |
| F | `community_verify_instruction_not_executable` | 2 | model |

Fの`community_planner_empty_response`原文は
`planner_empty_response: planner returned empty content on all attempts`。到達性、binary
pin、Ollama model IDは正常で、honest-failureのまま新規model署名として登録した。

## 5. 計器・費用・証跡

- E/Fの0run目gateはOllama `/api/tags`で`qwen3.8:27b-mlx`を確認し、OpenAI
  `/v1/models`とkey存在を確認した。key値は記録していない。
- suite metadataと全24 command argvで`think=medium/high`の一致を機械検査した。
- model return metadataはE 49件、F 63件、合計112件でdrift 0。
- 全24 artifactのrun単位scrubはpass。
- run所要合計およびcampaign windowはE 710秒、F 2,634秒、計3,344秒。
- raw campaignはrepositoryへ入れず、`summary.json`にuat-meta 2件とevents 93件の
  SHA-256を封緘した。

## 6. 結論

この分母では、mediumはAよりfull率点推定が16.7pp低い一方、p50を122.5秒短縮した。
highはmediumと同じfull 8/12だが、p50 +89.5秒、p95 +465.65秒だった。
従って`highは不調`という仮説については、この分母では少なくとも速度面で支持され、
full率改善は観測されなかった。候補既定の採用可否はowner裁定待ちとして停止する。
