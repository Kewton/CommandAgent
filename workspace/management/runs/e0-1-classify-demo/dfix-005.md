# Failure class classification

| run | class id | attribution | stop pattern |
|---|---|---|---|
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_pipe_gemma31_001/.anvil/runs/019f74be-b352-7da3-a53e-c3658d9a0c9f` | model_stagnation_read_only | model | model_stagnation:read_only_loop |
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_pipe_qwen35_001/.anvil/runs/019f74b6-6018-7a33-8efa-8a76f357c816` | model_stagnation_read_only | model | model_stagnation:read_only_loop |
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_pipe_qwen35_001/.anvil/runs/019f74bd-8b6e-7740-94b0-5a5384c2a4ed` | model_stagnation_read_only | model | model_stagnation:read_only_loop |
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_pipe_qwen35_002/.anvil/runs/019f74c1-6733-7243-a8d0-632c6a8be54c` | model_stagnation_read_only | model | model_stagnation:read_only_loop |
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_schema_gemma31_001/.anvil/runs/019f74c2-5453-7880-9edd-04d4415bf04a` | process_failure | machine | failure_kind:process_failure |
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_schema_qwen35_001/.anvil/runs/019f74c1-ddcd-7c01-9301-1daa4b302426` | process_failure | machine | failure_kind:process_failure |
| `workspace/management/runs/uat-test0718-dfix-005/artifacts/dfix5_schema_qwen35_002/.anvil/runs/019f74c4-9161-7241-8fb9-6a956c4d5299` | model_stagnation_read_only | model | model_stagnation:read_only_loop |

## UNKNOWN runs

- なし

## 終端形と死因帰属の検算

dfix-005 v2レポートの「6/6 read_only停滞」は死因（モデル）を示し、一次資料の`run_stop.failure_kind=process_failure`はプロセス終端の形を示す。各schema runの`stop_reason`にも`model_stagnation`が含まれ、停滞→フェーズ失敗→process_failure終端という層の重なりである。したがってレポートの6/6記述は死因としては精密であり、終端形を表す語ではない。訂正は不要だが、今後は両フィールドを併記する。
