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
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

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
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

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
- Worker message: not dispatched because scheduler batch 4 failed worker verification

## Issue #17

- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 4 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-12-ux-stream-assistant-output-token-by-token-in-the Codex issue worker task for Issue #12

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

- Title: [ux] Stream assistant output token-by-token in the REPL
- Objective: 現在、アシスタント応答は**完了後に一括表示**され、待機中はスピナーと固定フッターのみが動く。最先端CLI(Claude Code / Codex CLI / Gemini CLI)との最大の体感差はここにある。プロバイダ応答をトークン単位でストリーミングし、逐次レンダリングする。

## Acceptance Criteria

- Ollama(NDJSON stream)/ OpenAI(SSE)/ Gemini(`streamGenerateContent` SSE)の3プロバイダでストリーミング受信を実装する。
- 3実装が共通のインクリメンタルAPI(チャンクコールバック or イテレータ)を実装し、呼び出し側はプロバイダ非依存になる。
- **非ストリーミング経路は残す**。設定(例: `stream = on|off`。CLIフラグ+configファイル+既存の設定優先順位規約に従う)で切替可能。既定はREPLの対話応答でON。非TTY・`--prompt` 一括実行・テスト用フェイククライアント経路は従来どおり非ストリーミングで動くこと。
- async runtime を導入しない。blocking `reqwest` のレスポンスを逐次読みし(`Read`)、SSE/NDJSONの行分割を自前で行う。**UTF-8マルチバイトがチャンク境界で分割されるケース**を正しく扱う。
- `chat_timeout_secs` の意味を維持または明確化する(推奨: ストリーム全体のwall-clock上限として適用し、挙動をドキュメント化)。リトライ(`chat_retries`)は「最初のトークン受信前の失敗」のみ対象とする。
- 受信チャンクを `TerminalMarkdownRenderer` に逐次投入して描画する。**同一内容を非ストリーミングで一括描画した場合と最終出力が一致**すること(受け入れテスト化)。
- `<think>` ブロックはストリーミング中も表示されない(既存 `strip_think` のチャンク境界対応を利用)。
- スピナーは「最初のトークン受信まで」に役割を縮小し、最初のトークン到達時に確実に消去してから本文描画を始める(スピナー行の残骸が出ないこと)。
- 固定フッターとの共存: ストリーミング描画中もフッターが壊れない(既存のfreeze/pauseガードの範囲で整合させる。スクロール領域の外に本文が書かれること)。
- ストリーミング中の Esc/Ctrl+C でチャンク境界チェックにより速やかに中断し、**それまでの部分出力はスクロールバックに残す**。端末状態(rawモード/スクロール領域)は正しく復元。中断時の終端レコード(`interrupted` / `aborted_by_user`)の既存挙動(`tests/tui_integration.rs:759` 以降)を壊さない。
- 接続断・不正なSSE/NDJSONは、部分出力+明確なエラーメッセージで終了しパニックしない。
- ツールコールXMLフォールバック(`src/providers/xml_fallback.rs`)は**累積テキスト全体**に対して従来どおり機能する(ストリーミングは表示のみの変更であり、パース入力は完成テキスト)。
- セッション保存・events.jsonl・eval関連の記録内容は非ストリーミング時と同一(表示方式の変更がデータに漏れない)。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/providers/openai.rs
- src/providers/gemini.rs
- Cargo.toml
- src/tui/spinner.rs
- src/tui/footer.rs
- src/tui/interrupt.rs
- src/providers/xml_fallback.rs
- src/providers/streaming.rs
- src/planner/runner.rs
- src/minimal_loop/loop_run.rs
- docs/dev-guardrails.md
- src/providers/ollama.rs
- src/tui/repl.rs
- src/tui/slash.rs
- src/tui/mod.rs
- src/tui/markdown.rs
- tests/tui_integration.rs
- src/minimal_loop/browser_probe.rs

## References

- なし

## Required Predecessors

- Issue #11: branch `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`, worktree `../CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Issue #13: branch `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`, worktree `../CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Issue #14: branch `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`, worktree `../CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Issue #15: branch `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`, worktree `../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-12-ux-stream-assistant-output-token-by-token-in-the
- Worktree: ../CommandAgent-issue-12-ux-stream-assistant-output-token-by-token-in-the
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
