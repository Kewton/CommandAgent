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
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #13

- Branch: `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #14

- Branch: `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

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
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## Issue #17

- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 2 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d Codex issue worker task for Issue #13

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

- Title: [ux] Handle terminal resize for the fixed footer during long runs
- Objective: 固定フッター(画面最下部の進捗バー)は起動時に一度だけ端末サイズを取得しており、長時間実行中に端末をリサイズするとスクロール領域が古いまま残り、フッターの位置ずれ・描画残骸が発生する。リサイズに追従させる。

## Acceptance Criteria

- `render_loop` の各tickで `terminal::size()` を再取得し、サイズ変化を検出したら (1) 旧スクロール領域を解除、(2) 新サイズで領域とフッター行数を再確立、(3) フッター内容を新幅で再フィットして描画する。
- 縮小時・拡大時とも、旧フッターの描画残骸(ゴミ行)が本文スクロールバックに残らない。
- 幅が100列しきい値を跨いだ場合、1行⇔2行のフッター行数が正しく切り替わる。
- 高さ縮小で本文カーソルがフッター領域に食い込まない。
- リサイズ後もシャットダウン時(通常終了・割り込み・パニック)のスクロール領域復元が正しく行われる。
- サイズ取得失敗時(`terminal::size()` がErr)は直前のジオメトリを維持し、クラッシュしない。
- フッター無効時・非TTY時の挙動は不変。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/footer.rs
- src/tui/banner.rs
- tests/tui_pty.rs
- src/lib.rs
- src/minimal_loop/evidence.rs
- src/minimal_loop/interaction_probe.rs
- src/minimal_loop/loop_run.rs
- src/minimal_loop/loop_run/repair_pressure_tests.rs

## References

- なし

## Required Predecessors

- Issue #11: branch `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`, worktree `../CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Issue #15: branch `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`, worktree `../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d
- Worktree: ../CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
