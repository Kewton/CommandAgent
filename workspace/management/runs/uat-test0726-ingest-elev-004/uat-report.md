# uat-test0726-ingest-elev-004: ingest×create elevated再計測

実施日: 2026-07-28 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `35313d9fc33a7a45895fb839a88e53ff5390dc82`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-bはPASS。P1-aはfinal acceptance到達runが
0件のためNOT MEASURED。製品結果はfailed 6/6、full相当0/6だった。**

INGEST-4の主対象であるingest create presetはproduction 6runすべてで起動した。
`plan_preset=profile`、origin `default_create_ingest`、`planner_skipped=true`、
固定3phaseの先頭`ingest-implement`、固定4 expected pathsを機械観測した。
planner provider turnは0件であり、elev-003にあったplanner由来の段・verify変更
要求・検証script pathは再発しなかった。従って「plannerに計画を発明させない」
というD-2b構造転回の第3適用はproductionで成立した。

一方、全runが固定implement段で4納品物を作り切れず正直終端した。
`pipeline/main.py`は6/6、`output/inspection.json`は4/6に存在したが、
`output/records.json`と`output/report.md`は0/6だった。executorは各runで
11 tool callを消費しながらスナップショット内容をReadせず、Globと
`ls -R data/snapshots`を反復した。計測時はこれをmodel 6/6と仮裁定したが、
INGEST-5レビューで**machine 6/6へ訂正**した。INGEST-4指示とpreset自身が
pipeline実行で生まれる2出力をimplement expectedへ置き、run前に要求したため
である。一次資料の納品分布と20秒前後の同形停止はこの段分解gapと整合する。

run phase、structural gate、final acceptanceには到達しなかったため、
要求されたN1〜N5の実物値は0件である。途中workspaceのinspectionをN evidenceへ
読み替えず、N2/N3のpass/fail、不備2件の理由付き除外、和暦→ISO変換の成否を
推定しない。

## 0. 開始条件

