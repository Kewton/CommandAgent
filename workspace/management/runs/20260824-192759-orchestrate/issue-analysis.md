# Issue Analysis

## Issue #370: GUI Trial: 実行指示・実行状況・履歴・結果詳細を別ページに分離する

- 種別: `unknown`
- 目的: Trial の実行指示、実行中の監視、実行履歴一覧、実行結果詳細を別ページに分離し、ユーザーが現在の目的と状態を見失わない情報設計にする。
- 詳細化要否: `no`

### 受入条件

- 4ルートがそれぞれ独立したページタイトル、見出し、ナビゲーション状態を持つ。
- `/try/` に履歴一覧を常設せず、新規指示と Gate 1 の操作に集中できる。
- status は対象セッションの read-only な進行状況を表示し、再読み込み・別タブから再接続できる。
- history は実行日時、ID、状態、profile、intent、pack 等の要約に限定し、失敗診断を行内展開しない。
- detail は terminal verdict、失敗診断、acceptance、events・成果物への既存導線を集約する。
- 起動、実行中履歴、terminal 履歴、runtime badge、旧 deep link から正しいページへ遷移する。
- #100 の自動更新・鮮度表示・認証状態・最後に成功した一覧の保持を新しい history ページで満たす。
- Trial token は既存どおり base path ごとの `sessionStorage` を使い、URL・ログ・`localStorage` に含めない。
- Gate 1 の hash／明示確認、active lease、honest-failure、verification／acceptance、既存イベント名・スキーマを変更しない。
- `/` と `/proxy/commandagent/` の direct reload、デスクトップ／モバイル、主要状態を smoke で検証する。
- GUI の型検査・lint・build、関連 Rust テスト、read-only guard を更新して通す。
- Apply approved decision: Merge-recovery only in the existing feature/issue-370-gui-trial worktree; merge exact origin/develop 7abad1484fa29051a692f0b452b8158bb68808e2 with a normal merge commit; preserve Issue 369 deterministic completion-boundary assertions and Issue 370 four-route behavior; do not push, mutate PRs or Issues, use CommandMate, or alter historical evidence.

### 承認済み判断

- Merge-recovery only in the existing feature/issue-370-gui-trial worktree; merge exact origin/develop 7abad1484fa29051a692f0b452b8158bb68808e2 with a normal merge commit; preserve Issue 369 deterministic completion-boundary assertions and Issue 370 four-route behavior; do not push, mutate PRs or Issues, use CommandMate, or alter historical evidence.

### 推定影響ファイル

- gui/components/trial-run.tsx
- gui/components/trial-session-index.tsx
- CHANGELOG.md
- README.md
- docs/README.md
- docs/d3c-shell-design.md
- docs/dev/mechanism-ledger.md
- Cargo.toml

### 参考情報

- None

### テスト期待値

- None

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
