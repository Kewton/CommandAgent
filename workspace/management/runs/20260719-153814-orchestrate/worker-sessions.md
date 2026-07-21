# Worker Sessions

## Issue #11

- Branch: `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #12

- Branch: `feature/issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 3 failed worker verification

## Issue #13

- Branch: `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #14

- Branch: `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #15

- Branch: `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

## Issue #16

- Branch: `feature/issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 3 failed worker verification

## Issue #17

- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 3 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-14-ux-accept-and-queue-user-input-while-a-command-i Codex issue worker task for Issue #14

If `$codex-issue-worker` is available in this worktree, follow that skill.
If it is not available, treat this message as the full worker instruction.

## Required Workflow

1. Read the Issue summary, acceptance criteria, approved decision, suspected files, and references.
2. Write a short design note before editing.
3. Implement the smallest coherent change that satisfies the Issue.
4. Add or update focused tests where appropriate.
5. Run focused verification, and broader checks if shared contracts are touched.
6. Write `dev-reports/issue-<number>/design.md`, `implementation-summary.md`, and `verification.md`.
7. In `verification.md`, record the exact line "- Status: `passed`" only when every required check passed, followed by one "- `<command>`: `passed`" entry per check. Use `blocked` when any required check fails or cannot run.
8. Commit the work with a clear Issue-scoped commit message.
9. Report blockers only if implementation cannot safely proceed.

## Issue Summary

- Title: [ux] Accept and queue user input while a command is running
- Objective: コマンド実行中、ユーザーのキー入力は Esc/Ctrl+C(割り込み)以外**すべて破棄**される。Claude Code のような「実行中に次の指示を打っておき、完了後に順次処理される」体験を実現する。

## Acceptance Criteria

- 実装前にPR説明(または本Issueコメント)に短い設計ノートを書く: キーイベントの所有権(割り込み監視スレッドとの統合方法)、表示位置(フッター直上 or フッター内のペンディング行)、Escセマンティクスの整理。
- 実行中に印字可能キーを打つと、ペンディング入力バッファに蓄積され、画面上(推奨: フッターの上または2行フッターの1行を転用)にエコー表示される。Backspaceで編集可能。
- Enter でペンディング行がキューに積まれ、`queued: <先頭40文字>…` のような形で確認表示される。複数行キュー可。
- 実行完了後、キューされた行が入力された順に通常のREPL入力として処理される(履歴にも記録)。処理前に各行が何であるかが表示される。
- 割り込みキーのセマンティクス変更は次のとおり: **Ctrl+C は従来どおり常に中断要求**。Esc はペンディングバッファが非空なら「バッファクリア」、空なら従来どおり中断要求。この差はヘルプ(`/help`)とフッター表示で分かるようにする。
- キュー内容はメモリのみ(プロセス終了で消えてよい)。上限(例: 10行、各4KiB)を設け、超過時は明示的に拒否メッセージを出す。
- `ANVIL_NO_INTERRUPT` 設定時(`src/tui/interrupt.rs:28`)は本機能も無効(従来どおり)。
- 非TTY・フッター無効時も安全に無効化される。
- ペンディング行のエコーが、スピナー・フッター・本文出力(ストリーミング導入後はトークン流)と衝突して画面が壊れないこと。既存のfreeze/pauseガードの枠内で実装する。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/interrupt.rs
- src/tui/input_queue.rs
- docs/dev-guardrails.md
- tests/tui_integration.rs
- src/tui/repl.rs
- src/tui/mod.rs
- docs/generality.md
- docs/uat/scenarios.md

## References

- なし

## Required Predecessors

- Issue #11: branch `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`, worktree `../CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Issue #13: branch `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`, worktree `../CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Issue #15: branch `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`, worktree `../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i
- Worktree: ../CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
