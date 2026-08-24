# uat-test0726-ingest-elev-003: ingest×create elevated再計測

実施日: 2026-07-27〜28 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `8fdc5a90081c3d09ce514519dfa2db707ab0ed36`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-bはPASS。P1-aはfinal acceptance到達runが
0件のためNOT MEASURED。製品結果はfailed 6/6、full相当0/6だった。**

INGEST-3の字義例ガイダンスは正式6runすべてのproduction計画へ配布された。
前campaignの停止形
`ingest_phase_structure:selector_not_kind_value`は0/6になった一方、
全runがその前の生成・StepPlan段で停止し、構造gateおよびN1〜N5のlive通過は
未計測である。従って「字義例のproduction配布」は成立したが、
「正準形からN runtimeまでのend-to-end成立」は主張しない。

新しい機械gapを2runで観測した。ingest正準化はモデル生成verifyコマンドを
production構造gateへ置換したが、同じStepPlanの
`smoke-check.js` / `verify-artifacts.js`を`expected_paths`から除去せず、
存在しないモデル自作検証成果物をrequired pathとして残した。近因はplanner
出力、設計根因はmachine（正準化のcommand/path非対称）と裁定する。
残り4runはplannerが「verify stepで変更を要求しない」規則を3回の是正後も
満たさず、model帰属で正直終端した。計測タスクなのでいずれも修正しない。

初回`list_cloud_001`はexecutor初回呼出しのOllama HTTP 500で環境中断した。
確立プロトコルどおり、このセルだけ同じ入力・goal・planner・executorで
新規1回再実行し、そのretry結果を正式6runへ採用した。初回270秒はコストには
残すが、成績母集団から除外する。

## 0. 開始条件

計測開始時点の`develop`先端`4f856a9`に対する最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30274680183` | completed | success |
| acceptance | `30274680997` | completed | success |

## 1. INGEST-3字義例ガイダンス

### 1.1 production対応表

commit `8fdc5a9 Publish ingest canonical artifact shapes`で、構造gateの
全要求形に先行する字義例を追加した。

| 構造gateの拒否形 | 先行ガイダンス |
|---|---|
| `pipeline_missing` | `pipeline/main.py`実在 |
| `records_missing` / `records_invalid` | object配列の`output/records.json`字義例 |
| `selector_not_kind_value` | `candidate_selector.kind/value`字義例 |
| selector kind不正 | 許容語彙`css / html_tag / line_prefix` |
| accounting / format shape不正 | `accepted` / `excluded` / `record_format.fields`の完全字義例 |
| `report_missing` | `output/report.md`実在 |

productionへ配布したselector字義例:

```json
{"candidate_selector": {"kind": "css", "value": "ul.events > li"}}
```

同じガイダンスは、例の値を固定値として写さず、実スナップショットを観測して
selector、candidate id、field、normalization、record値、除外理由をすべて
置換するよう明記する。これはDATA-1形式の「形を先に配り、値は実測で埋める」
規則である。

正式6runのretained planを検索し、この字義形・許容語彙・完全inspection形・
records形・例の非固定化定型が6/6へ注入されたことを確認した。

### 1.2 fixtureとscaffold

- elev-002の実測文字列selector fixtureは構造gateで拒否される。
- 同じ実測形を`kind/value`へ直した対fixtureは構造gateを通過する。
- 合成計画snapshotはgeneration rulesに字義例と実測値置換定型を保持する。
- scaffold checklistの第3定形装備として
  「every structure-gate required shape has prior literal-example guidance
  and a fixture」を追加した。

手動の権限付き受理suite:

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets`:
  **1846 passed / 0 failed / 30 ignored**（41 test suites）
- ingest focused Rust: 25/25
- ingest conformance: 2/2
- generality guardrails: 9/9
- scaffold unittest: 3/3
- Ruff: green

elev-002帰属は「近因model / 根因machine (knowledge) 6/6」へ追記裁定し、
class `ingest_phase_structure:selector_not_kind_value`をmachine、
`first_seen=uat-test0726-ingest-elev-002`、INGEST-3解消として登録した。

## 2. Suite・preflight・retry

### 2.1 実効構成

- base suite: `ingest-create-elevated`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama`
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev3`
- base campaign: `ingest-create-elevated-20260727-153436`
- retry campaign: `ingest-create-elevated-list1-retry-20260727-165652`

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input sha256一致とzero-exit precheckは、base 6/6とretry 1/1。
acceptance sheetおよび`run_start` / `provider_turn_duration`で、
正式6runのeffective executor/provider一致は6/6、planner一致も6/6だった。

### 2.2 preflight

