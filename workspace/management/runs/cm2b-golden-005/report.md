# 結果サマリ

## 事前宣言（2026-08-18T01:11:09+09:00、実行前固定）

- 分母: 3 suite × 3変種 × 4 run = 36 run。
- Go/No-Go判定線: 一発full（修復0）≥60%、修復込みfull ≥90%、所要p50 ≤180秒、1生成cost_usd ≤$0.067。
- 事前予測帯: 直接priorなし。最近接の間接実測はingest×Luna 100% (n=6)、Quiz 92%。点予測は置かず、実測率にWilson 95% CIを付す。
- 予算上限: $5。超過見込みで停止する。
- 計器pin: implementation HEAD `a48dee358d85f6f5d2eed6a18f8c9d9281de946f`、measurement execution revision `7827269f8432686b53ad8009b231a2bc366fe6d7`、campaign配置binary SHA-256 `d4144db43fef29cb0018537959a770a07a56b1405c383fa8ba208da6d60a3fc0`。built/installed SHAは支出前に一致した。
- provider: executor=`gpt-5.6-luna` / OpenAI Responses native、planner=`qwen3.6:27b-coding-nvfp4` / Ollama、configured host=`http://127.0.0.1:11434`。
- sealed source suite SHA-256: warikan `215abae70c63d72be8bec4ad683b92d68b68349cd7edbeea5f741ee636cd9e0c`、mochimono `1246b8773ea3229dc1c728e3713b9e44346bd3daac734323872c496b61f7a9a6`、vote `da5166d86873a5cc9f31a212ea9bef3dd2e373c055a611425dd0ce73905d08df`、manifest `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86`。計測cloneは計器pin用の`bon_series` 1行だけを派生追加し、goal/run bytesは封緘sourceと同一。

### 除外窓の全系譜

| 窓 | 除外理由 |
|---|---|
| golden-001 | schema供給床。pinned platform schemaがrun workspaceへ未供給。 |
| golden-002 | planner伝達床。community spec/verify正形がplannerへ届かずquality retryで停止。 |
| golden-003 | spec字義・verify依存床。誤ったentity.fields形と依存設営を要するverify。 |
| golden-004 | install配置床。sandbox外の既定binary配置先へ書き込めず0run停止。 |
| golden-004r | Ollama不達12件。provider計画前停止でtoken 0。 |
| cm2f-resume-001 | 閉語彙床3件。schema metadataをapp.spec rootへ複写したDATA-1 guidance gap。 |
| golden-005 instrument window | computed entry完全形の配布漏れ。warikan_001 failed、002環境中断。返済前計器窓として除外。 |

環境中断はrun非消費とし、重大な系統的床を観測した場合は無効データを量産せず途中停止する、と実行前に固定した。

## 実測結果と停止判定

支出前preflightはOllamaの`/api/tags`と宣言モデル名、OpenAI key存在と`/v1/models`を確認してpassした。その後warikanを2 run完了し、3件目を新規床確認後に環境中断扱いで停止した。完了2/36、環境中断1（run非消費）、未起動33。mochimono/voteは起動していない。

| 指標 | 実測 | Wilson 95% CI / 分布 | 閾値との差 | 判定 |
|---|---:|---:|---:|---|
| 一発full（修復0） | 0/2 = 0% | [0%, 65.8%] | -60.0ポイント | 未達・停止 |
| 修復込みfull | 0/2 = 0% | [0%, 65.8%] | -90.0ポイント | 未達・停止 |
| 所要p50 | 235.0秒 | p95=244.9秒 | +55.0秒 | 未達・停止 |
| 1生成cost_usd | 最大$0.00230994 | min=$0.00174142、median=$0.00202568 | $0.06469006下回る | 観測2件は達成 |

36 runを完走していないためPhase 2 GOは宣言しない。既に3判定線が未達で、新規の決定的setup床も観測したため裁定待ちで停止する。

### run別