作業開始時点の`develop`先端
`eb191c8296bcdd14fea6939b3087694876e3fab6`に対する最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30289518177` | completed | success |
| acceptance | `30289518289` | completed | success |

## 1. INGEST-4実装と機械床

### 1.1 機械合成計画

ingest/createの明示preset未指定時はconfigが
`PlanPreset::Profile` / origin `default_create_ingest`を選ぶ。profile manifestと
leaf synthesis dispatcherが次の3段を固定する。

1. `ingest-implement`: executorへ4納品物を一括所有させた。この所有権配置は
   INGEST-5レビューでmachine gapと裁定し、model-authored 2件へ是正した。
2. `ingest-run`: machine command `python3 -B pipeline/main.py`を実行する。
3. `ingest-structural-gate`: machine-owned structural gateを実行し、成功後の
   final acceptanceがN1〜N5を起動する。

implementの固定expected paths:

```text
pipeline/main.py
output/inspection.json
output/records.json
output/report.md
```

`smoke-check.py`、`verify_pipeline.py`、`smoke-check.js`、
`verify-artifacts.js`を計画語彙から排除した。run commandとstructural gateは
planner/modelが発明するverify commandではない。合成した全StepPlanは既存
`step_plan_finalize::finalize_step_plan_for_execution`を通す。

elev-003一次資料から固定したfixtureは、verify内変更要求と2件のpath残存を
含む。canonical snapshotでは段・ownership・commandが固定され、これらを
表現できないことを両側fixtureで確認した。

### 1.2 scaffold第4定形装備

scaffoldのadmission checklistへ
`create planning is machine-synthesized/profile-preset and planner free
composition is disabled`を追加した。既存3項目（投影写像、production起動実在、
構造gate要求形の字義例）と合わせ、新profileの生成物・生成器の双方へ固定した。

### 1.3 機械床全数監査

常設監査表は
`workspace/management/runs/uat-test0726-ingest-elev-003/floor-audit.md`。
executor modelとN runtimeの間の22床を列挙した。INGEST-5で
「段×期待成果物×生成主体」を追加した。

- machine固定（字義例配布済みを含む）: 22床
- planner由来: **0床**
- open / unknown: **0床**

明示的な`--plan-preset none`はoperator opt-outとして互換維持するが、
suiteの`plan_preset=default`を含む既定production経路ではない。
elev-003のpath残存はclass
`ingest_phase_verify:canonicalized_verifier_path_remains_required`として
machine / first_seen付きで登録し、INGEST-4の固定ownershipで解消済みとした。

### 1.4 権限付きfull verification

| check | 結果 |
|---|---|
| `cargo fmt --all -- --check` | green |
| `cargo clippy --all-targets -- -D warnings` | green |
| `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets` | **1841 passed / 0 failed / 30 ignored** |
| focused ingest Rust | 35/35 |
| ingest plan synthesis | 4/4 |
| config / manifest / ultra preset | 1/1・3/3・5/5 |
| generality guardrails | 9/9 |
| scaffold unittest | 3/3 |
| Ruff | green |

growth tripwire baselineは変更していない。phase resolverの配線はleaf
`phase_plan_synthesis`へ置き、tripwire自身は既存上限内を維持した。

## 2. Suite・preflight

### 2.1 実効構成

- suite: `ingest-create-elevated`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner config: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama`
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev4`
- campaign: `ingest-create-elevated-20260728-010923`
- retry / interruption: なし

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input sha256一致とzero-exit precheckは6/6。effective executor/providerは
`run_start` 6/6および`provider_turn_duration` 66/66で一致した。
planner configも6/6一致したが、profile presetが計画を合成したためplanner
provider turnは0件だった。

### 2.2 preflight

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `35313d9 Audit the ingest machine floor` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 35313d9 2026-07-28T01:12:01Z` |
| built / installed sha256 | `0e6d769b8c2c6314446a12502bad00c30cd1930cdb37e64d83ae7162d0984d97` / 同一 |
| `NODE_ENV` | `production` |

## 3. preset production実測

### 3.1 6run共通の起動原文

`run_start`:

```json
{"event":"run_start","model":"gemma4:31b-cloud","provider":"ollama",
 "planner_model":"qwen3.6:27b-coding-nvfp4","planner_provider":"ollama",
 "plan_preset":"profile"}
```

preset解決とplanner skip:

```json
{"event":"plan_preset_resolved","plan_preset":"profile",
 "origin":"default_create_ingest"}
{"event":"preset_ultra_plan_used","planner_skipped":true}
```

先頭phase合成:

```json
{"event":"ingest_plan_synthesized","phase_id":"ingest-implement",
 "planner_skipped":true,
 "expected_paths":["pipeline/main.py","output/inspection.json",
                   "output/records.json","output/report.md"]}
```

実行時obligation:

```json
{"event":"step_obligation_scope",
 "effective_required_paths":["pipeline/main.py","output/inspection.json",
                             "output/records.json","output/report.md"]}
```

上記4 event形は6/6同一。旧verifier path、planner由来verify command、
planner lint終端は0/6だった。全runが第1phaseで停止したため、
第2・第3phaseのlive eventは未観測であり、fixture/test以上の到達主張はしない。

## 4. Run行列

`—`はfinal acceptance未到達によるN未実行。

| run | family | verdict | assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | run出力をimplement要求 / machine | 16 |
| `list_cloud_002` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | run出力をimplement要求 / machine | 18 |
| `list_cloud_003` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | run出力をimplement要求 / machine | 23 |
| `table_cloud_001` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | run出力をimplement要求 / machine | 21 |
| `table_cloud_002` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | run出力をimplement要求 / machine | 18 |
| `table_cloud_003` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | run出力をimplement要求 / machine | 24 |

全runのharness statusは`completed`、product exitは1。panic、理由なし終端、
環境中断、偽成功は0件。

