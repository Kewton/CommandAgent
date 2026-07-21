# UAT Report

## Merge Gate

- Status: `blocked`
- Message: UAT cannot proceed until every PR passes CI

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #10: [ux] Modernize REPL input: slash-command completion, hints, multi-line input, Ctrl+C conventions

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `行頭 `/` に対しスラッシュコマンド14個すべてを補完候補に出す(前方一致。候補が1つなら確定)。` を確認できる画面または実機操作を行う。
- 期待結果: 行頭 `/` に対しスラッシュコマンド14個すべてを補完候補に出す(前方一致。候補が1つなら確定)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `コマンドに続くフラグ名(`--profile` / `--style` / `--prompt-layout`)を補完する。` を確認できる画面または実機操作を行う。
- 期待結果: コマンドに続くフラグ名(`--profile` / `--style` / `--prompt-layout`)を補完する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--profile` の値はプロファイル定義(`src/planner/profile.rs` 系)を単一情報源として取得し、候補リストをハードコード複製しない。` を確認できる画面または実機操作を行う。
- 期待結果: `--profile` の値はプロファイル定義(`src/planner/profile.rs` 系)を単一情報源として取得し、候補リストをハードコード複製しない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/run-plan` `/run-ultra-plan` `/resume` の引数、および `$(cat ` の直後でファイルパス補完が効く(workspace_root 相対)。` を確認できる画面または実機操作を行う。
- 期待結果: `/run-plan` `/run-ultra-plan` `/resume` の引数、および `$(cat ` の直後でファイルパス補完が効く(workspace_root 相対)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `補完候補の定義はスラッシュコマンド一覧(`render_help` / ディスパッチ)と単一情報源を共有し、コマンド追加時に補完だけ漏れる構造にしない(コンパイル時 or テストで同期を担保)。` を確認できる画面または実機操作を行う。
- 期待結果: 補完候補の定義はスラッシュコマンド一覧(`render_help` / ディスパッチ)と単一情報源を共有し、コマンド追加時に補完だけ漏れる構造にしない(コンパイル時 or テストで同期を担保)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `入力中、履歴およびコマンド一覧からの前方一致サジェストを薄色(dim)で表示し、Right/End で受け入れられる(rustyline `Hinter`)。` を確認できる画面または実機操作を行う。
- 期待結果: 入力中、履歴およびコマンド一覧からの前方一致サジェストを薄色(dim)で表示し、Right/End で受け入れられる(rustyline `Hinter`)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``NO_COLOR` 設定時はヒント表示に色を使わない(`src/tui/terminal.rs:19-21` の判定を再利用)。` を確認できる画面または実機操作を行う。
- 期待結果: `NO_COLOR` 設定時はヒント表示に色を使わない(`src/tui/terminal.rs:19-21` の判定を再利用)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `rustyline `Validator` により、末尾 `\` またはダブルクォート未閉の行は継続入力(2行目以降は `... ` 等の継続プロンプト)。` を確認できる画面または実機操作を行う。
- 期待結果: rustyline `Validator` により、末尾 `\` またはダブルクォート未閉の行は継続入力(2行目以降は `... ` 等の継続プロンプト)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `bracketed paste を有効化し、改行を含むテキストの貼り付けが1回の入力として扱われる(貼り付け途中で意図せず送信されない)。` を確認できる画面または実機操作を行う。
- 期待結果: bracketed paste を有効化し、改行を含むテキストの貼り付けが1回の入力として扱われる(貼り付け途中で意図せず送信されない)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `複数行入力の結果は既存の `parse_words` / `handle_command` にそのまま渡せる1つの文字列に正規化される。` を確認できる画面または実機操作を行う。
- 期待結果: 複数行入力の結果は既存の `parse_words` / `handle_command` にそのまま渡せる1つの文字列に正規化される。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `入力中の行が空でない場合: Ctrl+C は行をクリアして新しいプロンプトを表示(終了しない)。` を確認できる画面または実機操作を行う。
- 期待結果: 入力中の行が空でない場合: Ctrl+C は行をクリアして新しいプロンプトを表示(終了しない)。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `行が空の場合: 1回目で `press Ctrl+C again to exit` 相当のメッセージを表示、**連続2回目**で正常終了(履歴保存を含む通常の終了経路を通ること)。間に他のキー入力があればカウントはリセット。` を確認できる画面または実機操作を行う。
- 期待結果: 行が空の場合: 1回目で `press Ctrl+C again to exit` 相当のメッセージを表示、**連続2回目**で正常終了(履歴保存を含む通常の終了経路を通ること)。間に他のキー入力があればカウントはリセット。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ctrl+D(EOF)の即時終了は現状維持。` を確認できる画面または実機操作を行う。
- 期待結果: Ctrl+D(EOF)の即時終了は現状維持。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `コマンド実行中の Esc/Ctrl+C 割り込みセマンティクス(`src/tui/interrupt.rs`)には影響を与えない。` を確認できる画面または実機操作を行う。
- 期待結果: コマンド実行中の Esc/Ctrl+C 割り込みセマンティクス(`src/tui/interrupt.rs`)には影響を与えない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `stdin が TTY でない場合の bail(`src/tui/repl.rs:11-13`)は現状維持。` を確認できる画面または実機操作を行う。
- 期待結果: stdin が TTY でない場合の bail(`src/tui/repl.rs:11-13`)は現状維持。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 16

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `非UTF-8ロケール(`LC_ALL`/`LANG` 判定は `src/tui/terminal.rs:23-30`)でも文字化けしない。` を確認できる画面または実機操作を行う。
- 期待結果: 非UTF-8ロケール(`LC_ALL`/`LANG` 判定は `src/tui/terminal.rs:23-30`)でも文字化けしない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
