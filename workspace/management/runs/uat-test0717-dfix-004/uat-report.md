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

## v4: 完全一致コマンド再発行 — preflightで停止

レビュー裁定を転記する: **v3の6試行はcommandagent起動ゼロのためrun消費に数えない。** v4では独自のtimeout・env・niceその他wrapperを一切追加せず、指定された完全一致コマンドだけを各1回起動する方針を事前固定した。

しかし、全green必須のpreflightで `cargo test` が終了コード101となった。HEADは `2d357047573ac2967a1acbf44db649861fbe2d13`（`2d35704` 以降の条件を満たす）で、`git fetch origin develop` 後も `HEAD == origin/develop == FETCH_HEAD` だった。作業ツリーに追跡済み・未追跡ファイルはなくstash対象なし、ホスト `NODE_ENV=production` も記録した。

full suiteコマンドはlibrary suiteの1432 pass / 0 fail / 15 ignoredを通過後、`tests/generality_guardrails.rs` の `runner_chokepoints_do_not_grow_past_interim_budget` で停止した。実行済み範囲の失敗はこれ1件である。原文は次のとおりで、focused再確認も同じ終了コード101だった。

```text
src/planner/fix_runtime/data_isolate.rs production code grew to 160 lines; production code baseline is 149, allowed max is 152. Move new subsystems to new modules or land a shrinking refactor first.
```

本タスクでは `src/`、`tests/`、`docs/` の変更が禁止され、guardrail baseline引上げも禁止されている。従ってpreflightを偽ってgreenにせず、release build/install、出発点調達、6本の `commandagent` 起動を行わなかった。新規計測workspaceも作成していない。v4の製品run消費は0、再試行も0である。証跡は `artifacts-v4/preflight.log` に保存した。

### v4判定

| 項目 | 結果 | 根拠 |
|---|---|---|
| preflight | failed | full `cargo test` exit 101、focused確認もexit 101 |
| P0-a 6/6正直終端 | 未判定 | v4 run起動ゼロ |
| P0-b 契約§4準拠 | 未判定 | F evidence未生成 |
| P0-c 偽成功ゼロ | 未判定 | v4 run起動ゼロ |
| P1-a FIX-8/9 4クラス再発ゼロ | 未判定 | events/run成果物未生成 |
| full率・族/executor差・残存クラス | 未集計 | v4 6 runが存在せず24run母集団を構成できない |

イベント発火表、F系evidence表、FIX-8/9の所有権重複lint・空verify scaffold error・不在成果物要求・UTF-8 phase死の再帰監査、および#1〜#4合算分布は、対象となるv4 `.anvil/runs/*/events.jsonl` と `evidence/fix-*.json` が存在しないため実施不能である。検索結果やevidenceを捏造してゼロ件とは判定しない。

### D-2コスト会計

| 発行 | 調達 | 製品実行 | レポート | 空振り要因 |
|---|---:|---:|---:|---|
| v1 | 0 | 0 | あり | 指定workspace不在 |
| v2 | 採取元探索のみ | 0 | あり | schema採取元不在 |
| v3 | 4セット調達完了 | 0 | あり | 存在しないtimeout wrapperで6件exit 127 |
| v4 | 0 | 0 | あり | full suite guardrail failureでpreflight停止 |

v4の実時計測は、preflight・上流確認・focused再確認が259秒（12:08:50〜12:13:09 JST）、調達0秒、製品実行0秒、初稿作成が84秒（12:13:09〜12:14:33 JST）だった。v1〜v3の調達・preflight・報告作成、およびv3の6回のハーネス空振りはD-2の空振りコストとして残る。ただしレビュー裁定どおりv3は製品run消費0であり、v4も製品run消費0である。D-2クローズ判定には使用しない。
