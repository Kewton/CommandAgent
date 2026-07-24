# E-2b段階1 互換証明記録

investigateの構成を`intents/investigate.yaml`から読み、合成実体は既存Rustを呼ぶ最小統合を実施した。

- schema parser unit: 1 passed
- investigate phase order: 3（reproduce-candidate / diagnose / bind-verify）
- investigate conformance: 既存テスト群を変更せず維持
- corpus: 既存fixtureを変更せず維持
- byte snapshot差分: 実測対象は既存snapshot全体の権限付きfull suiteで確認予定。差分が出た場合はここで停止し、差分原文をレビューへ提出する。

## E-2b段階1実行結果（2026-07-24）

- schema unit: 1 passed
- investigate構成: 3 phases、既存イベント発火コードは変更なし
- investigation conformance 7本: 実行対象は既存`tests/investigation_intent_conformance.rs`群、変更なし
- full suite: **非green**。環境／外部probe系を中心に22 failed、その他はpassed、ignoredは個別テスト出力の環境依存。失敗名は`/tmp/e2b-cargo.txt`の原文に保存（browser_probe、Ollama接続、dev-server、interaction probe等）。

受理条件の「権限付きfull green」は未達のため、互換証明済みとは宣言しない。schema以外の実体差分は確認しておらず、修正せず停止・報告する。

今回の変更は構成の読み込みと固定語彙検証だけで、合成計画・材料注入・照合・裁定のRust実体は変更していない。
