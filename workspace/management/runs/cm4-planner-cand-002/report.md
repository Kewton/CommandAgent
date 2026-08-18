# 結果サマリ

- **preflight**: 作業開始親`e4ece796b5827c1e43e759b11420c031b8266d28`はCI
  `32132994172`、acceptance `32132994180`とも`completed/success`。
- **事前宣言・計器**: provider支出前に追加24run、E合算n=36、採用閾値なし、
  owner裁定、execution revision `f2072b570b5eddde06215e8025cce859335c7916`、
  binary SHA-256 `b9f98186…13fa`、suite SHA-256 `41b180bc…b2b`を固定した。
- **実測**: 追加24/24完走、full 16/24。既存12本との合算はone-shot
  18/36=50.0% [34.47,65.53]、修復込みfull 24/36=66.7%
  [50.33,79.79]、p50/p95=61.5/129.5秒。
- **A対比**: A→E36の修復込みfull率差は−16.67pp [Newcombe 95% CI
  −36.92,+14.38]、一発率差は−25.00pp [−47.37,+7.22]。p50は
  120秒短い。両品質差CIは0を含み、自動採用しない。
- **費用**: local planner API費用は$0。same-armを保つLuna executorの追加費用は
  `$0.05106268`、E36合計は`$0.07234546`。local電力は未計測で推計しない。
- **封緘**: golden/schema/adversarial外側SHAは従来値で全entry一致。全追加runの
  scrubはpass、返却model ID 105件でdrift 0。
- **未実施・逸脱・新規床**: 同revisionの通常再buildはbinary SHAが変わり0run目で
  fail closed（支出0）。同一計器を守るため既存の封緘binary実物を再照合して使用した。
  新規model署名2種は分類登録したが、製品境界は緩和していない。採用はowner裁定待ち。

# CM-4x planner candidate extension

## 1. 支出前宣言

宣言時刻は`2026-08-18T21:31:57+09:00`。Eの既存12runと同じ
qwen3.8:27b-mlx/Ollama/`think=medium` planner、gpt-5.6-luna/OpenAI
Responses/native executorを追加24runで測る。golden 3種を各8run追加し、合算後は
各種12run、各封緘goal変種4回となる。点予測・自動採用線は置かず、Wilson 95% CIと
A行へのNewcombe差分CIを記録する。採用はowner裁定である。

同一計器要件はexecution revision、suite SHA、binary SHA、provider到達性、
`think=medium` argv、返却model IDで支出前・実行時に検査した。

## 2. 計器preflightの除外窓

cleanな同revision checkoutで通常のrelease rebuildを行う既存preflightは、provider
支出前に次の原文で停止した。

> preflight BoN series binary SHA-256 pin mismatch: expected b9f9818602d34c1b383a1910bcaf0c8737d596bcf0d792f5b3e0399d330c13fa, observed 1bdb04aff8567219faad483d651051d268f8fc3259054ee70d525d9925e4f3ff

これは0run、provider費用$0で、分母へ入れていない。同revision再buildを同一binaryと
みなさず、CM-4 Eで封緘済みのbinary実物をSHA照合してcampaign配下へcopyし、実行前に
再照合する`cm4x_pinned_campaign.py`を用いた。製品判定やsuiteを変更していない。

## 3. E 24run拡張とn=36

| sample | one-shot full [Wilson 95% CI] | repair-included full [Wilson 95% CI] | p50 / p95 | cost total |
|---|---:|---:|---:|---:|
| E既存 | 7/12=58.3% [31.95,80.67] | 8/12=66.7% [39.06,86.19] | 59.0 / 74.8秒 | $0.02128278 |
| E追加 | 11/24=45.8% [27.89,64.93] | 16/24=66.7% [46.71,82.03] | 61.5 / 152.3秒 | $0.05106268 |
| **E合算** | **18/36=50.0% [34.47,65.53]** | **24/36=66.7% [50.33,79.79]** | **61.5 / 129.5秒** | **$0.07234546** |

追加campaign windowは1,764秒。追加24runのtask別fullはwarikan 4/8、
mochimono 6/8、vote 6/8。E36では各種8/12である。成果物levelはE36でL2 34件、
L3 2件。全36件のscrubはpass、返却model metadata 154件でdrift 0。

## 4. A行との差

| comparison | point difference | Newcombe 95% CI | time difference |
|---|---:|---:|---:|
| repair-included full A→E36 | −16.67pp | [−36.92,+14.38]pp | p50 −120.0秒 / p95 −89.0秒 |
| one-shot full A→E36 | −25.00pp | [−47.37,+7.22]pp | — |

Aは前計器世代の引用であり、世代交絡を残す。Eのnは36へ拡張したがAはn=12で、
両差分CIは0を含む。従って「EがAより品質で劣る／同等」とは断定せず、この分母では
速度点推定が短く、品質点推定が低いという裁定材料に限定する。

## 5. 失敗署名

| stop class | E既存12 | E追加24 | E合計36 | attribution |
|---|---:|---:|---:|---|
| `community_spec_artifact_missing` | 1 | 2 | 3 | model |
| `community_verify_instruction_not_executable` | 2 | 1 | 3 | model |
| `community_computed_unregistered` | 1 | 1 | 2 | schema設計 / global集約は既存QUEUED |
| `community_workspace_path_invented` | 0 | 2 | 2 | model |
| `community_dangerous_command_blocked` | 0 | 1 | 1 | model; safety gate pass |
| `community_package_missing` | 0 | 1 | 1 | frozen旧計器上の既知伝達署名 |

追加8失敗の原文分類は、missing `app.spec.yaml` 2件、非TTYの裸verify 1件、
dangerous command拒否1件、L3 package不足1件、未登録computed 1件、未供給path
`.bench-product-stdout.md` / `core.yaml`の発明各1件である。境界を通す補修はしない。

## 6. 裁定停止

qwen3.8 mediumはE36でfull 66.7%、p50 61.5秒となった。Aとの差分CIが広く0を含み、
Eの既定採用は自動決定しない。状態を`owner_adjudication_pending`として停止する。
