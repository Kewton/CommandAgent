# Worker Sessions

## Issue #10

- Branch: `feature/issue-10-ux-modernize-repl-input-slash-command-completion`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-10-ux-modernize-repl-input-slash-command-completion`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-10-ux-modernize-repl-input-slash-command-completion Codex issue worker task for Issue #10

If `$codex-issue-worker` is available in this worktree, follow that skill.
If it is not available, treat this message as the full worker instruction.

## Required Workflow

1. Read the Issue summary, acceptance criteria, suspected files, and references.
2. Write a short design note before editing.
3. Implement the smallest coherent change that satisfies the Issue.
4. Add or update focused tests where appropriate.
5. Run focused verification, and broader checks if shared contracts are touched.
6. Write `dev-reports/issue-<number>/design.md`, `implementation-summary.md`, and `verification.md`.
7. In `verification.md`, record the exact line "- Status: `passed`" only when every required check passed, followed by one "- `<command>`: `passed`" entry per check. Use `blocked` when any required check fails or cannot run.
8. Commit the work with a clear Issue-scoped commit message.
9. Report blockers only if implementation cannot safely proceed.

## Issue Summary

- Title: [ux] Modernize REPL input: slash-command completion, hints, multi-line input, Ctrl+C conventions
- Objective: インタラクティブモード(REPL)の入力体験を最先端のコーディングエージェントCLI(Claude Code / Codex CLI / Gemini CLI)水準に近づける第一歩として、**入力レイヤー**(補完・ヒント・複数行入力・Ctrl+C作法)を近代化する。

## Acceptance Criteria

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

## Suspected Files

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

## References

- なし

## Orchestration Notes

- Branch: feature/issue-10-ux-modernize-repl-input-slash-command-completion
- Worktree: ../CommandAgent-issue-10-ux-modernize-repl-input-slash-command-completion
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
