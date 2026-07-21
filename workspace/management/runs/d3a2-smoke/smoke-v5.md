# D-3a-2e smoke v5

実失敗run `data9_ts_qwen35_profile_002` の実レイアウトを新規コピーし、
`.anvil/runs/*/events.jsonl` のfailed `run_stop`と
`.anvil/plans/recovery-*.yaml`を起点E-Bとして使用した。

再ビルド済みバイナリでの観測（開始epoch `1784638322`、終了epoch
`1784638332`、10秒後に観測終了）:

```json
{"entry":"create","event":"workflow_started",...}
{"checks":["E-A","E-B","E-C","E-D"],"edge":"create->investigate","event":"workflow_edge_fired"}
{"event":"workflow_node_started","intent":"investigate","node":"investigate"}
{"event":"workflow_node_run_created","node":"investigate","run_id":"019f84bb-b94f-7380-817e-ad80a23a219e","run_dir":".../.anvil/runs/019f84bb-b94f-7380-817e-ad80a23a219e"}
```

UUID run_idとroute順（create→investigate）を確認した。executorは継続実行
する設計だが、今回の証跡採取では10秒後に停止しており、investigate verdict・
fix辺・workflow_adjudicatedは未取得である。
