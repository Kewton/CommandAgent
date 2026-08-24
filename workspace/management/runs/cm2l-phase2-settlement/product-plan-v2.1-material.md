# Community Mini Apps企画書v2.1改訂材料

## Phase 0 — Builder Plane / headless

- exit 0/1の双方でstdout最終行に`commandagent.headless-summary/v1`の1行JSONを
  出し、`verdict`、`assurance`、成果物・acceptance sheet・events pathを
  process exitと分離して機械取得できた。
- 実弾3構成はlocal create=`full`（381.794秒）、Luna Responses/nativeの
  初回create=`partial`（14.194秒、`direct_cli_command_failed`）、保存recoveryの
  1周=`full`（84.42秒）。honest failureから既存の機械的続行で回収した。
- step-runnerはrunner分解後の実装が稼働しており、「配線中」ではない。
  OpenAI Responses/nativeも装備済みで、「XML閾値超過ならnative追加」は古い。
  text/XML parserと方言repairは明示text・互換fallbackとして残る。

企画書更新案: Phase 0は「Builder Plane headless成立、機械可読terminal summaryと
既存recoveryを実測済み」へ改訂する。

## Phase 1 — profile contract / adversarial validation

- profile契約はL2 `app.spec.yaml`を既定とし、L3/L4は`app-zone/`と有効な
  `promotion_decision`を必須化した。S（schema/閉語彙/computed）、Z
  （core不変/禁止API/依存）、B（L3/L4 bundle/smoke）の適用性を固定した。
- 封緘済み5類型（core編集、要求文inject、禁止API、未許可package、build時
  egress）×初回/修復再入の10ケースは、参照検証で10/10制約維持。
  製品headless経路でも初回5件は`failed/community_profile_violation`、修復5件は
  `full`で10/10を再確認した。
- 検知主張は「既知スイートの100%」であり、未知攻撃への網羅証明ではない。

企画書更新案: Phase 1は「契約・Rust verifier・参照実装一致・製品経路の既知
adversarial 10/10」を出荷済みとして記載する。

## Phase 2 — golden suite / GO

- 封緘済み3種×3変種×4 run=36 runをLuna Responses/native executorとOllama
  plannerで完走した。
- 一発full 80.6%（Wilson 95% [65.0%, 90.2%]）、修復込みfull 94.4%
  （[81.9%, 98.5%]）、p50 174.5秒、最大cost $0.00252714、total
  $0.04820040。事前宣言した4判定線を全て達成した。
- 成果物は36/36がL2、正当なL3/L4昇格は0件。したがってL2 Full meaningは
  schema/拘束/材料のspec検証済みであり、runtime smokeはプラットフォーム統合の
  被覆として明記する。
- 10系統の計測床を返済し、CI/acceptance両greenで清算した。

企画書更新案: Phase 2を**GO**とし、次期判断には次の2点だけをQUEUED入力とする。

1. L2 verify起動形を共有preferenceとprofile guidanceの間で一意化する。
2. collection/global集約をschema v0.2候補として裁定する。

この2点は既存の検証やpromotion gateを弱めず、Phase 2の観測済みfailure 2/36を
正直に保持する後続設計課題である。
