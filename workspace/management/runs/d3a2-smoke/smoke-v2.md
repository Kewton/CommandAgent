# D-3a-2c workflow smoke v2

起点は `workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002` を選定した。確認コマンドは `rg -l '"event":"run_stop"' workspace/management/runs/uat-test0716-data-009/artifacts/data9_ts_qwen35_profile_002/.anvil/runs/*/events.jsonl` と `find .../.anvil/plans -name 'recovery-*.yaml'` で、run_stop (failed) と recovery YAML の実在を確認した。

コピー先で `commandagent --workflow workflows/recovery-circle-data.yaml --origin /Users/maenokota/share/work/localwork/commandagent_mvp/01/d3a2_smoke2_origin` を実行した。証跡に残った原文は `workflow_started` のみで、実行環境のセッション終了により investigate ノードの `intent_resolved` / `run_id` には到達しなかった。したがって本試行では辺発火・fix起動・workflow_adjudicated は未観測であり、判定は行わず環境中断として記録する。
