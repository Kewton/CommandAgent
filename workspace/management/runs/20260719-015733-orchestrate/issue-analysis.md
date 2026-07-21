# Issue Analysis

## Issue #10: [ux] Modernize REPL input: slash-command completion, hints, multi-line input, Ctrl+C conventions

- 種別: `enhancement`
- 目的: インタラクティブモード(REPL)の入力体験を最先端のコーディングエージェントCLI(Claude Code / Codex CLI / Gemini CLI)水準に近づける第一歩として、**入力レイヤー**(補完・ヒント・複数行入力・Ctrl+C作法)を近代化する。
- 詳細化要否: `yes`

### 受入条件

- None

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

- 受入条件が明確ではありません。期待する完了条件を1-3点で補足してください。

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
