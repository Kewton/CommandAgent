# 結果サマリ

## 事前宣言

- 分母: 3suite × 3変種 × 4run = 36run。
- 閾値: 一発full≥60%、修復込みfull≥90%、p50≤180秒、1生成≤$0.067。
- 予測: 直接priorなし。間接実測はingest×Luna 100% (n=6)、Quiz 92%。点予測なし、Wilson CI併記。
- 予算: $5上限。超過見込みで停止。
- 計器pin: `commandagent` release binary SHAはbench preflightで固定する。

この宣言を実行前に固定した。

## 実測と停止

CM-2bのwarikan_001実測を設計床窓として分母から除外した。CM-2cでは、schema供給とcost配線の修正を先に実装し、旧eventsを原文解剖した。

- 旧warikan_001: 979秒、最終`community_schema_missing`、provider events 224237 tokens、cost導出 `$0.01628088`。
- schema供給: golden empty workspaceの空性検査後に、封緘v0 schema/pinを`schema/`へ注入。欠落・pin mismatchはfail closed。
- cost供給: `pricing.toml`のモデルIDを引用キーへ修正し、実eventsの`provider_total_tokens` / `provider_cached_input_tokens`から導出。
- 新36run matrix: **ライブ実行前に停止**。CI/acceptanceで修正の正本確認を先行させるためであり、未実行runを成功・失敗へ算入していない。

従ってCM-2cの36run判定値は未計測。次の裁定対象は、修正HEADでの新規matrix再開である。未計測をWilson分母へ混ぜていない。

## 証跡

- [schema fixture](../../bench/community/appspec-schema/README.md)
- [cost analysis](../cm2b-golden-001/cost-analysis.md)
- [cost derived JSON](../cm2b-golden-001/cost-analysis-warikan-001.json)
- [community band](../cm2b-golden-001/band_summary_community.md)
