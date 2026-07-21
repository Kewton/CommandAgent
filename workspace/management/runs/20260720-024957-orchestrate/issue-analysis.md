# Issue Analysis

## Issue #31: Add a clean release build that leaves only the executable

- 種別: `unknown`
- 目的: `cargo build --release` currently leaves Cargo link-time artifacts and stale hashed variants under `target/release/deps`.
- 詳細化要否: `no`

### 受入条件

- A documented repository command produces an optimized `commandagent` executable.
- The published executable reports the expected commit/version provenance through `--version`.
- After a successful clean release build, `target/release/deps` is absent or contains no generated libraries.
- `commandagentdev --version` succeeds after the build.
- An induced build or verification failure preserves the previously published executable.
- Temporary build artifacts are removed after both success and failure.
- Ordinary `cargo build`, `cargo test`, and development caching semantics remain unchanged.
- Focused automated tests cover success, failure preservation, cleanup, and launcher-path compatibility.
- `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` pass.

### 承認済み判断

- None

### 推定影響ファイル

- src/lib.rs
- src/main.rs
- build.rs
- docs/codex-harness.md
- docs/mechanism-ledger.md
- docs/model-probe.md
- docs/uat/scenarios.md
- scripts/eval_lib/report.py

### 参考情報

- None

### テスト期待値

- cargo test
- cargo clippy
- cargo fmt
- cargo build

### ユーザーへの質問

- None

### GitHub Issue 反映候補

詳細化要否が `yes` の場合、ユーザー回答後に反映する。
