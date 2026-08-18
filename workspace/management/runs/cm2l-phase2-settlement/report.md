# 結果サマリ

- 判定: **Phase 2 GOを清算確定**。CM-2k golden-008の4判定線は全て達成し、
  CM-2lのCI infra床返済後、CI `32099916047` とacceptance `32099915895` は
  ともに`success`で確定した。これによりCM-2kの受理条件も完了した。
- CI床: `Detect GUI changes`だけが3 attempt連続で
  `fatal: shallow file has changed since we read it`となった。主テストjobは
  3 attemptともgreenだった。checkoutを`fetch-depth: 0`へ固定後、対象jobを含む
  全CIがgreenとなった。
- golden-008: 36/36完走、一発full 29/36=80.6%（Wilson 95%
  [65.0%, 90.2%]）、修復込みfull 34/36=94.4%（[81.9%, 98.5%]）、
  p50 174.5秒、最大cost $0.00252714、合計$0.04820040。
- 封緘3層: adversarial/golden/schemaのmanifest SHA-256は全て不変で、
  全entryの再照合もpassした。
- 未実施・逸脱・新規床: なし。既知のQUEUED 2床は本書で責任範囲を固定した。

## CI infra床の返済証跡

| 対象 | 修正前 | 修正後 |
|---|---|---|
| CI | run `32098404720`、3 attemptともworkflow failure | run `32099916047`、`success` |
| Detect GUI changes | merge-base探索中のshallow deepenが3回とも同一原文で競合 | full history checkout後、job `95598309698`が8秒で`success` |
| 主テストjob | 3 attemptとも`success` | `success` |
| acceptance | run `32098404692`、`success` | run `32099915895`、`success` |

修正前の停止原文は次のとおりで、Rust製品コードや主テストのfailureではない。

```text
fatal: shallow file has changed since we read it
```

`dorny/paths-filter`がpush時のbase/head差分を求める前にcheckoutを完全履歴へ
固定し、action内部の反復的なshallow fetchを不要にした。分類は
`ci_infra_shallow_fetch_race`（environment、解消済み）として
`workspace/management/classes.toml`へ登録した。

## Phase 2 GO判定の確定値

| 判定線 | golden-008実測 | Wilson 95% CI / 分布 | 閾値との差 | 判定 |
|---|---:|---:|---:|---|
| 一発full（修復0） | 29/36 = 80.6% | [65.0%, 90.2%] | +20.6ポイント | 達成 |
| 修復込みfull | 34/36 = 94.4% | [81.9%, 98.5%] | +4.4ポイント | 達成 |
| 所要p50 | 174.5秒 | p95=216.25秒 | 5.5秒下回る | 達成 |
| 1生成cost_usd | 最大$0.00252714 | min=$0.00086468、median=$0.00122860 | $0.06447286下回る | 達成 |

36 runのcost合計は`$0.04820040`（清算表示 `$0.048`）、計測wall timeは
6,434秒（1時間47分14秒）だった。4判定線の全達成とCI/acceptance両greenを
もって **Phase 2 GO** を最終確定する。

## 床10系統の系譜

| 系統 | 除外窓・床 | 清算状態 |
|---:|---|---|
| 1 | golden-001: schema供給床 | pinned schemaのworkspace注入で返済 |
| 2 | golden-001/002: cost配線床 | events usage、pricing、summary転記を接続して返済 |
| 3 | golden-002: planner伝達床 | community正形とverify語彙のprofile guidanceで返済 |
| 4 | golden-003/004: spec字義・verify依存床 | schema照合済み字義例と自己完結verifyへ是正 |
| 5 | golden-004: install配置床 | workspace内binary配置と支出前SHA照合へ是正 |
| 6 | golden-004r: Ollama不達12件 | provider到達性preflightと0run停止で返済 |
| 7 | cm2f-resume-001: 閉語彙床3件 | schemaからの完全語彙機械生成で返済 |
| 8 | golden-005/006: computed連鎖・core manifest・B系誤適用 | schema v0.1、core供給、level適用性で返済 |
| 9 | golden-007: 検証適用性 | L2=S/Z/材料、L3/L4=S+Z+Bへ契約固定 |
| 10 | golden-007: ラダー強制欠落 | planner/Zのpromotion二重ゲートで返済 |

## 封緘3層

| 層 | manifest | SHA-256 | 再照合 |
|---|---|---|---|
| adversarial 22 files | `workspace/management/bench/adversarial/sha256sums.txt` | `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b` | 全21 entry `OK`（manifest自身を含め22 files） |
| golden 3 suites | `workspace/management/bench/suites/community-golden.sha256sums` | `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86` | 3/3 `OK` |
| schema v0.1 | `workspace/management/bench/community/appspec-schema/manifest.sha256sums` | `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1` | 8/8 `OK` |

## QUEUEDの責任範囲

1. `community_l2_verify_invocation_incomplete`: profileの完全な`--prompt`形と
   競合する共有deterministic verification preferenceが短いinteractive起動形を
   配布する床。Community L2のverify起動形を一意にする候補であり、CM-2lでは
   製品挙動を変更しない。
2. `community_computed_unregistered`: v0.1の同一entity内DAGは対応済みだが、
   `len(packingItem)`のようなcollection/global集約は未対応。schema v0.2候補として
   設計裁定を待つ。

これらはgolden-008で正直終端した2/36の帰属であり、promotion gateや検証を
弱めずに保持する。詳細な企画書改訂材料は`product-plan-v2.1-material.md`、
確定bandは`band_summary_community.md`、GO主文は`ledger.md`を正本とする。
