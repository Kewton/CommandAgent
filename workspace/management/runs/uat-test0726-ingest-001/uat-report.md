# uat-test0726-ingest-001: ingest×create初計測

実施日: 2026-07-27 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `ad95e98a4d325e6d17be2d28c810a7036af55279`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-bはPASS。P1-aはfinal acceptance到達runが
0件のためNOT MEASURED。製品結果はfailed 6/6、full相当0/6だった。**

正式campaignは6/6を`run_stop`とproduct exit 1で正直終端させた。
全runはfinal acceptance前に停止し、契約§4どおり
`failed / static (ingest_probe_not_run)`だった。N1〜N5 evidenceは0件で、
未実行を`claims_absent`、partial、fullとして扱ったrunはない。
CLIで起きた未実行×partial投影事件の再発は0件だった。

admissionは`off`のまま。今回はfull相当へ到達したrunがないため、
full相当を共通gateが`static / profile_not_admitted`へ制限する挙動は
**未計測**である。観測されたstaticはadmission capではなく、
N1未実行を表す`ingest_probe_not_run`だった。

benchのsource調達は6/6でsha256一致、zero-exit precheck一致。
検収シートは6/6生成、scrubは6/6とcampaign全体でgreen。
環境中断、新規run再実行、人手terminal切替はいずれも0件だった。

## 1. 入力資産とsuite

### 1.1 決定的HTML

| family | asset | 候補 | 完全 | 不備 | sha256 |
|---|---|---:|---:|---:|---|
| list | `events-list.html` | 10 | 8 | 2 | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | 10 | 8 | 2 | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

list族は`<article>` 10件、table族は`<tr>` 10件を機械計数できる。
各族の不備は日付欠落1件と`会場未定`1件である。日付表記の実弾は、
list族に`2026年8月1日`と`8/3(月)`、table族に
`令和8年8月2日`を含めた。乱数・現在時刻・外部入力はない。

source setは族別rootから`copy = ["data"]`で
`data/snapshots/events-{list,table}.html`へ配置した。
`input_sha256_expected == input_sha256_observed`は6/6。

### 1.2 precheck

bench v0.3は既に`precheck_expect = "zero_exit"`を受理・実行できたため、
v0.3.1のPython変更は不要だった。

list族の実測原文:

```json
{
  "exit_code": 0,
  "stdout_tail": "snapshot-ok:list\n",
  "expectation": "zero_exit",
  "exit_matches": true,
  "pattern": "snapshot-ok:list",
  "pattern_matches": true
}
```

table族も`exit_code=0`、`snapshot-ok:table`一致。precheck成功期待は6/6。

### 1.3 実効構成

- suite: `ingest-create`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: qwen35 4本、gemma31 2本
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_001`
- campaign: `ingest-create-20260727-065810`

6枚の`run_start`と自動検収シートでprofile、planner、executor/providerを
照合し、実効モデル一致は6/6だった。

## 2. preflight

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `ad95e98 Measure ingest create UAT` |
| minimum ancestor | `96ac8e1` verified |
| full `cargo test` | 1818 passed / 0 failed / 30 ignored |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 ad95e98 2026-07-27T07:00:46Z` |
| `NODE_ENV` | `production` |

## 3. Run行列

`—`はfinal acceptance未到達によるN未実行を表す。