停止原文は次の2形:

```text
phase ingest-implement failed: artifact_follow_through_exhausted:
missing expected paths: output/inspection.json, output/records.json,
output/report.md; artifact_stagnation_feedback_count: 1; incomplete
```

```text
phase ingest-implement failed: artifact_follow_through_exhausted:
missing expected paths: output/records.json, output/report.md;
artifact_stagnation_feedback_count: 1; incomplete
```

`loop_stop.reason`は6/6
`artifact_follow_through_exhausted`。`missing_paths`は上表の実在成果物と
整合する。

## 5. N1〜N5実物監査

### 5.1 evidence実在

| evidence | run |
|---|---:|
| final acceptance到達 | 0/6 |
| candidate selector / set freeze | 0/6 |
| ingest probe (N1) | 0/6 |
| source binding (N2) | 0/6 |
| candidate accounting (N3) | 0/6 |
| format schema (N4) | 0/6 |
| rerun consistency (N5) | 0/6 |
| nearest_miss | 0/6 |

従って今回の主眼だったlive実物はすべて未計測:

- セレクタ宣言・実行前凍結: なし
- `detected = accepted + excluded`: なし
- 不備2件の理由付き除外 / silent drop拒否: なし
- 和暦→ISOの宣言・値保存・フィールド別変換過程: なし
- N2 violation / nearest_miss: なし

### 5.2 途中inspectionの扱い

`output/inspection.json`はlist 1/3、table 3/3で作られたが、N runtimeの
freeze/evidenceではない。うち観測例:

```json
{"candidate_selector":{"kind":"css","value":"div.event-card"}}
```

```json
{"candidate_selector":{"kind":"css","value":"tr.event-row"}}
```

candidate sourceとして存在しない`data/snapshots/events.html`を記録した例も
あった。実入力は`events-list.html`または`events-table.html`である。
到達すればN2/N3が拒否すべき可能性はあるが、生成途中物なのでviolationとして
計上しない。これは偽成功を避けるためのevidence境界である。

### 5.3 assuranceとadmission cap

`run_stop`は6件とも:

```json
{"event":"run_stop","status":"failed","failure_kind":"process_failure",
 "final_acceptance_status":"not_checked","assurance_level":"static",
 "assurance_reason":"ingest_probe_not_run"}
```

契約写像どおり、N1未実行はstatic。未実行からpartial/fullを得たrunは0。
admission=offのfull相当capは、earned full相当が0件のため未計測である。

## 6. 死因の機械 / モデル帰属

### 6.1 計測コミット時の自動分類（class未登録）

`classify_runs`:

- known: 6
- UNKNOWN: 0
- class: `process_failure`
- registry attribution: model

### 6.2 計測時の初期裁定（INGEST-5で失効）

計測時は次の材料からmodel近因6件と仮裁定した:

1. plan preset origin、phase、instruction、4 expected pathsは6runで同一。
2. `planner_skipped=true` 6/6、planner provider turn 0。開いたplanner生成分布は
   今回の差分へ関与していない。
3. executor provider turnは66件、tool callは各11件。Writeは成功して
   `pipeline/main.py` 6/6、inspection 4/6を作れた。
4. executorはsnapshotの内容をReadせず、Globと
   `ls -R data/snapshots`を反復した。
5. 固定4納品物のうちrecords/reportを1件も作らず、artifact feedback後も
   完遂しなかった。

このため当時は実測subtypeを
`model_artifact_follow_through:ingest_delivery_incomplete_after_fixed_plan`
（6/6）と記録した。この裁定は次節で上書きする。

### 6.3 INGEST-5レビュー裁定

**帰属をmodel 6/6からmachine 6/6へ訂正する。**

INGEST-4のレビュー発行指示自体が、implement段へ
`pipeline/main.py`・inspection・records・reportの4件を一括所有させた。
しかしrecords/reportは`python3 -B pipeline/main.py`の実行成果物である。
実行前のimplement段で要求したため、runへ進む前にgeneric artifact
follow-throughが全runを停止した。

