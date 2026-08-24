# Issue Analysis

## Issue #10: [ux] Modernize REPL input: slash-command completion, hints, multi-line input, Ctrl+C conventions

- 種別: `enhancement`
- 目的: インタラクティブモード(REPL)の入力体験を最先端のコーディングエージェントCLI(Claude Code / Codex CLI / Gemini CLI)水準に近づける第一歩として、**入力レイヤー**(補完・ヒント・複数行入力・Ctrl+C作法)を近代化する。
- 詳細化要否: `no`

### 受入条件

- 行頭 `/` に対しスラッシュコマンド14個すべてを補完候補に出す(前方一致。候補が1つなら確定)。
- コマンドに続くフラグ名(`--profile` / `--style` / `--prompt-layout`)を補完する。
- `--profile` の値はプロファイル定義(`src/planner/profile.rs` 系)を単一情報源として取得し、候補リストをハードコード複製しない。
- `/run-plan` `/run-ultra-plan` `/resume` の引数、および `$(cat ` の直後でファイルパス補完が効く(workspace_root 相対)。
- 補完候補の定義はスラッシュコマンド一覧(`render_help` / ディスパッチ)と単一情報源を共有し、コマンド追加時に補完だけ漏れる構造にしない(コンパイル時 or テストで同期を担保)。
- 入力中、履歴およびコマンド一覧からの前方一致サジェストを薄色(dim)で表示し、Right/End で受け入れられる(rustyline `Hinter`)。
- `NO_COLOR` 設定時はヒント表示に色を使わない(`src/tui/terminal.rs:19-21` の判定を再利用)。
- rustyline `Validator` により、末尾 `\` またはダブルクォート未閉の行は継続入力(2行目以降は `... ` 等の継続プロンプト)。
- bracketed paste を有効化し、改行を含むテキストの貼り付けが1回の入力として扱われる(貼り付け途中で意図せず送信されない)。
- 複数行入力の結果は既存の `parse_words` / `handle_command` にそのまま渡せる1つの文字列に正規化される。
- 入力中の行が空でない場合: Ctrl+C は行をクリアして新しいプロンプトを表示(終了しない)。
- 行が空の場合: 1回目で `press Ctrl+C again to exit` 相当のメッセージを表示、**連続2回目**で正常終了(履歴保存を含む通常の終了経路を通ること)。間に他のキー入力があればカウントはリセット。
- Ctrl+D(EOF)の即時終了は現状維持。
- コマンド実行中の Esc/Ctrl+C 割り込みセマンティクス(`src/tui/interrupt.rs`)には影響を与えない。
- stdin が TTY でない場合の bail(`src/tui/repl.rs:11-13`)は現状維持。
- 非UTF-8ロケール(`LC_ALL`/`LANG` 判定は `src/tui/terminal.rs:23-30`)でも文字化けしない。

### 推定影響ファイル

- Cargo.toml
- src/planner/profile.rs
- src/tui/interrupt.rs
- src/tui/editor.rs
- docs/dev-guardrails.md
- src/tui/repl.rs
- tests/tui_pty.rs
- src/tui/slash.rs
- src/tui/mod.rs
- src/tui/terminal.rs
- src/tui
- src/eval_events.rs

### 参考情報

- None

### テスト期待値

- cargo test
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
