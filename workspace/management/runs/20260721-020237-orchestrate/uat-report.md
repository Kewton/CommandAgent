# UAT Report

## Merge Gate

- Status: `passed`
- Message: all 7 UAT scenarios passed with evidence

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #61: [ux][bug] Prevent stair-step REPL output from LF-only writes in raw mode

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `raw mode中でも、受理カードの各論理行が意図した列から始まる。` を確認できる画面または実機操作を行う。
- 期待結果: raw mode中でも、受理カードの各論理行が意図した列から始まる。
- Actual: PR HEAD d886672 の raw-mode PTY transcript で、受理カードの全論理行が意図した端末列から開始した。
- Evidence: CI 成功後に COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty tui_pty_screen_state_preserves_long_accepted_goal_across_footer_modes -- --ignored --nocapture --test-threads=1 を実行し、1 passed / 0 failed（31.15s）。テストは escape sequence、CR/LF、CJK 表示幅、端末折返しを追跡して各行の実カーソル列を比較する。
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``Accepted command`、`- Input:`、`- Command:`、`- Goal:`、profile/style/layout/port/run IDが階段状にずれない。` を確認できる画面または実機操作を行う。
- 期待結果: `Accepted command`、`- Input:`、`- Command:`、`- Goal:`、profile/style/layout/port/run IDが階段状にずれない。
- Actual: Accepted command と Input、Command、Goal、Profile、Style、Prompt layout、Requested port、Run ID の各トップレベル行は列 0 から始まり、階段状のずれはなかった。
- Evidence: 同じ post-CI PTY テストの assert_receipt_cursor_columns が 8 フィールドすべての存在、leading_spaces=0、actual_column=0 を footer/color の4構成で検証して成功。cargo test tui:: も 147 passed / 0 failed。
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `長い日本語・CJK Goalの継続行は、prefixに対応した意図的なインデントだけを持つ。` を確認できる画面または実機操作を行う。
- 期待結果: 長い日本語・CJK Goalの継続行は、prefixに対応した意図的なインデントだけを持つ。
- Actual: 長い日本語・CJK の Input と Goal は欠落せず、折返し継続行だけが各 prefix の表示幅に一致する意図的インデントを持った。
- Evidence: post-CI PTY テストが Input/Goal の継続行を必須化し、actual cursor column と prefix display width の一致を検証。cargo test tui:: 内の receipt_preserves_wrapped_cjk_goal_and_explicit_fields も成功。
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer on/off、color/no-color、wide/narrow terminalで成立する。` を確認できる画面または実機操作を行う。
- 期待結果: footer on/off、color/no-color、wide/narrow terminalで成立する。
- Actual: footer on/off、color/no-color の全4構成、および 48 列から 72 列へのリサイズ後もカーソル列と表示が正しかった。
- Evidence: post-CI PTY matrix を単一スレッドで実行して 1 passed / 0 failed。対象テストは全4構成をループし、狭幅開始後に72列へ resize して transcript と画面状態を検査する。
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `受理済みGoal全文の保持、scrollback永続化、footer resize/cleanupを壊さない。` を確認できる画面または実機操作を行う。
- 期待結果: 受理済みGoal全文の保持、scrollback永続化、footer resize/cleanupを壊さない。
- Actual: 受理済み CJK Goal 全文、scrollback の status/failure 出力、footer の resize/cleanup が保持された。
- Evidence: post-CI PTY テストが長い Goal の前半・後半、status/failure、footer 有無と cleanup を検証して成功。cargo test tui:: の footer scrollback/resize/shutdown 関連を含む147テストも全成功。
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `raw mode中に同じLF-only経路を使うMarkdown stream、failure block、status/summary等の複数行出力も監査する。` を確認できる画面または実機操作を行う。
- 期待結果: raw mode中に同じLF-only経路を使うMarkdown stream、failure block、status/summary等の複数行出力も監査する。
- Actual: raw mode の受理カードに加え、Markdown の raw/batch/rendered stream と末尾改行が同じ raw-mode-aware stdout helper を通ることを確認した。
- Evidence: src/tui/markdown.rs の各 stdout 境界（706, 720, 742, 746, 768, 770行）と src/tui/footer.rs の scrollback 境界（581, 583行）が write_stdout_text を使用。cargo test tui:: で stream=batch、failure/status/summary、terminal helper を含む147テストが成功。
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `event名・JSON schema・`.anvil/` runtime namespaceを変更しない。` を確認できる画面または実機操作を行う。
- 期待結果: event名・JSON schema・`.anvil/` runtime namespaceを変更しない。
- Actual: 変更は TUI 実装・PTY テスト・Issue 61 の新規レポートに限定され、event 名、JSON schema、.anvil runtime namespace、corpus、既存 run 記録を変更していない。
- Evidence: git diff --name-only origin/develop...d886672 は8ファイルのみ。保護パス .anvil/tests/corpus/workspace/management/runs の diff は空、event_name/schema_version と EVENT 定義の追加削除も空。GitHub CI 成功、cargo fmt、厳格 clippy、全 cargo test（lib 1576 passed / 0 failedほか）も成功。
- Result: passed

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