| run | family / executor | verdict | assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_qwen35_001` | list / qwen35 | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `artifact_follow_through_exhausted` / model | 559 |
| `list_gemma31_001` | list / gemma31 | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | smoke check TypeError後の`model_stagnation` / model | 1448 |
| `list_qwen35_002` | list / qwen35 | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | wrong verify filename後の`artifact_recovery_exhausted` / model | 984 |
| `table_qwen35_001` | table / qwen35 | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | 非契約selector key assert / model | 478 |
| `table_gemma31_001` | table / gemma31 | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | reportにliteral `count`要求 / model | 830 |
| `table_qwen35_002` | table / qwen35 | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | Python verifyをdependency setup誤分類 / machine（近因） | 592 |

全件のharness statusは`completed`、product exitは1。blocked、
`interrupted(environment)`、panic、理由なし終端は0件。

## 4. AssuranceとN evidence

6件の`run_stop`は同じ投影境界を保持した。

```json
{
  "profile": "ingest",
  "status": "failed",
  "failure_kind": "process_failure",
  "final_acceptance_status": "not_checked",
  "assurance_level": "static",
  "assurance_reason": "ingest_probe_not_run"
}
```

これは契約§4の`N1未実行 → static`に一致する。未実行なのにpartialを
表示したrun、生成物の存在だけで昇格したrun、failedなのにfullを表示した
runはいずれも0件。

N evidence実在監査:

| evidence | 実在run |
|---|---:|
| `ingest-candidate-freeze.json` | 0/6 |
| `ingest-probe.json` (N1) | 0/6 |
| `source-binding.json` (N2) | 0/6 |
| `candidate-accounting.json` (N3) | 0/6 |
| `format-schema.json` (N4) | 0/6 |
| `rerun-consistency.json` (N5) | 0/6 |
| `ingest-assurance.json` | 0/6 |

### 4.1 要求された実物監査

final acceptance到達runが0件なので、次のN evidence原文は**存在しない**。

- セレクタ宣言と実行前凍結記録: 0件
- `detected = accepted + excluded`のN3実数: 0件
- 不備2件の理由別除外をN3が裁定した記録: 0件
- 和暦→ISOの宣言済み変換過程をN2が記録した行: 0件
- N2 `source_binding_violation`原文: 0件

モデル生成workspaceは6本中5本で`pipeline/main.py`と3つのoutputを
最終状態に持っていたが、これをN evidenceへ読み替えない。例えば
`table_qwen35_001`のモデル生成inspectionは
`candidate_selector.type = "html_table"`を宣言し、会場未定候補を採用して
いた。これはmanifestの正準selector `kind/value`とも、入力設計の
理由付き除外とも一致しないが、N3は未起動なのでlive判定値には数えない。

従って実戦初値は:

- N1〜N5到達: `0/6`
- N2 pass / fail: `0 / 0`
- N3 pass / fail: `0 / 0`
- full相当: `0/6`

## 5. E-0検収

### 5.1 自動分類

benchの`classify_runs`はlogical run 6件を重複なしで分類した。

- known: 6
- UNKNOWN: 0
- class: `process_failure` 6
- registry attribution: `model` 6

新profile初戦でもgeneric `process_failure`が全件を捕捉したため、
期待され得たUNKNOWNは出なかった。自動分類の新規class登録入力は0件。
ただし§6の一次資料監査により、1件はmachine近因へ精密化した。

### 5.2 検収シート

`sheet_generated=true`は6/6。各sheetはgoal、profile、intent、実効
executor/provider、planner、failed verdict、recovery pathsを自動転記した。
シート自給率は`6/6 = 100%`。

未到達runのsheetでは完成定義・検証実録が`記録なし`となり、assuranceは
「未完了。回収情報あり」と表示された。正確な
`static (ingest_probe_not_run)`はsummary/meta側に存在する。sheet生成は
成立したが、ingest固有N欄の翻訳はまだない。

### 5.3 較正collector

自動collector追加は0件。理由は二層ある。

1. N2/N3到達runが0で、収集対象となるnearest_miss自体が0。
2. `calibration_corpus.py`はE2/I2/C2/C3形を認識するが、
   ingest `source-binding.json`のbindings/nearest_miss形は未対応。

後者を
`bench_calibration_corpus:ingest_evidence_shape_unsupported`としてgap記録
する。今回は記録のみでcollectorやclassesを変更していない。

## 6. 死因の機械/モデル帰属

自動classifierの`process_failure / model`は終端形の既定値であり、
一次資料による精密帰属は次のとおり。正式な恒久class裁定はレビュー対象。

### 6.1 model近因 5件

- `list_qwen35_001`: 最初のphaseで`pipeline/main.py`と
  `smoke-check.py`を作らず、artifact feedback 3回後に停止。
- `list_gemma31_001`: 4成果物は作成したが、モデル生成
  `smoke_check.py`が`TypeError: 'module' object is not callable`。
  bounded repair後も進展せず`model_stagnation:no_progress_recorded`。
- `list_qwen35_002`: required `verify_pipeline.py`に対し
  `verify_smoke.py`を書いた。missing path feedback 3回後にもファイル名を
  合わせず`artifact_recovery_exhausted`。
- `table_qwen35_001`: モデル生成verifyが契約にない
  `candidate_selector.table_selector/row_selector/field_mapping`を要求。
  実inspectionも正準`kind/value`形ではなく、モデル内のschema不整合。
- `table_gemma31_001`: reportは
  `Total candidates identified: 10 / Successfully extracted: 8 /
  Excluded: 2`と件数を持つが、モデル生成verifyが英単語`count`の字義を
  要求して拒否。artifactとverifyを同じモデルが不整合にした。

### 6.2 machine近因 1件

`table_qwen35_002`はverify command原文が単純な
`python3 verify_pipeline.py`だったが、runtimeは次を記録して実行前停止した。

```json
{
  "primary_reason": "dependency_setup_authority_required: python3 verify_pipeline.py",
  "dependency_missing": [
    "dependency_setup_authority_required: python3 verify_pipeline.py"
  ],
  "command_failures": 0,
  "reachable": false
}
```

ローカルPython script実行をdependency setupとして扱った近因はmachine。
`verify_dependency_classifier:python_script_misclassified_as_setup`を
新class候補として記録する。なお同runの生成inspectionはcontract正準形で
なく、verify scriptにもworkspace root算出誤りがあるため、このmachine
gapを除いてもfullを推定しない。

集計はmodel 5 / machine近因1。修正・classes登録はレビュー裁定まで行わない。

## 7. 族・executor差

| slice | run | failed | final acceptance到達 | full相当 | 平均秒 |
|---|---:|---:|---:|---:|---:|
| list | 3 | 3 | 0 | 0 | 997 |
| table | 3 | 3 | 0 | 0 | 633 |
| qwen35 | 4 | 4 | 0 | 0 | 653 |
| gemma31 | 2 | 2 | 0 | 0 | 1139 |

停止時点の4成果物実在はqwen35 3/4、gemma31 2/2。ただし全runがN未到達
なので、モデルが「作った」ことと契約が「受け入れた」ことを分離する。
N2/N3について族差・executor差を判定できる実弾値はまだない。

## 8. コスト

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| preflight開始 | 1785135490 | 2026-07-27 15:58:10 |
| run開始 | 1785135664 | 2026-07-27 16:01:04 |
| 最終run終端 | 1785140555 | 2026-07-27 17:22:35 |
| 一次監査終了 | 1785140961 | 2026-07-27 17:29:21 |

- preflight: 174秒
- 6 run合計: 4891秒
- preflight開始→最終run終端: 5065秒
- preflight開始→一次監査終了: 5471秒
- family合計: list 2991秒 / table 1900秒
- executor合計: qwen35 2613秒 / gemma31 2278秒

## 9. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed、product exit 1、`run_stop`、具体的stop reasonを6/6保持 |
| P0-b 契約§assurance準拠 | **PASS** | N1未実行を6/6 static (`ingest_probe_not_run`)。未実行×partial再発0 |
| P0-b off capのfull相当時挙動 | **NOT MEASURED** | full相当到達run 0。admissionはoffのまま |
| P0-c 偽成功ゼロ | **PASS** | verdict failed 6/6、partial/full表示0 |
| P1-a 到達runでN1〜N5実行 | **NOT MEASURED** | final acceptance到達run 0、N evidence 0 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- full相当率: `0/6`
- N2/N3 live成績: 未到達
- 新規自動class: 0
- 新class候補（記録のみ）:
  `verify_dependency_classifier:python_script_misclassified_as_setup`
- collector gap（記録のみ）:
  `bench_calibration_corpus:ingest_evidence_shape_unsupported`

## 10. Scrubと一次資料

- run別scrub: 6/6 `ok=true`
- campaign全体:
  `{"ok": true, "findings": []}`
- `.env`、credential、token、raw secretのcommit対象混入: 0
- environment interruption: 0
- one-time new-run retry: 0
- manual terminal switch: 0

一次資料hash:

- external `uat-meta.json`:
  `bace0244cca837c90d97db758538d0855087fcec63862fe22f974165568eec3e`
- external `report-skeleton.md`:
  `dc5b690ba64be017e487349b0fc0ba19f241c39738ddc24fe0b1eb9350adffb3`
- repository machine summary:
  `evidence/campaign-summary.json`

raw logsと外部workspaceはcommitせず、scrub済みの監査報告と派生machine
summaryだけを台帳化する。
