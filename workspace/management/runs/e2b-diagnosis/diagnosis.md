# E-2b段階1 単独失敗診断

## 行列

| revision | command | result |
|---|---|---|
| HEAD `ae8aeae` | `cargo test --lib planner::runner::tests::final_acceptance_budget_exhaustion_uses_last_cycle_reason -- --exact` | 再実行では pass。full-suite内では失敗（flaky） |
| `383952e` | 同上 | pass（1 passed） |
| `3481ab2` | 同上 | 実行セッションがビルド中に終了し完了値なし |

HEAD単独は同一環境で結果が揺れたため、移行起因の確定差分とは言えない。

## no-fail-fast full suite

- command: `cargo test --all-targets --no-fail-fast`
- result: 1615 passed / 15 ignored / 1 failed
- failed: `planner::runner::tests::final_acceptance_budget_exhaustion_uses_last_cycle_reason`
- panic: `called Result::unwrap_err() on an Ok value: "ultra-plan-run complete: 2 phases"`

## 解剖

テストは`planner::runner`の最終受理予算枯渇シナリオを構築し、budget exhaustion由来のErrとlast-cycle reasonを期待する。実測では計画が2 phasesで正常完了し、Err境界に到達しなかった。schema読込はinvestigate経路の構成だけを参照し、このテストのfix／final-acceptance経路を変更しないため、失敗原因の移行起因性は未確定。レビュー裁定まで修正しない。

## 原文

```text
thread 'planner::runner::tests::final_acceptance_budget_exhaustion_uses_last_cycle_reason' panicked at src/planner/runner.rs:13737:14:
called `Result::unwrap_err()` on an `Ok` value: "ultra-plan-run complete: 2 phases"
```
