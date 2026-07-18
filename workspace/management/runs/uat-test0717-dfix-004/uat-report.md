# UAT dfix-004 — blocked before execution

基準HEAD: `64e2616`。権限付き環境で `cargo test -q` full suite は成功した（browser probeを含む）。

しかし、指定された計測ワークスペース `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_dfix_004/` が存在せず、採取済みの出発点・provenance・artifactsも確認できなかった。合成禁止かつ各run最大1回の規律のため、出発点を推測・合成して実行していない。

従って本計測の6 run、イベント監査、F系evidence、FIX-8/9再発判定、#1〜#4合算更新は未実施。run再実行も行っていない。D-2クローズ判定には使用しない。

Preflight: cargo test PASS。調達・実行・レポート所要: 調達/実行 0（workspace不在で開始前停止）、本報告作成時間のみ。

## v2: 再発行による実施

再発行HEADは `64e2616` 以降（実際のHEAD `2066ef2`）。ホスト `NODE_ENV=production` を記録した。権限付き `cargo test -q` はfull suite greenだった。

ただし、指定された採取元のうちローカル `test0713_data_001` が存在せず、リポジトリ側 `uat-test0713-data-001` にも成果物artifactsがなく、schema/A・schema/Bの合成禁止調達を満たせなかった。pipe系の候補（`test0714_m4_001`、`test0715_data_005`）は存在するが、4セットが揃わないため、各run最大1回の実行規律に従い6本とも開始していない。

従ってevents.jsonlの再帰監査、F系evidence、FIX-8/9再発判定、24run合算更新は未実施。実行を捏造せず、D-2クローズ判定には使用しない。

## v3: 採取元をリポジトリ内スナップショットに変更して実施

HEAD `85a02c7` 以降で実施。指定された `uat-test0717-dfix-002/artifacts/source-checks/{pipe-a,pipe-b,schema-a,schema-b}` をコピーし、4セットのR事前確認と `sales.csv` SHA-256（全件 `2f6c04e42b0ebdff85a7eb6b52a342610155be6796bd89e5729075d87c78d873`）を確認した。権限付き `cargo test -q` と release build/install/version は成功し、ホスト `NODE_ENV=production` を記録した。

6本は各1回試行したが、実行ラッパーに `timeout` コマンドが存在せず、全件がcommand実行前に終了コード127となった。各runの `uat-console.log` に `run: command not found: timeout`、`outer-exit-code.txt` に127を保存している。再試行は行わない。

| run | 終端 | F1/F2/F3 | 帰属 |
|---|---|---|---|
| dfix4_pipe_qwen35_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_pipe_gemma31_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_pipe_qwen35_002 | failed(127) | not executed | 計測ハーネス |
| dfix4_schema_qwen35_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_schema_gemma31_001 | failed(127) | not executed | 計測ハーネス |
| dfix4_schema_qwen35_002 | failed(127) | not executed | 計測ハーネス |

従ってevents.jsonl、FIX-8/9再発監査、full/F evidenceは生成されず、P0/P1は未判定。調達は完了、実行はハーネス欠陥で停止、コスト詳細はv3 workspaceとartifacts-v3に保存した。D-2クローズ判定には使用しない。
