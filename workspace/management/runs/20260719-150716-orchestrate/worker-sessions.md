# Worker Sessions

## Issue #11

- Branch: `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #12

- Branch: `feature/issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #13

- Branch: `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #14

- Branch: `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #15

- Branch: `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #16

- Branch: `feature/issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## Issue #17

- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 1 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-11-ux-extend-terminal-markdown-renderer-tables-nest Codex issue worker task for Issue #11

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

- Title: [ux] Extend terminal markdown renderer: tables, nested lists, links, code highlighting
- Objective: REPLのアシスタント出力は自前の最小Markdownレンダラー(`src/tui/markdown.rs`)で描画されるが、表現力が最先端CLIに比べ大きく不足している(表・ネストリスト・リンク・言語別ハイライト非対応)。レンダラーを拡張し、モデル出力の可読性を引き上げる。

## Acceptance Criteria

- `| a | b |` + 区切り行の形式を検出し、列幅を揃えて描画する(桁揃えは表示幅ベース。CJK等の全角文字幅を考慮すること)。
- アライメント指定(`:---`, `:---:`, `---:`)を反映する。
- 不正な表(列数不一致など)はクラッシュせずプレーンテキストとして出力する。
- 端末幅を超える表は、はみ出してもエスケープ列が壊れない(折り返しは端末任せで可)。
- 2レベル以上のネスト(インデント2spまたは4sp)を字下げ+異なるマーカーで描画する。
- 番号付きリスト(`1. `)に対応する。
- `[text](url)` を `text (url)` 形式で描画する(最低ライン)。TTYかつ色有効時のみOSC 8ハイパーリンクにするのは任意(実装する場合は非対応端末での劣化を確認)。
- fenced code の言語タグ(`js/ts/tsx/python/rust/bash/json` 程度)を見て、キーワード/文字列/コメントの3種を色分けする**軽量**なハイライトを実装する。
- 新規依存クレートの追加は原則不可。追加する場合は理由・サイズ・`default-features` 最小化をPRで説明すること(自前実装を推奨)。
- 未知の言語タグ・タグなしは現行どおり単色。
- `strip_think` / `sanitize` / 64KiB上限 / `NO_COLOR` / `ANVIL_NO_MARKDOWN` ゲートの既存挙動を一切変えない(既存ユニットテストがそのまま通ること)。
- コンテンツ由来の文字列からSGR/エスケープ注入が起きないこと(sanitize後に装飾を適用する順序を維持)。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/tui/markdown.rs
- src/tui/markdown/table.rs
- docs/dev-guardrails.md
- src/tui/footer.rs
- src/tui/terminal.rs
- Cargo.lock
- README.md
- docs/generality.md

## References

- なし

## Required Predecessors

- None

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest
- Worktree: ../CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
- `commandmatedev send commandagent-feature-issue-15-brand-phase-1-replace-remaining-user-visible-anv Codex issue worker task for Issue #15

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

- Title: [brand] Phase 1: Replace remaining user-visible Anvil branding (banner art, REPL prompt, planner persona, docs)
- Objective: 本リポジトリは Anvil(`anvilminimal`)からの移行で作られており、crate/binary名等のリネームは完了済み(`d05a410`, `835c04f`。互換方針は `docs/mechanism-ledger.md` 末尾の記録を参照)。しかし**ユーザーの目に直接触れるブランディング**にまだ "Anvil" が残っている。本Issueは**動作変更ゼロ**の純粋なブランディング置換(Phase 1)を行う。

## Acceptance Criteria

- REPL起動時の画面(バナー+プロンプト)に "Anvil"/"anvil" が一切表示されない。
- `rg -in 'anvil' src/tui src/repl.rs README.md docs/generality.md docs/perf-notes.md` のヒットが「環境変数 `ANVIL_*`」「`.anvil/` パス」のみになる(製品名としてのAnvilが残らない)。
- `cargo build && cargo test --quiet` 全通過、`ANVIL_PTY_TESTS=1 cargo test --test tui_pty` 通過。
- 動作変更ゼロ(文字列以外の差分がないこと)。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- docs/mechanism-ledger.md
- tests/corpus_regression.rs
- README.md
- .anvil/config.toml
- docs/generality.md
- docs/perf-notes.md
- docs/uat-corpus.md
- docs/uat/scenarios.md
- eval/README.md
- src/tui/banner.rs
- src/tui/repl.rs
- tests/tui_pty.rs
- src/planner/runner.rs
- src/minimal_loop/interaction_probe.rs
- docs/migration
- workspace/management/runs
- src/tui
- src/repl.rs
- anvil/config.toml

## References

- なし

## Required Predecessors

- None

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv
- Worktree: ../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
