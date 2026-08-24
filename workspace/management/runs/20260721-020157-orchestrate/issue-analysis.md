# Issue Analysis

## Issue #61: [ux][bug] Prevent stair-step REPL output from LF-only writes in raw mode

- 種別: `bug`
- 目的: macOS の対話REPLで長い `/ultra-plan-run` を確定すると、`Accepted command` カードの各行が右へ階段状にずれる。
- 詳細化要否: `no`

### 受入条件

- raw mode中でも、受理カードの各論理行が意図した列から始まる。
- `Accepted command`、`- Input:`、`- Command:`、`- Goal:`、profile/style/layout/port/run IDが階段状にずれない。
- 長い日本語・CJK Goalの継続行は、prefixに対応した意図的なインデントだけを持つ。
- footer on/off、color/no-color、wide/narrow terminalで成立する。
- 受理済みGoal全文の保持、scrollback永続化、footer resize/cleanupを壊さない。
- raw mode中に同じLF-only経路を使うMarkdown stream、failure block、status/summary等の複数行出力も監査する。
- event名・JSON schema・`.anvil/` runtime namespaceを変更しない。

### 承認済み判断

- None

### 推定影響ファイル

- src/tui/repl.rs
- src/tui/interrupt.rs
- src/tui/command_receipt.rs
- src/tui/footer.rs
- tests/tui_pty.rs
- tests/corpus/apps/test0708_007/fixtures/events-exhaustion-pending-evidence.jsonl
- README.md
- docs/assets/ux-demo.md

### 参考情報

- None

### テスト期待値

- cargo test
- cargo clippy
- cargo fmt

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
