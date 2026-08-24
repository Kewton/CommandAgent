# 層2検証部品カタログ被覆（E本体入口）

## 列挙方法

```bash
rg -n 'pipeline_probe|claims.binding|claims_binding|reconciliation|results-schema|rerun|investigation-binding|verify_origin|evidence' src workspace/management/scripts
find workspace/management/runs -type f \( -name 'investigation-binding.json' -o -name 'claims-binding.json' -o -name '*regression*.json' \)
```

上記の実在出力と実装参照を突合した。見つからない部品は「記録なし」とし、推測で補完しない。

| id | 検証すること | profile依存 | 入力要件 |
|---|---|---|---|
| `pipeline_probe` | パイプラインを指定コマンドで実行できること | data | command、exit、stdout/stderr |
| `data_results_schema` | results.jsonが契約スキーマに適合すること | data | results.json、schema |
| `data_reconciliation` | 入力行・使用行・除外行の整合性 | data | reconciliation.json、入力件数 |
| `data_claims_binding` | レポート主張がresults実値へ束縛されること | data | claims-binding.json、results |
| `data_rerun_consistency` | 再実行結果が基準結果と一致すること | data | baseline/rerun results |
| `investigation-binding` | I2主張がI1出力に実在すること | investigate | investigation-binding.json、I1出力 |
| `fix before/after` | 修正前失敗・修正後成功を確認すること | fix | fix evidence before/after |
| `fix regression-*` | 回帰チェック群が成立すること | fix | regression evidence、check id |
| `verify_origin` | 起点束縛検証集合を再実行すること | workflow | origin evidence、check lineage |
| `E-A..E-D` | verdict/evidence/epoch/carryの辺条件 | workflow | workflow-circle.json |

## 第3profile候補の被覆試算

### (a) REST APIサーバー

完成の定義を「代表エンドポイントが規定応答schemaを返し、エラー応答も契約どおり」と仮置きする。✅ exit・schema照合は既存、🟡 HTTP probe・fixture・認証境界の新部品（Rust小規模）が必要、🔴認証・副作用の機械照合面は未確定。dataの入出力契約をネットワーク境界へ拡張できるため、部品だけで立つかを判定しやすい。

### (b) CLIツール

完成の定義を「入力契約、exit code、--helpが同一リリースで整合」と仮置きする。✅ command/exitと主張束縛、🟡 help snapshotとargv probeが必要、🔴対話TTYの完全照合は未知。既存probeを最小拡張でき、E-3の初手候補として比較優位がある。

### (c) ETL／ファイル変換

完成の定義を「入力と出力の全件対照合、変換失敗の分類、再実行一貫性」と仮置きする。✅ reconciliation・claims・rerun、🟡形式別diffと大容量ストリーム検証が必要、🔴意味保存の一般定義は未知。data兄弟として既存セルの再利用率が高い。

### (d) 静的サイト／ドキュメント生成

完成の定義を「ビルド成功、リンク整合、生成物構造が契約どおり」と仮置きする。✅ exit・artifact台帳、🟡link checkerと構造schemaが必要、🔴視覚的品質の機械照合は未知。nextjs系との差分を部品単位で測れる。

### (e) 技術調査レポート

完成の定義を「各主張が実在出典へ引用束縛され、反証可能な結論を持つ」と仮置きする。✅ I2の引用×出力実在、🟡出典取得・版固定・重複引用の部品が必要、🔴出典の十分性・結論妥当性の機械照合自体が未知。E-3の本丸であり、既存I2だけで立つかを検証する価値が最大。

候補選定と第3profileの確定はレビュー側の意思決定である。この文書は材料と分母を揃えるためのもので、結論を自動化しない。
