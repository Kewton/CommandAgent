# F-1a Score Institution

Status: **draft for review**

本稿は v2.5 roadmap の F-1 を、E-2a と同じ
**draft → review adjudication → implementation** の順で制度化する設計稿である。
この段階では `eval.yaml` を読み込む製品経路、verdict、assurance、admission、
event schema、既存 band の値を変更しない。実装と全 run 遡及走査はレビュー裁定後に
別タスクとして行う。

## 1. 目的と非目的

スコアの目的は、full / non-full の二値だけでは失われる「どの検証原子まで到達し、
どれを実証したか」を粗い同一尺度で表示し、次の測定予算を配分できるようにすること
である。スコアは既存の裁定結果を集計する表示・配分制度であり、新しい裁定器ではない。

固定する不変条件は次のとおり。

1. score は既存の登録済み検証原子と、その既存 evidence 状態だけを読む。
2. score は `verdict`、`assurance`、`earned`、admission、release gate を変更しない。
3. 同じ原子について `pass > absent > violation` を必ず満たす。
4. 実行されていない検証、成果物の裸の存在、自由文の印象評価から点を作らない。
5. checkpoint は既存 event / evidence の時刻つき観測だけから再計算し、新しい judge を
   呼ばない。
6. 初期の等配点は仮説ではなく固定した粗い物差しである。遡及スタディを配点探索に
   使わない。

## 2. 三層の宣言境界

F-1 は次の三層を一方向に接続する。下流から上流への昇格、特に score から earned への
逆流を禁止する。

```text
Layer 1: registered atom vocabulary
  typed Rust/Python ID + profile/intent manifest + parameter schema
        ↓ resolve; unknown is a schema error
Layer 2: eval.yaml score declaration
  usage + positive weights + registered parameters only
        ↓ join by canonical atom ID
Layer 3: observed vector and projection
  timestamped event/evidence states -> checkpoint vector -> report/allocation
```

### 2.1 Layer 1 — 登録済み検証原子

原子IDは `eval.yaml` の自由文字列を正本にしない。E-5a が terminal / violation IDで
採用した方式と同じく、typed producer 語彙、profile / intent manifest の check ID、
および両方向の整合 guard から機械導出する。原子には以下が必要である。

- stable ID と kind
- observation を発行する既存 event / evidence producer
- `pass` / `absent` / `violation` / `unobserved` への既存の型付き写像
- parameter を持つ場合は、名前、型、必須性、許容値を閉じた schema
- verdict / assurance で既に使われる contract binding

score loader はこの registry snapshot を読み、未知ID、producerのないID、contractに
束縛されないIDを拒否する。`eval.yaml` が新しい原子を登録することはできない。

### 2.2 Layer 2 — `eval.yaml` の配点宣言

`eval.yaml` は「どの登録済み原子を何対何で表示するか」だけを宣言する。照合器、
status の数値、正規化式、penalty、cap、自由式を宣言できない。

### 2.3 Layer 3 — 観測ベクトル

集計器は registry が認めた producer の保存済み event / evidence のみを atom observation
へ写像する。出力は score と同時に atom vector、coverage、source reference を保持する。
score だけを保存して原証拠を隠すことはできない。

## 3. `eval.yaml.score` schema v0

次が v0 の全文である。YAML表現だが、`additionalProperties: false` 相当を全 object に
適用する。

```yaml
score:
  schema_version: commandagent.eval.score/v0
  usage:
    - report
    - allocation
  weights:
    - atom:
        id: cli_probe
        params: {}
      points: 1
    - atom:
        id: help_binding
        params: {}
      points: 1
    - atom:
        id: cli_output_claims
        params:
          anchor: cli.readme.observed_stdout
      points: 1
    - atom:
        id: cli_rerun_consistency
        params: {}
      points: 1
```

構造制約は次のとおり。

| path | type / cardinality | 制約 |
|---|---|---|
| `score` | object, required when scoring is used | 未知fieldを拒否 |
| `score.schema_version` | string | `commandagent.eval.score/v0` のみ |
| `score.usage` | array, 1..2 | unique。要素は `report` または `allocation` のみ |
| `score.weights` | array, 1..256 | canonical atom key は重複不可 |
| `weights[].atom` | object | field は `id`, `params` のみ |
| `weights[].atom.id` | string | Layer 1 registry に実在し、対象profile / intentへ束縛済み |
| `weights[].atom.params` | object | registry が宣言したparameter schemaと完全一致。未知・欠落fieldを拒否 |
| `weights[].points` | integer, 1..1000 | 正数のみ。負配点、0、浮動小数、式を拒否 |

