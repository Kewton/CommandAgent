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
- Worker status: `verified-complete`
- Running: `None`
- Processing: `False`
- Worker message: clean committed worker verification already passed

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
- Worker status: `sent`
- Running: `None`
- Processing: `None`
- Worker message: task sent

## Issue #17

- Branch: `feature/issue-17-brand-phase-3-decision-internal-protocol-identif`
- Worktree: `/Users/maenokota/share/work/github_kewton/CommandAgent-issue-17-brand-phase-3-decision-internal-protocol-identif`
- Status: `reused`
- Message: existing clean worktree reused
- Worker status: `blocked`
- Running: `None`
- Processing: `None`
- Worker message: not dispatched because scheduler batch 5 failed worker verification

## CommandMate Dispatch

- `commandmatedev send commandagent-feature-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c Codex issue worker task for Issue #16

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

- Title: [brand] Phase 2: Migrate ANVIL_* env vars and .anvil config paths to COMMANDAGENT_* with compatibility shims
- Objective: Anvil→CommandAgent リネームのPhase 2として、**機能的な外部インターフェース**である環境変数 `ANVIL_*` と設定ファイルパス `.anvil/config(.toml)` を、後方互換を保ったまま `COMMANDAGENT_*` / `.commandagent/` へ移行する。

## Acceptance Criteria

- 新旧環境変数のマトリクステスト(新のみ/旧のみ/両方/どちらも無し)が env_compat ヘルパーに対して存在し通過する。旧のみ時の警告が1回だけ出ることもテストする。
- 設定パス優先順位のテスト(新のみ/旧のみ/両方)が `src/config.rs` に追加され通過する。
- `rg -n 'ANVIL_' src build.rs scripts tests eval docs README.md` のヒットが「env_compat のフォールバック定義」と「フォールバック動作を検証するテスト」のみになる。
- `cargo build && cargo test --quiet` 全通過。`docs/mechanism-ledger.md` に本Phaseの記録を追記。

## Approved Decision

None
The approved decision is authoritative when it narrows or contradicts the original Issue narrative or inferred file scope.

## Suspected Files

- src/env_compat.rs
- docs/dev-guardrails.md
- src/tui/terminal.rs
- tests/tui_pty.rs
- tests/live_provider.rs
- eval/README.md
- docs/uat/scenarios.md
- .github/workflows/ci.yml
- .anvil/config.toml
- ~/.anvil/config.toml
- README.md
- docs/mechanism-ledger.md
- src/config.rs
- src/tui/footer.rs
- src/tui/interrupt.rs
- src/tui/markdown.rs
- src/tui/spinner.rs
- src/eval_events.rs
- src/minimal_loop/completion.rs
- src/planner/runner.rs
- src/minimal_loop/interaction_probe.rs
- src/minimal_loop/loop_run.rs
- src/tui/ux_demo.rs
- src/build_info.rs
- scripts/bench.sh
- src/state.rs
- src/tui/status.rs
- github/workflows/ci.yml
- anvil/config.toml
- docs/generality.md

## References

- なし

## Required Predecessors

- Issue #11: branch `feature/issue-11-ux-extend-terminal-markdown-renderer-tables-nest`, worktree `../CommandAgent-issue-11-ux-extend-terminal-markdown-renderer-tables-nest`
- Issue #12: branch `feature/issue-12-ux-stream-assistant-output-token-by-token-in-the`, worktree `../CommandAgent-issue-12-ux-stream-assistant-output-token-by-token-in-the`
- Issue #13: branch `feature/issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`, worktree `../CommandAgent-issue-13-ux-handle-terminal-resize-for-the-fixed-footer-d`
- Issue #14: branch `feature/issue-14-ux-accept-and-queue-user-input-while-a-command-i`, worktree `../CommandAgent-issue-14-ux-accept-and-queue-user-input-while-a-command-i`
- Issue #15: branch `feature/issue-15-brand-phase-1-replace-remaining-user-visible-anv`, worktree `../CommandAgent-issue-15-brand-phase-1-replace-remaining-user-visible-anv`

The scheduler dispatches this Issue only after every listed dependency or file-conflict predecessor completed and passed verification. Inspect their committed changes before editing; do not assume those branches are already merged into this one.

## Orchestration Notes

- Branch: feature/issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c
- Worktree: ../CommandAgent-issue-16-brand-phase-2-migrate-anvil-env-vars-and-anvil-c
- Keep review lightweight and ask only blocking questions. --agent codex --auto-yes --duration 3h`
