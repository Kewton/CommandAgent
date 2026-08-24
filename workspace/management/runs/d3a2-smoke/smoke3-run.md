# D-3a-2c smoke v3 — 人手逐次実行手順

起点は `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002`。以下の検索で `run_stop` と recovery YAML の実在を確認済み:

```sh
rg -l '"event":"run_stop"' workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002/.anvil/runs/*/events.jsonl
find workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002/.anvil/plans -name 'recovery-*.yaml' -print
```

フェーズ2は通常のターミナルで、次を逐次実行する（コマンドは改変しない）。

```sh
date +%s
commandagent --workflow workflows/recovery-circle-data.yaml \
  --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke3_origin
date +%s
```

実行前後のepoch秒を保存する。期待観測は
`workflow_started` → investigateノード起動（`intent_resolved`、run_id、
`investigation_plan_synthesized`）→辺確認→終端
（`workflow_adjudicated`）である。途中打切りせず完走まで放置する。