weight合計を100に合わせる必要はない。表示値は固定式で正規化するため、`1/1/1/1`
と`25/25/25/25`は同じ結果になる。weightは相対比だけを表す。

`usage` の閉じた語彙に `adoption`、`earned`、`admission`、`gate` が存在しないことが
用途分離のschema guardである。未知fieldも拒否するため、別名で裁定を持ち込めない。

### 3.1 パラメータ化原子

v0 が許す parameter family は次の3種だけである。各 `id` と parameter signature は
Layer 1 registry が所有し、`eval.yaml` は値を束縛するだけである。

| family | parameter | 機械的還元先 |
|---|---|---|
| `execution_binding(binding)` | 登録済みprobe / command binding ID | `executed=true` と保存済み outcome |
| `schema_conformance(schema)` | registry登録済みschema IDとexact-byte hash | 保存成果物に対する既存schema照合event |
| `claims_binding(anchor)` | registry登録済みtyped anchor ID | 主張、実行観測、schemaの既存binding evidence |

生のshell、正規表現、任意path、自然言語rubricをparameterへ入れられない。例えば
`claims_binding(anchor="READMEを良い感じに評価")` は typed anchor ID でないため拒否する。
parameterのcanonical keyは registry が field 順と型を正規化して生成し、YAML key順に
依存しない。

既存の非parameter checkも `params: {}` を明示する。裸の成果物存在checkをscore側で
新設することは禁止する。ただし既存contractに登録され、verdictへ既に束縛された
存在原子を参照することまでは禁止しない。

### 3.2 禁止形とschema負例

以下はすべて load 前にschema errorとなる。

```yaml
# NG: 存在しただけで加点する未登録原子
score:
  schema_version: commandagent.eval.score/v0
  usage: [report]
  weights:
    - atom: {id: file_exists, params: {path: README.md}}
      points: 10
```

```yaml
# NG: 自由文judge / rubric
score:
  schema_version: commandagent.eval.score/v0
  usage: [report]
  weights:
    - judge: "READMEが十分よければ10点"
      points: 10
```

```yaml
# NG: scoreによる採用・earned介入
score:
  schema_version: commandagent.eval.score/v0
  usage: [report, adoption]
  adoption: {minimum_score: 80}
  weights: []
```

```yaml
# NG: status値や式を宣言して誠実性順序を逆転する
score:
  schema_version: commandagent.eval.score/v0
  usage: [report]
  state_values: {pass: 1, absent: 2, violation: 0}
  formula: "sum(weights)"
  weights: []
```

## 4. 得点式と誠実性フロア

原子 `i` の正のweightを `w_i`、checkpoint `t` の状態係数を `s_i(t)` とする。

| state | `s_i(t)` | 意味 |
|---|---:|---|
| `pass` | `+1` | 既存judgeが実行証拠をpassとした |
| `absent` | `0` | 対象主張等が存在せず、違反はしていない |
| `violation` | `-1` | 既存judgeが不一致・違反・実行済みfailureを確定した |
| `unobserved` | `0` | checkpointまでに時刻つき観測がない。順序比較の対象外 |

```text
score(t) = 100 * Σ(w_i * s_i(t)) / Σ(w_i)
```

scale は `[-100, 100]`、丸めは表示時だけ小数1桁のround-half-evenとする。保存JSONは
分子 `weighted_state_sum` と分母 `weight_sum` を整数で保持し、再計算可能にする。
原子を一つも観測していない時点は `score=null, reached=false` とし、0点runへ混ぜない。

この固定式は、同じ他原子ベクトルに対して必ず
`pass > absent > violation` を満たす。violationをabsentへ変えると `w_i`、passへ
変えると `2*w_i` だけ分子が増える。正weight以外をschema拒否するため逆転できない。

「violation必負」は二重に守る。

1. violationは当該原子で必ず負点となり、absentと同点にならない。
2. contract上violationでfailedとなったrunは、合計scoreが正でもfailedのままである。
   scoreにverdictを上書きする経路を持たない。

`unobserved` と `absent` は数値寄与が同じでも意味が異なるため、atom vectorと
`observed_weight / total_weight` を必ず併記する。到達率をscoreで代用しない。

### 4.1 逆転不可guard

実装時にはschema testに加えて、registryの全原子・許容weight端点に対して次の
property guardを置く。

- 任意の固定された他原子ベクトルで `S(pass) > S(absent) > S(violation)`
- 原子順、YAML key順、event同時刻の入力順を正準化した結果が同一
- unknown / producerなし / profile非束縛原子はload失敗
- score出力前後で terminal event、verdict、assurance、admission bytesが同一
- `usage=allocation` を加えても run のearned結果とscore自体が不変

