# G-1 Next.js acceptance flaky決定化

- 実施日: 2026-08-15 (Asia/Tokyo)
- 基点: `022ade69`（変更前HEAD）
- production gate / 判定閾値の変更: なし
- 変更範囲: `src/planner/runner/tests/acceptance_boundary_tests.rs` のtest fixture 2行のみ

## 処置

次の2試験に、同じtest moduleで既に使用している
`write_fake_nextjs_package_manager(dir.path(), false)`を注入した。

- `final_acceptance_budget_exhaustion_uses_last_cycle_reason`
- `final_acceptance_repair_cycle_reprobes_restart_hook_recovery_to_pass`

fixtureが生成する`package.json`、契約hook、probe result列、repair cycle、
最終verdictのassertは変更していない。fake package managerはbuildを成功させ、
dev commandをtest binary内のfake dev server childへ委譲する。このため検証対象の
Gate判定は保ったまま、実registryへのdependency setupと実Next.js dev serverの
ready時刻だけを試験境界から除いた。

## 反復結果

両試験を`NODE_ENV`未設定、loopback利用可能環境で各20回、`--exact`で実行した。
全試行の結果と秒単位の所要は[`repetition-results.log`](repetition-results.log)に
記録した。

| test | green | failed | 合計所要 |
|---|---:|---:|---:|
| budget exhaustion / last-cycle reason | 20/20 | 0 | 59秒 |
| repair cycle / restart-hook reprobe | 20/20 | 0 | 43秒 |

## 49feed6 CI failureとの同一族判定

**同一族**と判定する。`49feed64acbc9758b87c7c5d392be5ba105a1384`のCI
run `31175327521`は、同じbudget-exhaustion試験がdependency setupの
600,000ms timeoutを経て失敗した。今回の事前再現でも当該試験が期待した
budget exhaustionへ入らずsuccessとなり、siblingのrepair/reprobe試験も
実Next.jsが`Ready`を出した後にstartup timeoutとして揺れた。表面の終端は
timeout / unexpected successで異なるが、いずれも同じNext.js acceptance
fixtureが実package manager・実dev serverの外部時刻に依存していた族である。

既存のephemeral port leaseはport所有権のTOCTOUを閉じているが、package manager
とdev server readinessは決定化していなかった。今回の2行はこの残存境界を閉じる。
productionのtimeout、acceptance cycle上限、verdict閾値には触れていない。
