# 結果サマリ

## 事前宣言（2026-08-18T08:23:02+09:00、実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run。
- Go/No-Go判定線: 一発full（修復サイクル0）≥60%、修復込みfull ≥90%、所要p50 ≤180秒、1生成cost_usd ≤$0.067。
- 事前予測帯: 直接priorなし。最近接の間接実測はingest×Luna 100% (n=6)、Quiz 92%。点予測は置かず、実測率にWilson 95% CIを付す。
- 予算上限: $5。超過見込みで停止する。
- 計器pin: implementation HEAD `641911d284109160d30c5ece2fae74db8cca3b49`、measurement execution revision `9f4ba706923294588a8447cca12793ea8f0052c1`、campaign配置binary SHA-256 `cb35a866e1a95ba6ecd5c8ee769122c439e56a00cc8cdd8363cd04d2ba9309cf`。benchは支出前にbuilt/installed/executed binaryのSHA一致を検査する。
- provider: executor=`gpt-5.6-luna` / OpenAI Responses native、planner=`qwen3.6:27b-coding-nvfp4` / Ollama、configured host=`http://127.0.0.1:11434`。
- sealed source suite SHA-256: warikan `215abae70c63d72be8bec4ad683b92d68b68349cd7edbeea5f741ee636cd9e0c`、mochimono `1246b8773ea3229dc1c728e3713b9e44346bd3daac734323872c496b61f7a9a6`、vote `da5166d86873a5cc9f31a212ea9bef3dd2e373c055a611425dd0ce73905d08df`、manifest `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86`。計測cloneは計器pin用の`bon_series` 1行だけを派生追加し、goal/run bytesは封緘sourceと同一。
- schema改訂manifest SHA-256: `6242f3549c8b7eea08dd75067fd7e338e24659b76079d03c6ed5185fa58572c1`（CM-2i契約儀式内の更新）。敵対的fixture manifest SHA-256: `792c9696ca86127966810ec4a376a3815c4fb93de4ad2c9d6aa205dad09a2b0b`（不変）。

### 除外窓の全系譜（8系統）

| 系統 | 除外窓と理由 |
|---:|---|
| 1 | golden-001: schema供給床。pinned platform schemaがrun workspaceへ未供給。 |
| 2 | golden-001/002: cost配線床。usage/model pricingからsummaryへの転記がnull。 |
| 3 | golden-002: planner伝達床。community spec/verify正形がplannerへ届かず停止。 |
| 4 | golden-003/004: spec字義・verify依存床。誤ったfields形と依存設営を要するverify。 |
| 5 | golden-004: install配置床。sandbox外の既定binary配置先へ書き込めず0run停止。 |
| 6 | golden-004r: Ollama不達12件。provider計画前停止でtoken 0。 |
| 7 | cm2f-resume-001: 閉語彙床3件。schema由来の完全語彙未配布。 |
| 8 | golden-005: computed連鎖とcore manifest供給床。computed同士の参照拒否およびZ系manifest未供給。 |

環境中断はrun非消費とする。同一系統の重大な乖離が連続した場合は、無効データを量産せず途中停止する。4判定線を全て達成した場合のみPhase 2 GOを宣言し、一部未達なら乖離幅を裁定材料として停止する。

## 実測結果

支出前preflightはOllamaの`/api/tags`と宣言モデル名、OpenAI key存在と`/v1/models`を確認してpassした。warikanを2 run完了し、同一の新規B系setup床を連続観測したため、3件目を環境中断扱いで停止した。完了2/36、環境中断1（run非消費）、未起動33。mochimono/voteは起動していない。

| 指標 | 実測 | Wilson 95% CI / 分布 | 閾値との差 | 判定 |
|---|---:|---:|---:|---|
| 一発full（修復0） | 0/2 = 0% | [0%, 65.8%] | -60.0ポイント | 未達・停止 |
| 修復込みfull | 0/2 = 0% | [0%, 65.8%] | -90.0ポイント | 未達・停止 |
| 所要p50 | 224.5秒 | p95=244.8秒 | +44.5秒 | 未達・停止 |
| 1生成cost_usd | 最大$0.00231666 | min=$0.00168934、median=$0.00200300 | $0.06468334下回る | 観測2件は達成 |

