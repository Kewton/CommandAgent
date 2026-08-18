# 結果サマリ

## 事前宣言（2026-08-18T11:09:47+09:00、実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run。
- Go/No-Go判定線: 一発full（修復サイクル0）≥60%、修復込みfull ≥90%、所要p50 ≤180秒、1生成cost_usd ≤$0.067。
- 事前予測帯: 直接priorなし。最近接の間接実測はingest×Luna 100% (n=6)、Quiz 92%。点予測は置かず、実測率にWilson 95% CIを付す。
- 予算上限: $5。超過見込みで停止する。
- 計器pin: implementation HEAD `a931b777`、measurement execution revision `a0f71b9741775b178a77945e8200e9d6f4a8df9c`、campaign配置binary SHA-256 `1811f186eccf164a1d76760813bac0ac7dfc2fc71f67d923dc119d3828ef116c`。benchは支出前にbuilt/installed/executed binaryのSHA一致を検査する。
- provider: executor=`gpt-5.6-luna` / OpenAI Responses native、planner=`qwen3.6:27b-coding-nvfp4` / Ollama、configured host=`http://127.0.0.1:11434`。
- sealed source suite SHA-256: warikan `215abae70c63d72be8bec4ad683b92d68b68349cd7edbeea5f741ee636cd9e0c`、mochimono `1246b8773ea3229dc1c728e3713b9e44346bd3daac734323872c496b61f7a9a6`、vote `da5166d86873a5cc9f31a212ea9bef3dd2e373c055a611425dd0ce73905d08df`、manifest `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86`。計測cloneは計器pin用の`bon_series` 1行だけを派生追加し、goal/run bytesは封緘sourceと同一。
- schema manifest SHA-256: `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1`。敵対的fixture manifest SHA-256: `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b`。CM-2kでは封緘fixture/schema/suiteを変更しない。

### 除外窓の全系譜（10系統）

| 系統 | 除外窓と理由 |
|---:|---|
| 1 | golden-001: schema供給床。pinned platform schemaがrun workspaceへ未供給。 |
| 2 | golden-001/002: cost配線床。usage/model pricingからsummaryへの転記がnull。 |
| 3 | golden-002: planner伝達床。community spec/verify正形がplannerへ届かず停止。 |
| 4 | golden-003/004: spec字義・verify依存床。誤ったfields形と依存設営を要するverify。 |
| 5 | golden-004: install配置床。sandbox外の既定binary配置先へ書き込めず0run停止。 |
| 6 | golden-004r: Ollama不達12件。provider計画前停止でtoken 0。 |
| 7 | cm2f-resume-001: 閉語彙床3件。schema由来の完全語彙未配布。 |
| 8 | golden-005/006: computed連鎖・core manifest供給・B系入力誤適用。 |
| 9 | golden-007: 検証適用性。L2はS/Z/材料、L3/L4はS+Z+Bへ条件化。 |
| 10 | golden-007: ラダー強制欠落。L2 pass後にpromotion記録なしでapp-zoneへ進んだ2件。CM-2kで計画品質/Zの二重ゲートへ変更。 |

L3誤昇格の消滅によりp50の大幅短縮を見込むが、この見込みは事前予測・分母・判定には使用しない。環境中断はrun非消費とする。同一系統の重大な乖離が連続した場合は無効データを量産せず途中停止する。4判定線を全て達成した場合のみPhase 2 GOを宣言し、一部未達なら乖離幅を裁定材料として停止する。

最初の2 preflight窓では、L2の`expected_paths`が`app.spec.yaml`だけでもinstruction proseにあるapp-zone/L3境界記述を旧lintが肯定的L3 stepと誤認した。いずれもwarikan_001をexecutor支出前に環境中断し、run非消費とした。成果物宣言の正本`expected_paths`だけからplanner gate適用性を導出し、実workspaceは独立したZ gateで検査する形へ修正後、implementation/measurement binaryを上記SHAへ再ピンした。この2窓は36 runの分母へ含めない。

## 実測結果

36/36を環境中断なしで完走した。支出前preflightは3 campaignすべてで
execution revision、built/installed/executed binaryのSHA-256一致、
Ollamaの`/api/tags`と宣言モデル名、OpenAI key存在と`/v1/models`を
確認してpassした。raw metadataと全workspaceのscrubもgreenである。

| 指標 | 実測 | Wilson 95% CI / 分布 | 閾値との差 | 判定 |
|---|---:|---:|---:|---|
| 一発full（修復0） | 29/36 = 80.6% | [65.0%, 90.2%] | +20.6ポイント | 達成 |
| 修復込みfull | 34/36 = 94.4% | [81.9%, 98.5%] | +4.4ポイント | 達成 |
| 所要p50 | 174.5秒 | p95=216.25秒 | 5.5秒下回る | 達成 |
| 1生成cost_usd | 最大$0.00252714 | min=$0.00086468、median=$0.00122860 | $0.06447286下回る | 達成 |

4判定線をすべて達成したため、事前確定規則に従い **Phase 2 GO** とする。
Wilson区間は点推定の不確実性として併記し、判定線との比較は事前宣言どおり
実測率・実測分布で行った。

### suite別と分布

