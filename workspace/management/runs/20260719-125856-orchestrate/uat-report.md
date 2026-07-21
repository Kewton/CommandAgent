# UAT Report

## Merge Gate

- Status: `passed`
- Message: all 16 UAT scenarios passed with evidence

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #10: [ux] Modernize REPL input: slash-command completion, hints, multi-line input, Ctrl+C conventions

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `行頭 `/` に対しスラッシュコマンド14個すべてを補完候補に出す(前方一致。候補が1つなら確定)。` を確認できる画面または実機操作を行う。
- 期待結果: 行頭 `/` に対しスラッシュコマンド14個すべてを補完候補に出す(前方一致。候補が1つなら確定)。
- Actual: Typing slash at the start of an empty prompt exposed all 14 canonical slash commands; prefix completion also selected a unique command.
- Evidence: Latest build c14b387: editor test command_completion_uses_all_fourteen_canonical_specs passed; expect PTY printed the 14-entry list from /exit through /ultra-plan-run and exited with status 0.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `コマンドに続くフラグ名(`--profile` / `--style` / `--prompt-layout`)を補完する。` を確認できる画面または実機操作を行う。
- 期待結果: コマンドに続くフラグ名(`--profile` / `--style` / `--prompt-layout`)を補完する。
- Actual: Command option completion returned --profile, --prompt-layout, and --style with prefix matching.
- Evidence: Latest build c14b387: editor test command_and_flag_completion_are_prefix_matched passed; expect PTY changed --p to --pro and listed --profile plus --prompt-layout, status 0.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``--profile` の値はプロファイル定義(`src/planner/profile.rs` 系)を単一情報源として取得し、候補リストをハードコード複製しない。` を確認できる画面または実機操作を行う。
- 期待結果: `--profile` の値はプロファイル定義(`src/planner/profile.rs` 系)を単一情報源として取得し、候補リストをハードコード複製しない。
- Actual: Profile value completion reads the planner domain-profile registry rather than a TUI-local profile list.
- Evidence: Latest build c14b387: profile_completion_reads_the_domain_profile_registry passed and compares completion output directly with planner::profile::profile_names; DOMAIN_PROFILES drives runtime dispatch and profile_names.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``/run-plan` `/run-ultra-plan` `/resume` の引数、および `$(cat ` の直後でファイルパス補完が効く(workspace_root 相対)。` を確認できる画面または実機操作を行う。
- 期待結果: `/run-plan` `/run-ultra-plan` `/resume` の引数、および `$(cat ` の直後でファイルパス補完が効く(workspace_root 相対)。
- Actual: Workspace-relative path completion worked for /run-plan, /run-ultra-plan, /resume, quoted paths, and the argument after cat substitution; parent traversal was rejected.
- Evidence: Latest build c14b387: path_completion_is_workspace_relative_for_commands_and_cat passed; expect PTY completed .anvil/plans/st to .anvil/plans/step.yaml, status 0.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `補完候補の定義はスラッシュコマンド一覧(`render_help` / ディスパッチ)と単一情報源を共有し、コマンド追加時に補完だけ漏れる構造にしない(コンパイル時 or テストで同期を担保)。` を確認できる画面または実機操作を行う。
- 期待結果: 補完候補の定義はスラッシュコマンド一覧(`render_help` / ディスパッチ)と単一情報源を共有し、コマンド追加時に補完だけ漏れる構造にしない(コンパイル時 or テストで同期を担保)。
- Actual: Completion, help rendering, alias lookup, and command dispatch share the SLASH_COMMANDS registry containing 14 commands.
- Evidence: Latest build c14b387: slash_registry_is_the_help_and_alias_source passed and validates every registry entry against rendered help and lookup; command completion test consumes the same registry.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `入力中、履歴およびコマンド一覧からの前方一致サジェストを薄色(dim)で表示し、Right/End で受け入れられる(rustyline `Hinter`)。` を確認できる画面または実機操作を行う。
- 期待結果: 入力中、履歴およびコマンド一覧からの前方一致サジェストを薄色(dim)で表示し、Right/End で受け入れられる(rustyline `Hinter`)。
- Actual: History hints take precedence, command hints provide fallback, dim styling is used when enabled, and Right or End accepts the hint.
- Evidence: Latest build c14b387: hints_prefer_history_then_fall_back_to_commands and color_hint_uses_dim_sgr_only passed; separate expect PTY runs accepted /ex to /exit with Right and End and both exited status 0.
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``NO_COLOR` 設定時はヒント表示に色を使わない(`src/tui/terminal.rs:19-21` の判定を再利用)。` を確認できる画面または実機操作を行う。
- 期待結果: `NO_COLOR` 設定時はヒント表示に色を使わない(`src/tui/terminal.rs:19-21` の判定を再利用)。
- Actual: NO_COLOR disables SGR styling for hints while retaining a plain continuation prompt.
- Evidence: Latest build c14b387: no_color_hints_and_continuation_prompt_are_plain_ascii passed; all manual expect PTY runs used NO_COLOR=1 and rendered plain hint text.
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `rustyline `Validator` により、末尾 `\` またはダブルクォート未閉の行は継続入力(2行目以降は `... ` 等の継続プロンプト)。` を確認できる画面または実機操作を行う。
- 期待結果: rustyline `Validator` により、末尾 `\` またはダブルクォート未閉の行は継続入力(2行目以降は `... ` 等の継続プロンプト)。
- Actual: A trailing backslash or unclosed double quote keeps input incomplete and displays the continuation prompt.
- Evidence: Latest build c14b387: validator_detects_trailing_backslash_and_unclosed_quote passed; expect PTY entered /help followed by a trailing backslash and displayed ... before accepting the second line, status 0.
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `bracketed paste を有効化し、改行を含むテキストの貼り付けが1回の入力として扱われる(貼り付け途中で意図せず送信されない)。` を確認できる画面または実機操作を行う。
- 期待結果: bracketed paste を有効化し、改行を含むテキストの貼り付けが1回の入力として扱われる(貼り付け途中で意図せず送信されない)。
- Actual: Bracketed multiline paste remained one editable input and was not submitted until a separate Enter key.
- Evidence: Latest build c14b387: bracketed_paste_is_explicitly_enabled passed; expect PTY sent bracketed-paste start/end around two lines, then only the later Enter produced the help response, status 0.
- Result: passed

#### Scenario 10

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `複数行入力の結果は既存の `parse_words` / `handle_command` にそのまま渡せる1つの文字列に正規化される。` を確認できる画面または実機操作を行う。
- 期待結果: 複数行入力の結果は既存の `parse_words` / `handle_command` にそのまま渡せる1つの文字列に正規化される。
- Actual: Multiline input was normalized to a single space-separated command string accepted by the existing word parser and command handler.
- Evidence: Latest build c14b387: multiline_input_normalizes_for_existing_word_parser passed with /plan-run first second third; continuation and paste PTY sessions normalized /help plus the second line and rendered Commands, status 0.
- Result: passed

#### Scenario 11

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `入力中の行が空でない場合: Ctrl+C は行をクリアして新しいプロンプトを表示(終了しない)。` を確認できる画面または実機操作を行う。
- 期待結果: 入力中の行が空でない場合: Ctrl+C は行をクリアして新しいプロンプトを表示(終了しない)。
- Actual: Ctrl+C on a nonempty input cleared the line and returned to a fresh prompt without exiting.
- Evidence: Latest build c14b387: ctrl_c_requires_two_uninterrupted_empty_line_presses passed; expect PTY typed x, sent Ctrl+C, then continued at a fresh prompt and exited normally, status 0.
- Result: passed

#### Scenario 12

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `行が空の場合: 1回目で `press Ctrl+C again to exit` 相当のメッセージを表示、**連続2回目**で正常終了(履歴保存を含む通常の終了経路を通ること)。間に他のキー入力があればカウントはリセット。` を確認できる画面または実機操作を行う。
- 期待結果: 行が空の場合: 1回目で `press Ctrl+C again to exit` 相当のメッセージを表示、**連続2回目**で正常終了(履歴保存を含む通常の終了経路を通ること)。間に他のキー入力があればカウントはリセット。
- Actual: The first empty-line Ctrl+C warned, the second consecutive press exited normally, and intervening input reset the sequence.
- Evidence: Latest build c14b387: Ctrl+C state-machine test passed; expect PTY observed press Ctrl+C again to exit twice around an intervening x plus clear, then the final consecutive Ctrl+C exited status 0.
- Result: passed

#### Scenario 13

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ctrl+D(EOF)の即時終了は現状維持。` を確認できる画面または実機操作を行う。
- 期待結果: Ctrl+D(EOF)の即時終了は現状維持。
- Actual: Ctrl+D at an empty prompt exited immediately through the normal REPL termination path.
- Evidence: Latest build c14b387: dedicated expect PTY sent Ctrl+D at the first prompt and the process exited status 0 without an additional prompt.
- Result: passed

#### Scenario 14

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `コマンド実行中の Esc/Ctrl+C 割り込みセマンティクス(`src/tui/interrupt.rs`)には影響を与えない。` を確認できる画面または実機操作を行う。
- 期待結果: コマンド実行中の Esc/Ctrl+C 割り込みセマンティクス(`src/tui/interrupt.rs`)には影響を与えない。
- Actual: Execution-time Esc/Ctrl+C interrupt and forced-finalization semantics remained intact.
- Evidence: Latest build c14b387: five tui::interrupt tests passed; in_flight_provider_interrupt_finishes_before_sleep_and_writes_terminal_records and in_flight_bash_interrupt_force_finalizes_without_waiting_for_grace both passed with interrupted terminal records.
- Result: passed

#### Scenario 15

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `stdin が TTY でない場合の bail(`src/tui/repl.rs:11-13`)は現状維持。` を確認できる画面または実機操作を行う。
- 期待結果: stdin が TTY でない場合の bail(`src/tui/repl.rs:11-13`)は現状維持。
- Actual: REPL startup with non-TTY stdin still fails with the required action guidance.
- Evidence: Latest build c14b387: tui_non_tty_requires_action passed and asserted the stdin is not a TTY diagnostic.
- Result: passed

#### Scenario 16

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `非UTF-8ロケール(`LC_ALL`/`LANG` 判定は `src/tui/terminal.rs:23-30`)でも文字化けしない。` を確認できる画面または実機操作を行う。
- 期待結果: 非UTF-8ロケール(`LC_ALL`/`LANG` 判定は `src/tui/terminal.rs:23-30`)でも文字化けしない。
- Actual: The REPL, hints, completion lists, continuation prompt, and control-key messages rendered without mojibake in a non-UTF-8 locale.
- Evidence: Latest build c14b387: all expect PTY sessions ran with LC_ALL=C and LANG=C, including slash candidates, flags, paths, multiline paste, Ctrl+C, and Ctrl+D; all exited status 0, and plain-ASCII hint test passed.
- Result: passed

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
