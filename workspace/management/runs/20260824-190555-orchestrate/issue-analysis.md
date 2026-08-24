# Issue Analysis

## Issue #375: Trial Events: StepPlanタスクの開始・完了・失敗を後方互換イベントとして記録する

- 種別: `unknown`
- 目的: UltraPlanの各フェーズ内で実行される `StepPlan.steps` について、タスク単位の開始・正常完了・省略・失敗を機械的に再構成できる後方互換イベント契約を追加する。
- 詳細化要否: `no`

### 受入条件

- 実行された全Stepに一意に対応するstarted eventと、正常経路では1件だけのterminal eventが記録される。
- 通常完了、既存成果によるshort circuit、検証失敗、bounded repair後の失敗、中断を区別できる。
- 各Stepを実行区間、Plan、phaseへ曖昧なく関連付けられる。
- 同一セッションの初回実行と追加依頼、再試行、同一Step IDを誤って統合しない。
- terminal eventから完了数、失敗Step、変更パス、検証結果を推測なしで集約できる。
- 既存イベントconsumerと既存セッションの読み取りが壊れない。
- イベントサイズと機密情報の境界をテストする。
- 関連する `tests/corpus/apps/` fixtureとevent schemaテストを追加・更新する。
- honest-failure、verification、acceptance、release gateを弱めない。
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`が通る。

### 承認済み判断

- None

### 推定影響ファイル

- src/planner/step_plan.rs
- src/planner/runner/phase/step_plan_execution.rs
- summary.md
- src/planner/runner.rs
- src/minimal_loop/loop_run.rs
- phase/step_plan_execution.rs
- src/eval_events
- tests/corpus/apps
- CHANGELOG.md

### 参考情報

- None

### テスト期待値

- cargo test
- cargo clippy
- cargo fmt

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

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

### 承認済み判断

- None

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

## Issue #374: GUI Trial: セッションの作業ディレクトリを表示してコピー可能にする

- 種別: `unknown`
- 目的: GUI Trial の実行中・実行結果詳細で、生成コードや実行対象が置かれたセッション作業ディレクトリを明示し、利用者がそのパスをそのままコピーして動作確認できるようにする。
- 詳細化要否: `no`

### 受入条件

- Trial起動直後、実行中、terminal、履歴から開いた結果詳細で同じ作業ディレクトリを確認できる。
- 表示された絶対パスが委譲CLIの `current_dir` / `--cwd` と一致する。
- コピーボタンでパスをクリップボードへ保存でき、キーボード操作と読み上げ通知が機能する。
- 作業ディレクトリと `events.jsonl` / `summary.md` の保存先が明確に区別される。
- 作業ディレクトリが削除済みの場合、成果物が存在するように見せず明示的な状態を表示する。
- 絶対パスは認証済みの専用セッションAPI以外から取得できない。
- 不正session id、path traversal、symlink、execution root外への参照を拒否するテストがある。
- 既存のpublic projection、Trial token、origin検証、read-only guardを弱めない。
- `/` と `/proxy/commandagent/`、デスクトップ／モバイルで表示とコピーをsmoke検証する。
- Rust関連テストとGUIのtypecheck、lint、buildが通る。

### 承認済み判断

- None

### 推定影響ファイル

- summary.md
- src/bin/gui_server/session_paths.rs
- delegate.rs
- tests/gui_server.rs
- CHANGELOG.md
- README.md
- docs/README.md
- docs/d3c-shell-design.md

### 参考情報

- None

### テスト期待値

- None

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #376: GUI Trial: Planのフェーズ配下にタスク単位の進捗・実行結果を表示する

- 種別: `unknown`
- 目的: GUI Trial の実行状況と実行結果詳細で、UltraPlanのフェーズだけでなく、各フェーズ内の `StepPlan.steps` をタスクとして表示し、それぞれの進捗・検証・実行結果を確認できるようにする。
- 詳細化要否: `no`

### 受入条件

- 実行中に現在のフェーズと現在のタスク、タスク番号／総数を確認できる。
- terminal結果でPlanに含まれる全タスクのterminal状態を確認できる。
- completed、short-circuited、failed、interruptedを視覚・テキストの両方で区別できる。
- FAILEDタスクが自動展開され、失敗理由と関連証跡への導線が表示される。
- 初回実行と追加依頼が別の実行区間として表示され、同一Step IDを誤統合しない。
- フェーズ／タスクの状態集計が #375 のtyped eventだけに基づき、イベント順から成功を推測しない。
- 旧セッションでは明示的なunsupported表示となり、不正確な成功件数を表示しない。
- history一覧はコンパクトさを維持し、詳細は #370 の結果詳細ページへ移す。
- 100タスク程度でもpolling payloadと描画が過度に増大しない。
- status／detailのdirect reload、再接続、`/` と `/proxy/commandagent/`をsmoke検証する。
- キーボード、見出し階層、aria-expanded、状態の非色依存表示を検証する。
- GUI typecheck、lint、build、関連Rustテスト、read-only guardが通る。

### 承認済み判断

- None

### 推定影響ファイル

- gui/components/trial-gate-two.tsx
- CHANGELOG.md
- README.md
- docs/README.md
- docs/d3c-shell-design.md
- docs/dev/mechanism-ledger.md
- docs/dev/e5f-phase-state-machine.md
- docs/dev/generality.md

### 参考情報

- None

### テスト期待値

- None

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。

## Issue #377: GUI Trial: FAILED原因と具体的なリカバリー手順を構造化して表示する

- 種別: `unknown`
- 目的: GUI TrialがFAILEDになったとき、「どこで・何が・なぜ失敗したか」「どこまで完了したか」「次に何をすべきか」を構造化して表示し、既に記録されているRecovery Plan、修復プロンプト、推奨コマンドへ直接辿れるようにする。
- 詳細化要否: `no`

### 受入条件

- bounded repair後のverification failureで、失敗フェーズ／タスク、失敗検証、primary reason、repair回数を確認できる。
- `recovery_prompt_saved` のrepair prompt、Recovery Plan、2種類の推奨コマンドが表示・コピーできる。
- `fix_command_failure` だけで終わらず、利用者が次に行う具体的操作を確認できる。
- completion contractのworkspace境界違反等を、単なる一般コマンド失敗ではなく発生場所と根拠付きで表示する。
- 成果物の有無を `Changed files` の件数だけから判断せず、#374の作業ディレクトリ状態と区別する。
- 初回FAILED後に継続実行が成功したセッションで、最終区間と過去区間の診断を混在させない。
- release gate failure、probe failure、実行中断、spawn／preflight failureを区別する。
- 原因を構造化できない旧セッションは推測せず、明示的なfallbackと `summary.md` / `events.jsonl` 導線を出す。
- raw codeと技術詳細を保持しつつ、通常表示は途中で切れたコマンド文字列にならない。
- Recovery操作は自動実行せず、Gate 1、directive confirmation、honest-failureを維持する。
- 失敗fixture、継続実行fixture、legacy fixtureをRustとGUI smokeへ追加する。
- `/` と `/proxy/commandagent/`、デスクトップ／モバイル、keyboard／screen readerを検証する。
- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`、GUI typecheck／lint／buildが通る。
- Apply approved decision: Propagate exact finalized dependency heads 375=1acfc81aa0ba7d7f338db4013d94df95e0d7d779, 370=9e8e178b97b49c78411ad9d2ba1783168227cdd9, 374=839f9c335a4af780a72433130e452ad984b87c3e, 376=810dd041a20cf73b03c1979cc66015d26cf65a6e into the existing feature/issue-377-gui-trial-failed worktree; preserve Issue 377 behavior; verify and commit only; do not push, mutate PRs or Issues, dispatch workers, or start/stop CommandMate.

### 承認済み判断

- Propagate exact finalized dependency heads 375=1acfc81aa0ba7d7f338db4013d94df95e0d7d779, 370=9e8e178b97b49c78411ad9d2ba1783168227cdd9, 374=839f9c335a4af780a72433130e452ad984b87c3e, 376=810dd041a20cf73b03c1979cc66015d26cf65a6e into the existing feature/issue-377-gui-trial-failed worktree; preserve Issue 377 behavior; verify and commit only; do not push, mutate PRs or Issues, dispatch workers, or start/stop CommandMate.

### 推定影響ファイル

- src/bin/gui_server/session_diagnostics.rs
- summary.md
- CHANGELOG.md
- README.md
- docs/README.md
- docs/d3c-shell-design.md
- docs/dev/mechanism-ledger.md
- Cargo.toml

### 参考情報

- None

### テスト期待値

- cargo test
- cargo clippy
- cargo fmt

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
