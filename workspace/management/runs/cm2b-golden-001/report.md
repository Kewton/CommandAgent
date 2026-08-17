# 結果サマリ

## 事前宣言（実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run（各suite 12 run）。
- Go/No-Go: 一発通過（修復サイクル0でfull）≥60%、修復込みfull≥90%、所要p50≤180秒、1生成コスト≤¥10（=$0.067）。
- 予測帯: 直接priorなし。最近接の間接実測は ingest×Luna 100%（n=6）、Quiz 92%。点予測は置かず、実測にWilson CIを併記する。
- 予算上限: $5。超過見込み時は実行停止し、原記録を残す。
- executor: gpt-5.6-luna / OpenAI Responses/native。planner: ローカル既定。
- 環境中断はrun非消費として扱い、resumeで続行する。

実行開始前の宣言をここに封緘した。以下は実行後に追記する。

## 実行結果

（36run完了後に追記）
