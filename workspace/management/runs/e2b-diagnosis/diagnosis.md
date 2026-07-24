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

## 反復行列（2026-07-24）

| revision | run 1 | run 2 | run 3 | 対象テストfail |
|---|---|---|---|---|
| HEAD `ae8aeae` | 0 failed | 0 failed | 0 failed | 0/3 |
| 基線 `383952e` | 0 failed | 0 failed | 0 failed | 0/3 |

6回とも`final_acceptance_budget_exhaustion_uses_last_cycle_reason`はpassし、他の失敗名もなし。全体のno-fail-fastでは一度だけ発現したため、低頻度flakeとして記録する。次回発現時は同テスト単独→並列抑制→共有override状態の順に再診断する。

## テスト解剖（ソース読解）

- 計画由来: `src/planner/runner.rs:13680`のcreate intent／nextjs profile、`UltraPlan`に`first`と`final`の2 phasesを直接構築し、FakeClientとinteraction probe overrideを注入する。
- 予算・期待: `dev_server_probe_test_guard`を取得し、3つのprobe結果（state missing→recovery not observed→recovery not observed）を設定する。final acceptance cycle 2でErrと`final_acceptance_repair_exhausted`を期待する。
- 共有状態候補: `static DEV_SERVER_PROBE_TEST_LOCK: Mutex<()>`、`static TEST_DEV_SERVER_PORT: AtomicUsize`（runner.rs:18034-18044）、interaction probeのworkspace override/result override、環境変数／ポート割当。テストはguardを持つが、並列テストやoverrideの残留が低頻度揺れの候補である。

## 原文

```text
thread 'planner::runner::tests::final_acceptance_budget_exhaustion_uses_last_cycle_reason' panicked at src/planner/runner.rs:13737:14:
called `Result::unwrap_err()` on an `Ok` value: "ultra-plan-run complete: 2 phases"
```
