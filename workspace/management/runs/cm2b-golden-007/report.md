# 結果サマリ

## 事前宣言（2026-08-18T09:19:07+09:00、実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run。
- Go/No-Go判定線: 一発full（修復サイクル0）≥60%、修復込みfull ≥90%、所要p50 ≤180秒、1生成cost_usd ≤$0.067。
- 事前予測帯: 直接priorなし。最近接の間接実測はingest×Luna 100% (n=6)、Quiz 92%。点予測は置かず、実測率にWilson 95% CIを付す。
- 予算上限: $5。超過見込みで停止する。
- 計器pin: implementation HEAD `d2ddcb6bf1663a257ea018dbc6e429ba6c134f75`、measurement execution revision `96c78ce9e1f4dcffb8109ac4ed03c7d2c52c917e`、campaign配置binary SHA-256 `bbdeb6b66fefaf8a36c809ad5946e11066b974ef22af3d8104e836d82a7371b0`。benchは支出前にbuilt/installed/executed binaryのSHA一致を検査する。
- provider: executor=`gpt-5.6-luna` / OpenAI Responses native、planner=`qwen3.6:27b-coding-nvfp4` / Ollama、configured host=`http://127.0.0.1:11434`。
- sealed source suite SHA-256: warikan `215abae70c63d72be8bec4ad683b92d68b68349cd7edbeea5f741ee636cd9e0c`、mochimono `1246b8773ea3229dc1c728e3713b9e44346bd3daac734323872c496b61f7a9a6`、vote `da5166d86873a5cc9f31a212ea9bef3dd2e373c055a611425dd0ce73905d08df`、manifest `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86`。計測cloneは計器pin用の`bon_series` 1行だけを派生追加し、goal/run bytesは封緘sourceと同一。
- schema manifest SHA-256: `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1`。敵対的fixture manifest SHA-256: `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b`。CM-2jの契約改訂儀式では封緘fixture/schema/suiteを変更しない。

### 除外窓の全系譜（9系統）

| 系統 | 除外窓と理由 |
|---:|---|
| 1 | golden-001: schema供給床。pinned platform schemaがrun workspaceへ未供給。 |
| 2 | golden-001/002: cost配線床。usage/model pricingからsummaryへの転記がnull。 |
| 3 | golden-002: planner伝達床。community spec/verify正形がplannerへ届かず停止。 |
| 4 | golden-003/004: spec字義・verify依存床。誤ったfields形と依存設営を要するverify。 |
| 5 | golden-004: install配置床。sandbox外の既定binary配置先へ書き込めず0run停止。 |
| 6 | golden-004r: Ollama不達12件。provider計画前停止でtoken 0。 |
| 7 | cm2f-resume-001: 閉語彙床3件。schema由来の完全語彙未配布。 |
| 8 | golden-005/006: computed連鎖・core manifest供給・B系入力誤適用。computed連鎖とcore manifestを返済後、L2へB系を誤適用して停止。 |
| 9 | golden-007: 検証適用性。L2はS/Z/材料検査でFullを判定しBは非適用、L3/L4は従来どおりS+Z+B必須へ契約改訂。 |

L2でB系誤適用に伴う修復試行が消えるためp50改善を見込むが、この見込みは事前予測・分母・判定には使用しない。環境中断はrun非消費とする。同一系統の重大な乖離が連続した場合は無効データを量産せず途中停止する。4判定線を全て達成した場合のみPhase 2 GOを宣言し、一部未達なら乖離幅を裁定材料として停止する。

## 実測結果

支出前preflightはexecution revision、built/installed/executed binaryの
SHA-256一致、Ollamaの`/api/tags`と宣言モデル名、OpenAI key存在と
`/v1/models`を確認してpassした。warikanを2 run完了し、両件とも
L2検証をpassした後に不正なL3生成・verifyへ進む同一系統の重大乖離を
観測したため、3件目を環境中断扱いで停止した。完了2/36、環境中断1
（run非消費）、未起動33。mochimono/voteは起動していない。

| 指標 | 実測 | Wilson 95% CI / 分布 | 閾値との差 | 判定 |
|---|---:|---:|---:|---|
| 一発full（修復0） | 0/2 = 0% | [0%, 65.8%] | -60.0ポイント | 未達・停止 |
| 修復込みfull | 0/2 = 0% | [0%, 65.8%] | -90.0ポイント | 未達・停止 |
| 所要p50 | 612.5秒 | p95=814.55秒 | +432.5秒 | 未達・停止 |
| 1生成cost_usd | 最大$0.00518952 | min=$0.00415078、median=$0.00467015 | $0.06181048下回る | 観測2件は達成 |

