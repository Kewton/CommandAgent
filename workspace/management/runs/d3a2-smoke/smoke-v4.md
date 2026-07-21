# D-3a-2d smoke v4

実失敗run `data9_ts_qwen35_profile_002` のartifactsを新規コピーし、
`.anvil/runs/*/events.jsonl` のfailed `run_stop` と
`.anvil/plans/recovery-*.yaml` を起点E-Bとして検証した。ルート直下の
発明形式は使用していない。

実走開始epochは `1784636732`。実レイアウト起点確認後、監視証跡に
`workflow_started`、`workflow_node_started {node: fix, intent: fix}`、
`workflow_node_run_created {run_id: fix, run_dir: evidence/fix-events.jsonl}`
が記録された。ノードexecutorは継続中で、完走・workflow_adjudicated・
investigate run_idは未取得である。

観測された実装課題: YAMLのnodesをBTreeMap順に走査しているため、契約の
create→investigate→fix順ではなくfixが先に起動した。これはroute順逐次実行
契約に反するため、レビュー裁定対象として記録する。