baseとretryのbench preflightはいずれも次を満たした。

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `8fdc5a9 Publish ingest canonical artifact shapes` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 8fdc5a9 2026-07-27T15:37:11Z` |
| built / installed sha256 | `216118c82d42e4f2eb185ddcb126a5be71d90b1212937477ebc8e44761f8c285` / 同一 |
| `NODE_ENV` | `production` |

初回`list_cloud_001`の原文:

```text
phase inspect-snapshots-and-design-selector failed:
Ollama request failed: HTTP 500 Internal Server Error
```

これは環境中断として正式母集団から除外し、同一条件の
`list_cloud_001_retry1`を1回だけ実行した。retryはHTTP 500を再発せず、
通常のmodel/planner経路を1115秒実行して正直終端したため、
logical `list_cloud_001`の正式値に採用する。追加retryはない。

## 3. Run行列

`—`はfinal acceptance未到達によるN未実行。

| logical run | family | verdict | assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` (`retry1`) | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | StepPlan verify変更要求 / model | 1115 |
| `list_cloud_002` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | verifier required-path残留 / machine | 740 |
| `list_cloud_003` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | StepPlan verify変更要求 / model | 624 |
| `table_cloud_001` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | StepPlan verify変更要求 / model | 1123 |
| `table_cloud_002` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | StepPlan verify変更要求 / model | 784 |
| `table_cloud_003` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | verifier required-path残留 / machine | 1188 |

全正式runのharness statusは`completed`、product exitは1。
panic、理由なし終端、偽成功は0件。

model帰属4runの共通原文:

```json
{"event":"planner_error",
 "planner_error_kind":"planner_lint_error",
 "planner_error_message":"verify step instruction must not request file changes",
 "planner_stage":"lint",
 "repair_attempt":3}
```

```json
{"event":"ultra_phase_failed",
 "stage":"scaffold",
 "reason":"invalid StepPlan after corrective retries: verify step instruction must not request file changes"}
```

## 4. N1〜N5実物監査

### 4.1 evidence実在

| evidence | 正式run |
|---|---:|
| candidate selector freeze | 0/6 |
| ingest probe (N1) | 0/6 |
| source binding (N2) | 0/6 |
| candidate accounting (N3) | 0/6 |
| format schema (N4) | 0/6 |
| rerun consistency (N5) | 0/6 |

6runともfinal acceptance前で停止した。従って要求されたlive値は:

- セレクタ凍結記録: 0件
- `detected = accepted + excluded`: 0件
- 不備2件の理由付き除外 / silent drop拒否: 0件
- 和暦→ISOのフィールド別変換過程: 0件
- N2 violation / nearest_miss: 0件

生成途中のworkspace成果物をN evidenceへ読み替えず、N2/N3の
pass/failを推定しない。

### 4.2 assurance

正式6件の`run_stop`はすべて:

```json
{
  "status": "failed",
  "failure_kind": "process_failure",
  "final_acceptance_status": "not_checked",
  "assurance_level": "static",
  "assurance_reason": "ingest_probe_not_run"
}
```

未実行からpartial/fullを得たrunは0。off profileのfull相当capは
full相当run 0のため未計測である。

## 5. 字義例の実戦観測と新gap

### 5.1 配布の成立、end-to-endは未計測

各runのretained StepPlan goalに次のproduction原文が存在した。

```text
Allowed candidate_selector kind values are exactly css, html_tag, and
line_prefix. Literal selector shape:
{"candidate_selector": {"kind": "css", "value": "ul.events > li"}}.
...
The values shown are examples only: inspect the actual snapshots, replace
every selector, candidate id, record value, exclusion reason, field, and
normalization with actual observed declarations, and never copy example
values as fixed data.
```

この原文の配布は6/6。前campaignの
`selector_not_kind_value`終端は0/6だった。ただしfinal構造gate到達が0なので、
これを正準形のlive passとは数えない。

bench scrub前の一次workspace観測では、retryのinspectionは
`{"candidate_selector":{"kind":"css","value":"tr.event-row"}}`を生成した。
型は正準だが、list入力の候補は`article.event`であり、
candidate idにも存在しない`snapshot1.html`等を記録していた。
これはN evidenceではなく、到達すればN2/N3が拒否すべきモデル生成途中物である。

### 5.2 verifier required-path残留

`list_cloud_002`の原文:

```json
{"event":"verify_canonicalized",
 "field":"ingest_phase_verify",
 "original":"expected_paths=output/inspection.json,output/records.json,output/report.md,smoke-check.js; verify=test -f smoke-check.js",
 "replacement":"deferred to terminal ingest phase structure gate"}
```