36 runを完走していないためPhase 2 GOは宣言しない。一発full、修復込み
full、p50の3判定線が未達である。L2誤適用は2/2で解消した一方、両件が
L3へ昇格して契約外の生成形・verifyを出したため、裁定材料として停止する。

### run別と分布

| run | status | duration | cost_usd | terminal class |
|---|---|---:|---:|---|
| warikan_001 | failed | 388秒 | $0.00415078 | `community_build_inputs_missing`（L3文脈で正しく発火） |
| warikan_002 | failed | 837秒 | $0.00518952 | `community_l3_verify_command_timeout` |
| warikan_003 | `interrupted(environment)`（非消費） | 6秒 | $0 | `interrupted by user` |

完了runの五数要約はmin=388、Q1=500.25、median=612.5、Q3=724.75、
max=837秒、p95=814.55秒。完了run合計は1,225秒、campaign wall timeは
1,231秒、費用合計は$0.00934030で予算$5以内。費用は各workspace配下の
全events正本にある`provider_turn_duration` usageと`pricing.toml`から
機械算出し、ローカルOllamaは$0としている。タスク種別はwarikan完了2、
mochimono完了0、vote完了0。

## 系統的乖離の原文と帰属

両runの第1段階では`app.spec.yaml`だけが存在するL2 workspaceに対して
製品内蔵verifyがexit 0となり、profile checkまでpassした。この2/2は
`community_build_inputs_missing`がL2で発生しないlive証拠である。その後、
plannerはgoalをL2で満たせるかを確定せず、両件とも`src/app-zone/`を追加した。

warikan_001は`src/app-zone/app.js`だけを生成し、L3で必須の`index.html`、
`app.ts`、bundle/smoke材料を揃えなかった。停止原文:

```text
community_profile_violation:community_build_inputs_missing
```

warikan_002は`index.html`と`app.js`を生成したが、verifyを次の非対話action
なしの起動形として生成した。

```text
commandagent --offline --profile community-mini-app
```

停止原文:

```text
verify_command_timeout:commandagent --offline --profile community-mini-app: the verify command hangs - replace it with a bounded check
```

2件のterminal classは異なるが、共通する上流形は「L2 pass後にL3へ昇格し、
契約のL3成果物形と自己完結verifyを満たさない」である。CM-2jの契約改訂は
L3のB系を緩和せず、両件を正直に拒否した。新しいtimeout原文は
`classes.toml`へ登録し、planner/L3生成契約床として未解消・裁定待ちとした。

## 生成物とpromotion

完了2件はL2 phaseを2/2 (100%) passした後、最終成果物に`src/app-zone/`が
存在するためL3 candidate 2/2 (100%)、L2 final candidate 0/2 (0%)。
acceptance fullは0件なのでaccepted L2/L3率はいずれも0%。両件ともL3
verify前後で停止し、`promotion_decision` evidenceは0件である。したがって
promotion理由の実物2-3件は存在せず、理由分布は「機械記録なし」2/2。
計画文の昇格記述はevidence正本ではないため代用しない。

warikan代表spec `observed/warikan_001.app.spec.yaml` の原文:

```yaml
entities:
  - name: expense
    fields:
      description: string
      amount: number
      participants: list
      payer: string
views:
  - name: expenses
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

## Full meaningと証跡

- L2 Full meaning: spec検証済み（schema/拘束/材料）・runtime smokeは
  プラットフォーム統合の被覆。runtime smoke実測済みとは表示しない。
- L3/L4 Full meaning: S+Z+Bを全件passし、bundleとmanaged runtime smokeを
  実測済み。本計測では0件。
- `uat-meta-warikan.json`: curated mechanical record。raw metadata/spec scrub
  green。raw uat-meta SHA-256 `dc90fef992783154327803fa53c7023bfa5ce62606b1f291d952da6fc89d005c`。
- predeclaration SHA-256: warikan
  `c03a7dcd94c2cdecb3b1eb08a50504a180cb400594f9ac32a8b0f8200d3bca61`、
  mochimono `42669be8d9ec9c9aacddb855a91cc2eeb9732db9a94ddd91d60421efb090ef4c`、
  vote `e96eae2a49a5ea29e5a36906903ad0db2d99ca1e85fa8b253266c52813c6516c`。
- observed spec SHA-256: warikan_001
  `23a0ade0206709858ff0113d29e7dbf88eb206df9f0abbb2bddc84274f3a5398`、
  warikan_002 `5e34645b39611f94caa4c486a69dd89949f4665585a6275201b1a4a2803ee179`。