## 5. 用途分離

`usage=report` はband、run sheet、比較表への表示だけを許す。
`usage=allocation` は次キャンペーンのrun数、model / family層別、再測定優先順位の
入力にだけ使える。allocationはすでに記録されたrunを採用・棄却したり、contract
denominatorから除外したりできない。

禁止するデータフローは次である。

```text
score -> verdict / assurance / earned / admission / release gate   # forbidden
score -> historical run inclusion or exclusion                      # forbidden
score -> report and future measurement allocation                   # allowed
```

score consumerは `usage` を検証し、用途不一致ならfail closedする。consumerがscoreから
独自の「合格」を作ることも禁止する。

## 6. checkpoint計算可能性

任意時点 `t` のベクトルは、runのimmutable event streamと参照evidenceから再集計する。
mtime、現在時刻、レポート本文の再解釈、LLM judgeは使わない。

1. run開始時にregistry snapshotとscore declarationのhashを固定する。
2. event streamを保存順で読み、registry producerに一致するeventだけを候補にする。
3. `epoch` または `evidence_envelope.epoch` を持ち、source hashが一致する観測だけを採る。
4. 同一atomの `epoch <= t` にある最新の有効観測を選ぶ。同epochはimmutable streamの
   ordinalで決定する。
5. stale lineage、未実行、schema不一致を既存judgeの規則どおり除外し、推測で補わない。
6. atom state、coverage、整数分子・分母、source refsを出力する。

時刻のないhistorical eventはcheckpointへ時刻を捏造しない。final summaryに既存の
typed状態があればfinal-only vectorへ利用できるが、checkpoint studyでは
`timestamp_unavailable` としてcoverageから除外する。ファイル行順をwall-clockへ
読み替えない。

標準出力形は次である。

```json
{
  "run_id": "stats_luna_002",
  "checkpoint_epoch": 1785664000,
  "reached": true,
  "score": 75.0,
  "weighted_state_sum": 3,
  "weight_sum": 4,
  "observed_weight": 4,
  "atoms": [
    {"key": "cli_probe", "state": "pass", "source_ref": "events.jsonl:241"},
    {"key": "help_binding", "state": "pass", "source_ref": "events.jsonl:242"},
    {"key": "cli_output_claims(anchor=cli.readme.observed_stdout)", "state": "absent", "source_ref": "events.jsonl:243"},
    {"key": "cli_rerun_consistency", "state": "pass", "source_ref": "events.jsonl:244"}
  ]
}
```

このJSONは形の例であり、記載epochやlineを既存runの事実として採用しない。

## 7. 遡及妥当性スタディ

裁定後に既存全runを一度だけ同じ固定specで走査し、次を出力する。

- run / campaign inventoryと除外理由
- atom transitionごとのcheckpoint vector
- final vector、final verdict、assurance、full/non-full
- profile、intent、model、family、protocol、directive_roundの層別
- checkpoint scoreと最終verdictのSpearman順位相関
- final scoreのfull / non-full分布、重なり、到達coverage
- violation / absent / unobservedを分離した誠実性監査

目的はweightを選ぶことではなく、**等配点という粗い得点で十分か**を検証することで
ある。したがって以下を禁止する。

- 同じhistorical runに対するweight grid search、回帰学習、閾値探索
- 相関が最大になるatomの事後除外
- profile / model別のweight調整
- score閾値からfullまたはadmissionを再定義すること

最初の本走査では各contractのrequired atomを等配点に固定する。結果が粗すぎる場合は
その事実を報告し、同じデータで重みを調整せず、制度draftへ差し戻す。重み変更を
裁定する場合は新しいschema versionと将来runの事前登録が必要である。

## 8. T2F scripted directive suite

`directive_round` はD-3d v1.1で実装済みの測定構成軸をそのまま使う。scripted suiteは
人間directiveとは別armとし、次のstrict形を `eval.yaml` の隣接節として宣言する。

```yaml
directive_suite:
  schema_version: commandagent.eval.directive-suite/v0
  target_atom:
    id: cli_output_claims
    params: {anchor: cli.readme.observed_stdout}
  max_rounds: 3
  rounds:
    - round: 1
      script: eval/directives/cli-c3/round-1.txt
      sha256: "<64 lowercase hex>"
    - round: 2
      script: eval/directives/cli-c3/round-2.txt
      sha256: "<64 lowercase hex>"
    - round: 3
      script: eval/directives/cli-c3/round-3.txt
      sha256: "<64 lowercase hex>"
```