正準化後もstep obligationにはpathが残った。

```json
{"event":"step_obligation_scope",
 "effective_required_paths":["output/inspection.json","output/records.json","output/report.md","smoke-check.js"]}
```

終端:

```json
{"event":"loop_stop",
 "reason":"artifact_follow_through_exhausted",
 "missing_paths":["output/report.md","smoke-check.js"]}
```

`table_cloud_003`も同形で:

```text
artifact_follow_through_exhausted:
missing expected paths: verify-artifacts.js
```

`verify_canonicalized`はモデル自作verifyをmachine構造gateへ置換したと記録する
一方、verifier artifact pathをrequired pathから落としていない。
これはINGEST-1の「モデル自作検証の廃止」がcommand側だけに閉じ、
expected-path側へ完結していない機械gapである。

記録用の新live subtype:

`ingest_phase_verify:canonicalized_verifier_path_remains_required`

- count: 2/6
- attribution: machine
- first seen: `uat-test0726-ingest-elev-003`
- status: record only（本タスクでは修正・registry追加をしない）

## 6. 自動分類・シート・資格情報

`classify_runs`は正式6 logical runを重複なしで分類した。

- known: 6
- UNKNOWN: 0
- registry class: `process_failure` 6
- registry attribution: `model` 6
- 精密監査帰属: model 4 / machine 2

registryの`process_failure=model`は形状既定であり、上記2件は一次eventの
command/path非対称に基づいてmachineへ精密化した。

- `sheet_generated=true`: 正式6/6（base初回を含む物理7/7）
- シート自給率: 100%
- effective executor/provider一致: 6/6
- effective planner一致: 6/6
- N2/N3 nearest_miss: 0（N未到達）
- 較正collector追記: 0

scrub:

- base campaign: `{"ok":true,"findings":[]}`
- retry campaign: `{"ok":true,"findings":[]}`
- run別: 物理7/7 `ok=true`
- `.env`、credential、token、raw secretのcommit対象混入: 0
- クラウド資格情報の値をreport/summaryへ転記: 0

## 7. コスト

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| base preflight開始 | 1785166476 | 2026-07-28 00:34:36 |
| base run開始 | 1785166648 | 2026-07-28 00:37:28 |
| base最終run終端 | 1785171377 | 2026-07-28 01:56:17 |
| retry preflight開始 | 1785171412 | 2026-07-28 01:56:52 |
| retry run開始 | 1785171520 | 2026-07-28 01:58:40 |
| retry終端 | 1785172635 | 2026-07-28 02:17:15 |
| 一次監査終了 | 1785172857 | 2026-07-28 02:20:57 |

- base preflight: 172秒
- base物理6run: 4729秒
- retry preflight: 108秒
- retry 1run: 1115秒
- 正式6run合計: 5574秒
- family合計: list 2479秒 / table 3095秒
- 除外した環境中断: 270秒
- 両preflight + 物理7run: 6124秒
- base preflight開始→一次監査終了: 6381秒

## 8. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | formal 6/6 completed、exit 1、具体的stop reason保持 |
| P0-b 契約assurance準拠 | **PASS** | N1未実行を6/6 static。未実行×partial/full再発0 |
| P0-b off capのfull相当時挙動 | **NOT MEASURED** | full相当0/6 |
| P0-c 偽成功ゼロ | **PASS** | verdict failed 6/6 |
| P1-a 到達runでN1〜N5実行 | **NOT MEASURED** | final acceptance到達0、N evidence 0 |
| P1-b 検収シート6/6 | **PASS** | formal 6/6 `sheet_generated=true` |

記録値:

- full相当率: `0/6`
- N2/N3 live成績: 未到達
- 字義例production配布: `6/6`
- `selector_not_kind_value`終端: `0/6`
- 新規自動class: 0
- 新live subtype:
  `ingest_phase_verify:canonicalized_verifier_path_remains_required` 2/6

## 9. 一次資料hash

- base external `uat-meta.json`:
  `925588d78b51a553f7e10cf976e33bfc4af189c6a3066a45c4d807a222040429`
- base external `report-skeleton.md`:
  `6298bf7735c0c767bcad721ca84e461410de58f3606ef7b916dd92b0bc1b1453`
- retry external `uat-meta.json`:
  `b34a0fd85382b0edc12512d7f28f5f7264a60b8732d3334215cb50320611a48e`
- retry external `report-skeleton.md`:
  `2886e1ca71dd4c567523192f68b06362bb5f37be47d8f011446349a29da4e81c`
- repository machine summary:
  `evidence/campaign-summary.json`

raw logs、外部workspace、temporary retry suite、credentialはcommitしない。
