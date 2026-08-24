# 結果サマリ

## 事前宣言（実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run（各suite 12 run）。
- Go/No-Go: 一発通過（修復サイクル0でfull）≥60%、修復込みfull≥90%、所要p50≤180秒、1生成コスト≤¥10（=$0.067）。
- 予測帯: 直接priorなし。最近接の間接実測は ingest×Luna 100%（n=6）、Quiz 92%。点予測は置かず、実測にWilson CIを併記する。
- 予算上限: $5。超過見込み時は実行停止し、原記録を残す。
- executor: gpt-5.6-luna / OpenAI Responses/native。planner: ローカル既定。
- 環境中断はrun非消費として扱い、resumeで続行する。

実行開始前の宣言をここに封緘した。

## 実行結果

Go/No-Go判定は停止。warikan_001のみ完了し、warikan_002は環境中断、残り35runは未実施。実行runは、L2 spec生成・smoke・L3 promotion_decision生成まで進んだが、最終acceptanceで`community_schema_missing`となった。

| 閾値 | 実測 | Wilson 95% CI | 判定 |
|---|---:|---:|---|
| 一発full ≥60% | 0/1 = 0% | [0%, 79.3%] | 未達 |
| 修復込みfull ≥90% | 0/1 = 0% | [0%, 79.3%] | 未達 |
| 所要p50 ≤180s | 979s（完了run 1件） | — | 未達 |
| 1生成 ≤$0.067 | `null`（events/summaryにcost_usdなし） | — | 未達・cost配線床 |

所要p95は完了run集合が1件のため979s。失敗帰属は既存classes語彙の`acceptance`（primary reason: `community_schema_missing`）。新クラスは登録していない。費用は正本に記録がなく、推測値を補っていない。

### 生成物の実態

- L2率: 1/1（app.spec.yaml生成）。
- L3昇格率: 1/1（`src/app-zone/promotion_decision.json`実物を`evidence/warikan_001`へ保存）。これは生成器がL3へ進んだ実測であり、最終fullを意味しない。
- 代表app.spec.yaml: `evidence/warikan_001/app.spec.yaml`。
- 最終失敗原文: `ultra final acceptance failed after bounded repair: community_schema_missing`。

### バンド・score/time・台帳

- [band_summary_community.md](band_summary_community.md) にFull meaningラベル、種別行、五数要約を追加。
- [score_time_map_community.md](score_time_map_community.md) にcommunity点を追加。cost欠測を明示。
- [ledger.md](ledger.md) にCM-2bのGo/No-Go停止行を追加。

### 実行境界

`community_schema_missing`は、契約が要求するplatform-owned pinned schemaが新規workspaceへ注入されていないことを示す。これはCM-2bで発明的に補修せず、次の裁定対象として停止した。予算超過はない（記録costはnull）。
