# E-2b段階1 互換証明記録

investigateの構成を`intents/investigate.yaml`から読み、合成実体は既存Rustを呼ぶ最小統合を実施した。

- schema parser unit: 1 passed
- investigate phase order: 3（reproduce-candidate / diagnose / bind-verify）
- investigate conformance: 既存テスト群を変更せず維持
- corpus: 既存fixtureを変更せず維持
- byte snapshot差分: 実測対象は既存snapshot全体の権限付きfull suiteで確認予定。差分が出た場合はここで停止し、差分原文をレビューへ提出する。

今回の変更は構成の読み込みと固定語彙検証だけで、合成計画・材料注入・照合・裁定のRust実体は変更していない。
