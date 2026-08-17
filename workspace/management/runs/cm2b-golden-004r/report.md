# 結果サマリ

## 事前宣言・除外窓

分母は3 suite × 3変種 × 4 run = 36 run。閾値は一発full ≥60%、修復込みfull ≥90%、所要p50 ≤180秒、1生成 ≤$0.067、予算上限$5。Wilson 95% CIを併記し、直接priorは置かない（最近接間接実測: ingest×Luna 100%, n=6 / Quiz 92%）。計器pinは新配置campaign `bin/commandagent`、SHA-256 `3a60dd196caf458354e404cd379699024d4043f884370dfa2c87889eeecf8f78`。

| 除外窓 | 理由 |
|---|---|
| CM-2b-001/002 warikan_001/002 | schema供給・cost配線床返済前 |
| CM-2b-003 warikan_001〜003 | spec字義・verify自己完結化前 |
| CM-2b-004 warikan全窓 | bench install sandbox floor返済前 |

sealed suite SHAはwarikan `215aba…d9e0c`、mochimono `1246b8…a9a6`、vote `da5166…d08df`、manifest `4ea74f…dad86`で不変。

## 実測と停止

新既定配置へのinstallと、支出前のbuilt/installed SHA一致照合は成功した。warikan 12/12を起動したが、全件 `provider request failed ... Ollama request failed ... localhost:11434/api/chat`、product_exit=1、duration=0秒、provider token=0で終了した。これは同一環境停止クラスの系統的連続のため、規定どおりmochimono/voteの24 runを開始せず停止。36分母へ未実施runを算入しない。

| 指標 | 実測 | Wilson 95% CI | 判定 |
|---|---:|---:|---|
| 一発full | 0/12 = 0% | [0%, 24.3%] | 停止 |
| 修復込みfull | 0/12 = 0% | [0%, 24.3%] | 停止 |
| 所要p50/p95 | 0s / 0s（provider起動前） | — | 性能判定対象外 |
| 1生成cost_usd | null（provider未到達） | — | 床として記録 |

計測campaignの原文証跡は`uat-meta-warikan.json`に保存した。costは算出可能なprovider usageがなくnullであり、推測値を補っていない。

## 帰属・判定

失敗帰属は既存の環境/provider停止語彙。新規環境クラス`bench_install_sandbox_denied`はCM-2fで登録済みで、今回の配置床では発火していない。L2/L3生成物はprovider計画前停止のため観測なし。Phase 2 GOは宣言しない。

## CI・前回結論

CM-2e最終commit `7f0c4be3`: CI `32039236122` success、acceptance `32039236123` success。CM-2f配置commit `a9a73510`: CI `32040061429` failure、acceptance `32040061380` failure（いずれもGitHub action取得429/503、コード実行前）。絶対パスbinary修正commit `5e9c6026`: CI `32040265398` failure、acceptance `32040265416` failure（同じ外部action取得429/503）。失敗原文はworkflowログに保存されており、製品テスト失敗ではない。
