# UAT Report

## Merge Gate

- Status: `pending`
- Message: missing UAT evidence for Issue #31 scenario 1

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #31: Add a clean release build that leaves only the executable

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `A documented repository command produces an optimized `commandagent` executable.` を確認できる画面または実機操作を行う。
- 期待結果: A documented repository command produces an optimized `commandagent` executable.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `The published executable reports the expected commit/version provenance through `--version`.` を確認できる画面または実機操作を行う。
- 期待結果: The published executable reports the expected commit/version provenance through `--version`.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `After a successful clean release build, `target/release/deps` is absent or contains no generated libraries.` を確認できる画面または実機操作を行う。
- 期待結果: After a successful clean release build, `target/release/deps` is absent or contains no generated libraries.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``commandagentdev --version` succeeds after the build.` を確認できる画面または実機操作を行う。
- 期待結果: `commandagentdev --version` succeeds after the build.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `An induced build or verification failure preserves the previously published executable.` を確認できる画面または実機操作を行う。
- 期待結果: An induced build or verification failure preserves the previously published executable.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Temporary build artifacts are removed after both success and failure.` を確認できる画面または実機操作を行う。
- 期待結果: Temporary build artifacts are removed after both success and failure.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ordinary `cargo build`, `cargo test`, and development caching semantics remain unchanged.` を確認できる画面または実機操作を行う。
- 期待結果: Ordinary `cargo build`, `cargo test`, and development caching semantics remain unchanged.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Focused automated tests cover success, failure preservation, cleanup, and launcher-path compatibility.` を確認できる画面または実機操作を行う。
- 期待結果: Focused automated tests cover success, failure preservation, cleanup, and launcher-path compatibility.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.` を確認できる画面または実機操作を行う。
- 期待結果: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.
- Actual: unchecked
- Evidence: screenshot / relevant logs / 操作メモ / device or browser version
- Result: unchecked

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