一次資料は、modelが直接書ける`pipeline/main.py` 6/6・inspection 4/6に対し、
実行が生むrecords/report 0/6、run到達0/6、所要16〜24秒という分布である。
これはmodel能力差より段分解の同一machine floorで説明できる。
class
`ingest_preset:runtime_outputs_bound_before_run`
をmachine / first_seen=`uat-test0726-ingest-elev-004`として登録し、
INGEST-5で解消する。裁定者の指示も監査対象である。

## 7. E-0・scrub・コスト

### 7.1 E-0

| 項目 | 結果 |
|---|---|
| 自動分類 | known 6 / UNKNOWN 0 |
| acceptance sheet自動生成 | **6/6** |
| N2/N3 nearest_miss | 0（N runtime未到達） |
| calibration collector追記 | 0 |

### 7.2 資格情報scrub

benchのrun別scrubは6/6 `ok=true / findings=[] / allow=[]`。
campaign全体を再走査した結果も:

```json
{"ok":true,"findings":[]}
```

raw console、`.anvil` runtime state、workspace途中物はcommitしない。
repoへ保存するのはscrub済み集計と人手監査レポートだけである。

### 7.3 date +%sコスト

| 区間 | epoch / 秒 |
|---|---:|
| preflight start | `1785200963` |
| preflight completed / run start | `1785201139` |
| last run end | `1785201259` |
| audit + scrub end | `1785201586` |
| preflight | 176秒 |
| formal run合計 | 120秒 |
| list族 | 57秒 |
| table族 | 63秒 |
| preflight + formal run | 296秒 |
| preflight start → audit/scrub end | 623秒 |

## 8. 事前合否

| criterion | 判定 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed 6/6、failed/exit 1、理由あり |
| P0-b §assurance | **PASS** | N1未実行6/6をstatic表示。off full capは未計測 |
| P0-c 偽成功ゼロ | **PASS** | success/partial/fullへの誤投影0 |
| P1-a 到達runでN1〜N5 | **NOT MEASURED** | final acceptance到達0 |
| P1-b sheet 6/6 | **PASS** | 自動生成6/6 |

記録値:

- full相当率: 0/6 (0%)
- N2/N3 live成績: 未計測
- family差: list/tableとも到達0、full 0
- 訂正class: run出力をimplementへ事前束縛したmachine段分解gap 6/6
- preset production起動: 6/6
- planner自由作文: 0/6

## 9. 一次資料

外部campaign:

```text
/Users/maenokota/share/work/localwork/commandagent_mvp/01/
test0726_ingest_elev4/ingest-create-elevated-20260728-010923
```

| 資料 | sha256 |
|---|---|
| `uat-meta.json` | `632571160307f419659321da9da392602a0e0de03f0c42bc18dfdde5231e1217` |
| `report-skeleton.md` | `38c4f2d8a1b460b20b2ca3704c93233ff93a3ca39886596a53ef344b99e41acc` |
| list acceptance sheet 001/002/003 | `ef55faf59996e925aa767704ab270518cd72107dc7824e93e2c4ca21a6b038bf` / `e3711d6f1b21b4b6b5eda8d25d00280a7eeb772839272c4867f359f0a9900ee8` / `881d4f1e79149e20371805c2753b30cf1a3faaf9c9ae7e5cececd4ed0eafd3dd` |
| table acceptance sheet 001/002/003 | `a90572695913356306b9e6ecb4301fe1e134f29deaaf347110ecd3d33d600856` / `a13055f6e06c1b5e3897bd9ae4e2c69aadb5da414fe62319da09849e4b2e4148` / `6dbc8c21e506fcbf792b35c6b55d9a2e1608c8fe24912bb63e9666522d79028c` |

機械可読な集計は`evidence/campaign-summary.json`へ保存する。