| run | status | duration | cost_usd | primary stop class |
|---|---|---:|---:|---|
| warikan_001 | failed | 246秒 | $0.00174142 | `community_computed_unregistered` |
| warikan_002 | failed | 224秒 | $0.00230994 | `community_core_manifest_missing` |
| warikan_003 | `interrupted(environment)`（非消費） | 51秒 | $0 | `interrupted by user` |

完了run合計は470秒、campaign wall timeは523秒、費用合計は$0.00405136。除外済みinstrument windowの$0.00131134を含めても本作業のlive API費用は$0.00536270で、予算$5以内である。費用はeventsの`provider_turn_duration` usageと`pricing.toml`から機械算出し、Ollamaはローカルのため$0としている。

## 新規床の原文と帰属

### warikan_001: computed operand制約の未伝達

生成物は先行computed名を後続式で参照した。

```yaml
computed:
  - name: participantCount
    expression: len(participants)
    type: number
  - name: shareAmount
    expression: amount / len(participants)
    type: number
  - name: netBalance
    expression: amount - shareAmount
    type: number
  - name: settlementAmount
    expression: max(0, netBalance)
    type: number
```

events正本の停止原文:

```text
community_profile_violation:community_computed_unregistered:shareAmount
```

verifierは式の識別子をentity field/nameに限定し、computed nameの連鎖参照を登録しない。一方guidanceは登録関数とentry shapeを配布したが、このoperand制約を字義配布していない。従って新規DATA-1 machine床`community_computed_unregistered`として登録した。今回の返済範囲外なので挙動は変更していない。

### warikan_002: Z系core manifestの未供給

このspecはS系を通過した後、Z系で次の原文により停止した。

```text
community_profile_violation:community_core_manifest_missing
```

製品`verify_zone`はworkspaceの`.community/core.sha256sums`または`core.sha256sums`を必須とする。benchのempty community setupが支出前に注入するのは`schema/app-spec.schema.yaml`とpinだけで、core manifestは注入しない。従って生成物の内容によらず正しいspecも決定的に停止する新規machine setup床`community_core_manifest_missing`として登録した。推測補修は行っていない。

## 生成物とpromotion

生成された2件はいずれも`app.spec.yaml`のみで、`app-zone`はないため成果物形はL2 candidate 2/2 (100%)、L3 candidate 0/2 (0%)。ただしacceptance fullは0件なので、accepted L2/L3率はいずれも0%。`promotion_decision` evidenceは0件で、引用可能な実物はない（捏造しない）。

warikan代表specは`observed/warikan_002.app.spec.yaml`に原文を保存した。主要部分は次のとおり。

```yaml
entities:
  - name: Participant
    fields:
      name: string
  - name: Expense
    fields:
      description: string
      amount: number
      payer: string
      participants: list
views:
  - name: inputParticipants
    entity: Participant
  - name: inputPaidAmounts
    entity: Expense
actions:
  - name: recordPayment
    entity: Expense
  - name: calculateSettlement
    entity: Expense
validations:
  - name: positiveAmount
  - name: requiredDescription
computed:
  - name: total
    expression: amount
    type: number
  - name: balance
    expression: amount / len(participants)
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

- `uat-meta-warikan.json`: scrub green、SHA-256 `2a42928d8c692cb275bdf4353688f4a6011ad1806802e35ff2eed01c42568361`。
- predeclaration SHA-256: warikan `5938da137db0e76d062e5392862d4487904c67dce2941b190e99d923cf6e1e1c`、mochimono `53645709d5f401ecaa4289272f6216d00888b3720bb5b532028f9ee0b0309e97`、vote `9e88ee82a9288d4b983127eb3c811e8a8e3eb39298b26d47261dd688728259a1`。
- observed spec SHA-256: warikan_001 `52ec838917b3921ca588614829d226237754535259e1d585a2b454a8bda1a73d`、warikan_002 `ceadbfda373f7b14273517a378767868f77438f07b233c925dcc0ff872b381c6`。