- `rounds` は1から連続し、`max_rounds`と一致する。上限超過runを起動しない。
- scriptはUTF-8 exact bytesをsha256でpinし、実行前後で一致を検証する。
- 1 lineage、同じworkspace family、model、profile、intent、pack pinを維持する。
- 各roundは独立したband構成であり、集約時に潰さない。
- directiveはguidanceであり、target atomやfullを直接満たさない。
- normal acceptanceが再実行され、target atom passかつfullを得た最初のroundをT2Fとする。

観測表記は次で固定する。

| shape | notation | 条件 |
|---|---|---|
| 成功観測 | `T2F = r` | risk setへ入り、round `r`でtarget passかつfull |
| 右打ち切り | `T2F > R` | target violationを観測してclock開始後、上限`R`までfullなし |
| clock未開始 | `T2F = NA @ R (target_not_observed)` | round `R`までtarget violation自体が未観測 |

D-3d human round 1–2はC3未到達だったため、現時点の正準表記は
`T2F = NA @ 2 (target_not_observed)` である。`T2F > 2`とは書かない。

## 9. 二軸band表示案

bandに既存のfull、到達、C3分布を残したまま、次の2列を加える。

| configuration | reached | score at reach distribution | T2F |
|---|---:|---|---|
| model / family / protocol / directive round | `n/N` | `n; min / p25 / median / p75 / max; violation n` | `r`, `>R`, or `NA @ R` |

score分布の分母は `reached=true` のrunだけとし、未到達を0点へ混ぜない。band全体には
必ず `reached n/N` を隣接表示する。T2Fはscripted directive armだけに値を持ち、
通常armへ0を埋めない。

## 10. Gemma × Luna 事前検算

F-2a-8の全窓対照表を第一検算材料にする。ここではCLI C1〜C4を等配点
`1/1/1/1` とし、§4の固定式を既存の最終C状態へ手計算した。これは本走査ではなく、
公理が実測の直観差を壊さないかを見るdraft内の事前検算である。

| arm | full | reached | reached score distribution | C3 integrity |
|---|---:|---:|---|---|
| Gemma formal Window B | 0/6 | 2/6 | `[50, 50]` | pass 0 / absent 0 / violation 2 |
| Luna 006 Responses/native | 0/6 | 5/6 | `[-25, 50, 50, 50, 75]` | pass 2 / absent 2 / violation 1 |
| Luna 007 Responses/native | 1/6 | 2/6 | `[50, 100]` | pass 2 / absent 0 / violation 0 |
| Luna 008 Responses/native | 1/6 | 3/6 | `[25, 50, 100]` | pass 2 / absent 1 / violation 0 |

計算例は次のとおり。

- `pass/pass/absent/pass = 75`: fullではないが3/4項を実証し、違反はない。
- `pass/pass/violation/pass = 50`: 同じ3 passでもviolationが25点を失わせる。
- `fail/fail/absent/pass = -25`: 到達したが、未実証を成功へ見せない。
- `pass/pass/pass/pass = 100`: 既存contractがfullを与えたrunだけの完全ベクトル。

したがってfull列だけではともに0となるGemmaとLuna 006に対し、粗い等配点でも
Luna 006の「最大75、C3非違反4/5、C3 pass 2」とGemmaの「最大50、C3 pass 0、
到達2/2がviolation」を分離できる。一方でGemmaのC1/C2/C4 passを0へ消さず50として
残すため、単なる勝敗ラベルでもない。007/008の100は既存fullと一致し、scoreが
fullを新造していない。

この事前検算は等配点案を採択する証明ではない。全profile / intentの遡及coverageと
checkpoint相関が得られるまで、制度はdraftのままである。

## 11. 実装裁定で決める事項

レビューは少なくとも次を裁定する。

1. score schema v0のfield集合と3 parameter familyを固定するか。
2. signed scale `[-100, 100]` と等配点を最初の遡及specに採用するか。
3. timestampのないhistorical atomをfinal-onlyへ限定するか。
4. 相関出力の層別・最小sample表示規則を固定するか。
5. scripted directiveの最大roundとsuite script本文を別commitで事前登録するか。
6. bandの分布要約を五数要約にするか、固定bucketにするか。

裁定まではscore loader、runtime event、band列、directive suite dispatchを実装しない。
次コミットの `scripts/score_retrospective.py` も入力inventoryを示すdry-run骨格に限定し、
historical event走査と相関計算を無効のまま保つ。