| 種別 | run | 一発full | 修復込みfull | duration p50 | cost合計 |
|---|---:|---:|---:|---:|---:|
| warikan | 12 | 10/12 | 12/12 | 173.5秒 | $0.01574440 |
| mochimono | 12 | 9/12 | 10/12 | 178.5秒 | $0.01711858 |
| vote | 12 | 10/12 | 12/12 | 173.5秒 | $0.01533742 |
| 全体 | 36 | 29/36 | 34/36 | 174.5秒 | $0.04820040 |

durationの五数要約はmin=139、Q1=161、median=174.5、Q3=194、max=235秒、
p95=216.25秒、run所要合計=6,368秒。最初のrun開始から最後のrun終了までの
計測wall timeは6,434秒（1時間47分14秒）。costの五数要約は
min=$0.00086468、Q1=$0.00101360、median=$0.00122860、
Q3=$0.00147271、max=$0.00252714。合計$0.04820040で予算$5以内だった。
費用は全events正本の`provider_turn_duration` usageと`pricing.toml`から算出し、
ローカルOllamaは$0とした。

### 失敗の帰属

失敗は2/36で、同一停止クラスの連続はなかった。

| run | duration | cost | class | 帰属・状態 |
|---|---:|---:|---|---|
| mochimono_002 | 185秒 | $0.00252714 | `community_l2_verify_invocation_incomplete` | machine / QUEUED |
| mochimono_003 | 197秒 | $0.00178194 | `community_computed_unregistered` | schema gap (b), global集約 / QUEUED |

`mochimono_002`の計画にはprofile DATA-1の完全な字義例が渡っていた一方、
共有のdeterministic verification preferenceが短い対話形も配布していた。
plannerは後者をverifyへ採用し、bounded repair後も変更しなかった。停止原文:

```text
commandagent --offline --profile community-mini-app
error: stdin is not a TTY; pass --prompt or an action flag
failure_kind=verify_repair_progress_unchanged
```

`mochimono_003`は自然な持ち物件数をcollection/global参照で表そうとした。
v0.1は同一entity内のfield/computed DAGだけを許可し、global参照は契約上
QUEUEDである。生成spec原文と停止原文:

```yaml
- name: totalItems
  entity: packingItem
  expression: len(packingItem)
  type: number
```

```text
community_profile_violation:community_computed_unregistered:packingItem
```

前者は新規classとして登録した。後者はCM-2iで解消した局所computed連鎖とは
異なる未対応global集約として既存classを再観測し、解消済み範囲とQUEUED範囲を
`classes.toml`に分離記録した。どちらもCM-2kのpromotion gateを弱めず正直終端した。

### 成果物レベルとpromotion

全36件が`app.spec.yaml`だけを持つL2で、app-zone 0/36、L3/L4 0/36、
`promotion_decision` evidence 0/36だった。accepted L2は34/36、failed L2は2/36。
したがって引用可能なpromotion理由の実物は存在せず、理由分布は「昇格なし」36件。
存在しないevidenceを計画文で代用しない。正当なL3が塞がれないことはコミット1の
valid-promotion fixtureで、promotion gate後に従来のB系
`community_build_inputs_missing`まで進む対として確認した。

代表spec `observed/warikan_001.app.spec.yaml` の原文:

```yaml
entities:
  - name: expense
    fields:
      amount: number
      participants: list
      payer: string
views:
  - name: inputExpense
    entity: expense
  - name: settlementDisplay
    entity: expense
actions:
  - name: addExpense
    entity: expense
validations: []
computed:
  - name: shareAmount
    entity: expense
    expression: amount / len(participants)
    type: number
  - name: netBalance
    entity: expense
    expression: amount - shareAmount
    type: number
  - name: settlementAmount
    entity: expense
    expression: max(0, netBalance)
    type: number
permissions:
  - name: read
    subject: minIdentity
minIdentity:
  mode: anonymous
```

### Full meaningと証跡

- 最終local full suite: 2,083 passed / 33 ignored / 0 failed。
  `headless_summary::omitted_flag_preserves_stdout_bytes`、既存profileの
  adjudication/snapshot互換、generality guardrailを含む。
- L2 Full meaning: spec検証済み（schema/拘束/材料）・runtime smokeは
  プラットフォーム統合の被覆。runtime smoke実測済みとは表示しない。
- L3/L4 Full meaning: valid promotionとS+Z+Bを全件passし、bundleとmanaged
  runtime smokeを実測済み。本計測では該当0件。
- `summary.json`: events/metaから導出した36件のcurated mechanical record。
- raw uat-meta SHA-256: warikan
  `c826c449ddeed67a6c1934f5729745bea2b6491abae5fa74fc853f18f6bc57b1`、
  mochimono `296a270582f8dae629afcf8ca41580d4ba576b8c6f6b95a20a027aa6bad2f7e4`、
  vote `0f7a9e6836b8b63e856bc5c1e6cf18686d0e4286527c1019403b78813ae3a704`。
- predeclaration SHA-256: warikan
  `12c84e32441fced63e825b8c045870266bde3d81771acc8743b738ab0244c93e`、
  mochimono `fd082e33cc689e6e23e8bc930693c25d52c899378e939bebff5faaa16f7025bb`、
  vote `b0f4ab27ca70f6c6c6ec5d55b68a29afc5b43d0c0f11a6c110c4b763353a67eb`。
- representative spec SHA-256:
  `96ea47aa7d94e3dfde03d3b83d06f4390195aba632eef934e121d3e5f6c4bae5`。
- scrub: warikan 12/12、mochimono 12/12、vote 12/12 green。
