# Smoke v3 hang diagnosis

対象PID: `7498`

## 採取結果

コマンド:

```sh
ps -o pid,stat,etime,command -p 7498
sample 7498 5 -file /tmp/smoke3-sample.txt
ollama ps
find /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke3_origin_retry/.anvil/runs -name events.jsonl -newer /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke3_origin_retry/evidence/workflow-events.jsonl -print
```

結果:

- `ps`: PID 7498は存在せず、実行終了済み。
- `sample`: `sample cannot examine process 7498 ... because it no longer appears to be running.`
- `ollama ps`: 実行時点でモデル一覧は空（ヘッダのみ）。
- newer events: 該当ファイルなし。起点`.anvil/runs`に新規ノードイベントの書き進みは確認できなかった。
- 監視ログの最終epoch: `1784636060`。最終観測イベントは`workflow_started`のみ。

## 判定

PID終了後の採取となったためstack sampleは得られなかった。観測可能な証跡上はノードrun作成・イベント進行なしで、`workflow_started`後に停滞し、最終的にSIGINT（終了コード130）で終了した状態である。
