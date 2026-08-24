# UAT Report

## Merge Gate

- Status: `passed`
- Message: all 9 UAT scenarios passed with evidence

## Automated Checks

- Worker command evidence: see `worker-verification.md`.
- Pull-request checks: see `ci-report.md`.

## Manual CLI / TTY / GUI / Real-device Checks

### Issue #31: Add a clean release build that leaves only the executable

#### Scenario 1

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `A documented repository command produces an optimized `commandagent` executable.` を確認できる画面または実機操作を行う。
- 期待結果: A documented repository command produces an optimized `commandagent` executable.
- Actual: The documented ./scripts/build-release.sh command completed successfully for PR head 5230131 and published an optimized commandagent executable.
- Evidence: ./scripts/build-release.sh exited 0 after Cargo finished the release profile and printed Published .../target/release/commandagent.
- Result: passed

#### Scenario 2

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `The published executable reports the expected commit/version provenance through `--version`.` を確認できる画面または実機操作を行う。
- 期待結果: The published executable reports the expected commit/version provenance through `--version`.
- Actual: The published executable reports package version 0.1.0 and the expected PR-head commit provenance 5230131.
- Evidence: target/release/commandagent --version exited 0 with: commandagent 0.1.0 5230131 2026-07-20T02:54:47Z; gh pr view confirmed headRefOid 52301315a41d343ac8021bb5d1da37001913aa9a.
- Result: passed

#### Scenario 3

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `After a successful clean release build, `target/release/deps` is absent or contains no generated libraries.` を確認できる画面または実機操作を行う。
- 期待結果: After a successful clean release build, `target/release/deps` is absent or contains no generated libraries.
- Actual: After the successful clean release build, target/release contains only the commandagent executable and no deps directory.
- Evidence: find target/release -mindepth 1 -maxdepth 1 -print returned only target/release/commandagent; find target/release -type d -name deps -print returned no entries.
- Result: passed

#### Scenario 4

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``commandagentdev --version` succeeds after the build.` を確認できる画面または実機操作を行う。
- 期待結果: `commandagentdev --version` succeeds after the build.
- Actual: The installed commandagentdev launcher succeeds, and a launcher-compatible symlink targeting the candidate executable reports the candidate provenance.
- Evidence: commandagentdev --version exited 0; /tmp/commandagent-issue31-uat/bin/commandagentdev --version exited 0 with commandagent 0.1.0 5230131; successful_build_publishes_only_the_executable_and_supports_launcher_symlink passed.
- Result: passed

#### Scenario 5

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `An induced build or verification failure preserves the previously published executable.` を確認できる画面または実機操作を行う。
- 期待結果: An induced build or verification failure preserves the previously published executable.
- Actual: Both an induced Cargo build failure and an induced provenance verification failure preserve the previously published executable.
- Evidence: cargo test --test release_build passed failed_build_preserves_previous_executable_and_removes_staging and failed_provenance_verification_preserves_previous_executable_and_removes_staging.
- Result: passed

#### Scenario 6

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Temporary build artifacts are removed after both success and failure.` を確認できる画面または実機操作を行う。
- 期待結果: Temporary build artifacts are removed after both success and failure.
- Actual: Temporary build and publish staging artifacts are removed after success and both tested failure paths.
- Evidence: find target -maxdepth 1 -name .commandagent-release-* -print returned no entries after the real build; all three release_build tests, including both staging-removal failure tests, passed.
- Result: passed

#### Scenario 7

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Ordinary `cargo build`, `cargo test`, and development caching semantics remain unchanged.` を確認できる画面または実機操作を行う。
- 期待結果: Ordinary `cargo build`, `cargo test`, and development caching semantics remain unchanged.
- Actual: Ordinary development Cargo build and test commands still succeed and do not repopulate target/release.
- Evidence: cargo build exited 0; cargo test exited 0 with 1521 unit tests passed and 15 ignored plus all integration/doc suites passing; a subsequent target/release listing still contained only commandagent.
- Result: passed

#### Scenario 8

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: `Focused automated tests cover success, failure preservation, cleanup, and launcher-path compatibility.` を確認できる画面または実機操作を行う。
- 期待結果: Focused automated tests cover success, failure preservation, cleanup, and launcher-path compatibility.
- Actual: Focused automated coverage exercises successful publication, launcher compatibility, build-failure preservation, provenance-failure preservation, and cleanup.
- Evidence: cargo test --test release_build exited 0: 3 passed, 0 failed, covering successful_build_publishes_only_the_executable_and_supports_launcher_symlink and both failure-preservation tests.
- Result: passed

#### Scenario 9

- 前提: 対象ドラフトPRの最新コミットまたは候補ビルドを使用する。
- 操作: ``cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.` を確認できる画面または実機操作を行う。
- 期待結果: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.
- Actual: Formatting, clippy with warnings denied, and the full test suite all pass on the PR head.
- Evidence: cargo fmt --all -- --check exited 0; cargo clippy --all-targets -- -D warnings exited 0; cargo test exited 0; PR #32 CommandAgent Test and Guardrails check passed.
- Result: passed

## Fix Loop

UAT が fail した場合は、該当 Issue / PR / file に mapping する。
そのうえで focused failure prompt から follow-up worktree を作成する。
Retry limit: 3