36 runを完走していないためPhase 2 GOは宣言しない。一発full・修復込みfull・p50の3判定線が未達で、同一の決定的setup床が2件連続したため裁定待ちで停止する。

### run別

| run | status | duration | cost_usd | primary stop class |
|---|---|---:|---:|---|
| warikan_001 | failed | 247秒 | $0.00231666 | `community_build_inputs_missing` |
| warikan_002 | failed | 202秒 | $0.00168934 | `community_build_inputs_missing` |
| warikan_003 | `interrupted(environment)`（非消費） | 23秒 | $0 | `interrupted by user` |

完了run合計は449秒、campaign wall timeは472秒、費用合計は$0.00400600。予算$5以内である。費用は各workspaceのevents正本にある`provider_turn_duration` usageと`pricing.toml`から機械算出し、Ollamaはローカルのため$0としている。

## 新規床の原文と帰属

両runともpinned schema v0.1とcore manifestがprocurementで注入され、生成specはcomputed連鎖を含めS系・Z系を通過した。その後、計画内のschema検証コマンドが製品verification全体を実行し、L2段階でB系入力を要求した。

events正本の2件共通停止原文:

```text
community_profile_violation:community_build_inputs_missing
```

製品のB系は`src/app-zone/`または`app-zone/`の`index.html`と`app.ts`を必須とする。一方、L1-L2既定の計画1段階目は正しく`app.spec.yaml`だけを生成してschema verifyを行うため、B系入力はまだ存在しない。生成内容によらず自己完結schema verifyが決定的に停止するmachine setup床として`classes.toml`へ登録した。これはCM-2iの指定返済範囲外なので、verificationの分離・延期・fixture追加は行っていない。

## 生成物とpromotion

生成された2件はいずれも`app.spec.yaml`のみで、成果物形はL2 candidate 2/2 (100%)、L3 candidate 0/2 (0%)。acceptance fullは0件なのでaccepted L2/L3率はいずれも0%。停止が第1phase内だったため`promotion_decision` evidenceは0件で、要求された2-3件の実物引用は存在しない（捏造しない）。

warikan代表spec `observed/warikan_001.app.spec.yaml` の原文:

```yaml
entities:
  - name: expense
    fields:
      description: string
      payer: string
      amount: number
      participants: list
views:
  - name: listExpenses
    entity: expense
  - name: viewExpense
    entity: expense
actions:
  - name: addExpense
    entity: expense
  - name: deleteExpense
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
  - name: write
    subject: minIdentity
minIdentity:
  mode: anonymous
```

## 証跡

- `uat-meta-warikan.json`: curated mechanical record、scrub green、SHA-256 `4fcbc027bb59b3127b2ceebbe08a780054f54da564ade65c8c0c59ef1e94062d`。
- live campaign raw metadata scrub: green。campaign tree全体のscrubは配置binaryが17,939,888 bytesのoversize規則に該当したため、raw treeやbinaryは収載せず、scrub greenのmetadata/specだけを固定した。
- predeclaration SHA-256: warikan `2602f62750b2c5a6757278de162f1d3d92989520eb3648a026b646cc0c8b69ed`、mochimono `86a9ce1c0abca3d9410d9e36a5918518f7d180cf5e17a10676eaf537e26908b0`、vote `b7e98367f76db573b9575551b52dcec10e5d229ad7decb1ce34596d59afc939b`。
- observed spec SHA-256: warikan_001 `8b032acfa9f3815f5228ad997359ab5d51c8ddb712d09072b78208d3069ff62a`、warikan_002 `a301db223ab98c676fb96760405ff29f94f96779be12871e6cf470d18575fc54`。
