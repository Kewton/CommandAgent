# UAT Report

## Merge Gate

- Status: `pending`
- Message: missing UAT evidence for Issue #61 scenario 1

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #61: [ux][bug] Prevent stair-step REPL output from LF-only writes in raw mode

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `raw mode中でも、受理カードの各論理行が意図した列から始まる。` を確認できる画面または実機操作を行う。
- 期待結果: raw mode中でも、受理カードの各論理行が意図した列から始まる。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``Accepted command`、`- Input:`、`- Command:`、`- Goal:`、profile/style/layout/port/run IDが階段状にずれない。` を確認できる画面または実機操作を行う。
- 期待結果: `Accepted command`、`- Input:`、`- Command:`、`- Goal:`、profile/style/layout/port/run IDが階段状にずれない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `長い日本語・CJK Goalの継続行は、prefixに対応した意図的なインデントだけを持つ。` を確認できる画面または実機操作を行う。
- 期待結果: 長い日本語・CJK Goalの継続行は、prefixに対応した意図的なインデントだけを持つ。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `footer on/off、color/no-color、wide/narrow terminalで成立する。` を確認できる画面または実機操作を行う。
- 期待結果: footer on/off、color/no-color、wide/narrow terminalで成立する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `受理済みGoal全文の保持、scrollback永続化、footer resize/cleanupを壊さない。` を確認できる画面または実機操作を行う。
- 期待結果: 受理済みGoal全文の保持、scrollback永続化、footer resize/cleanupを壊さない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `raw mode中に同じLF-only経路を使うMarkdown stream、failure block、status/summary等の複数行出力も監査する。` を確認できる画面または実機操作を行う。
- 期待結果: raw mode中に同じLF-only経路を使うMarkdown stream、failure block、status/summary等の複数行出力も監査する。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `event名・JSON schema・`.anvil/` runtime namespaceを変更しない。` を確認できる画面または実機操作を行う。
- 期待結果: event名・JSON schema・`.anvil/` runtime namespaceを変更しない。
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
