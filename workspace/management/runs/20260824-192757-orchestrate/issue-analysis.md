# Issue Analysis

## Issue #371: GUI Extensions: 拡張レイヤーを定義しレイヤー別カタログと登録導線を構築する

- 種別: `unknown`
- 目的: CommandAgent の拡張境界をレイヤーとして定義し、GUI の拡張画面で「何を、どこまで、安全に拡張できるか」と登録方法を一貫して示す。
- 詳細化要否: `no`

### 受入条件

- 拡張画面の最上位に4レイヤーと依存関係、できること／できないことが表示される。
- 各拡張項目に layer、source、status、hash、assurance、登録／昇格方法が一貫して表示される。
- Contract / Suite が拡張種別と誤認されない情報構造になる。
- extension root の設定状態と、未設定・不正・非互換・未 pin 等の利用不可理由が表示される。
- Layer 2 は draft profile の一覧と安全な登録Issueへの導線を持つ。
- Layer 3 は既存 pack catalog／作成ウィザード／Trial 選択導線を維持する。
- Layer 1 の能力語彙や Layer 4 の admission を GUI から任意追加・自己昇格できない。
- Gate 1 と acceptance に実際に有効な拡張と exact hash が引き続き投影される。
- `/` と `/proxy/commandagent/`、デスクトップ／モバイルで表示・導線を smoke 検証する。
- GUI ヘルプと EN/JA の拡張ドキュメントを同じレイヤー定義へ同期する。

### 承認済み判断

- None

### 推定影響ファイル

- README.md
- docs/guide/README.md
- docs/guide/en/extensions.md
- docs/user/gui-extensions.md
- docs/user/gui-operations.md
- CHANGELOG.md
- Cargo.toml
- docs/README.md

### 参考情報

- None

### テスト期待値

- None

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #372: GUI Extensions: 外部 draft profile を安全に登録する供給APIとウィザードを追加する

- 種別: `unknown`
- 目的: extension root に外部 draft profile の manifest / additive overlay を安全に登録・検証できる、境界付き供給 API と GUI ウィザードを追加する。
- 詳細化要否: `no`

### 受入条件

- extension root が設定済みの場合のみ、Layer 2 から profile 登録ウィザードを開始できる。
- compact manifest v2 と、許可された additive overlay を既存 parser/validator で検証する。
- profile id、供給先の正規化済みパス、exact hash、draft／未承認、assurance 上限 static を保存前に確認できる。
- path traversal、絶対パス、symlink 追従、サイズ超過、未知フィールド／capability、既存 built-in／外部 id 衝突、不正 overlay を fail-closed で拒否する。
- 既存ファイルを暗黙に上書きせず、同一内容は idempotent、競合内容は明示エラーにする。
- 一時ファイルへの書込み・同期・rename 等で atomicity を確保し、途中失敗で半端な manifest を残さない。
- 認証失敗、origin 不一致、検証失敗、競合、I/O 失敗を安定した error code と行動可能な日本語で返す。
- 保存・失敗を秘密情報なしで journal に記録し、catalog に source/status/hash/利用可否を投影する。
- 再起動が必要な場合は、保存成功と runtime 未反映を混同せず `restart_required` と手順を表示する。
- 登録済み profile は再起動後に Trial 候補へ draft として現れ、Gate 1 / acceptance に exact hash と static 上限が残る。
- auth/origin/body limit、path/symlink、atomicity、collision、idempotency、error mapping の Rust テストを追加する。
- `/` と `/proxy/commandagent/` の GUI smoke、型検査・lint・build、read-only/protection guard を通す。

### 承認済み判断

- None

### 推定影響ファイル

- <extension-root>/profiles/<id>/manifest.toml
- overlay.toml
- src/planner/runner.rs
- src/minimal_loop/loop_run.rs
- README.md
- docs/guide/README.md
- docs/guide/en/extensions.md
- docs/user/gui-extensions.md

### 参考情報

- None

### テスト期待値

- None

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
