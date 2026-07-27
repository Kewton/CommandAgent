# uat-test0726-ingest-elev-002: ingest×create elevated再計測

実施日: 2026-07-27 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `f0f4761f3f2769be3c563771550216189c369be4`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-bはPASS。P1-aはfinal acceptance到達runが
0件のためNOT MEASURED。製品結果はfailed 6/6、full相当0/6だった。**

INGEST-2の主対象だった成功実行後の誤停滞は解消した。6runで
`python3 pipeline/main.py`を合計18回exit 0で実行し、
`model_stagnation:no_progress_recorded`は0件だった。同時に、
既存の`read_only_stagnation_feedback`は5run・9件で発火した。
相異なる成功実行を進捗とする精密化と、read-onlyループ検知の不変条件を
production計測の両側で確認した。

全runはfinal acceptance直前の機械正準構造gateへ到達したが、
モデルが`output/inspection.json`の`candidate_selector`を6/6で文字列として
生成した。要求される`kind/value`形ではないため、6/6が
`ingest_phase_structure:selector_not_kind_value`で正直に停止した。
N1〜N5 evidenceは0件であり、不備2件のsilent drop拒否、和暦→ISO変換、
N2/N3成績を生成物から推定しない。

admissionは`off`のまま。全件がN1未実行を示す
`failed / static (ingest_probe_not_run)`で、未実行をpartial/fullへ投影した
偽成功は0件だった。full相当runがないため、`profile_not_admitted`による
draft上限の実挙動は今回も未計測である。

## 0. 開始条件

計測開始時点の`develop`先端`d34300f`に対する最終確定値:

| workflow | run id | status | conclusion |
|---|---:|---|---|
| CI | `30264620504` | completed | success |
| acceptance | `30264620524` | completed | success |

## 1. 進捗判定の精密化

### 1.1 production規則

新しいleaf `src/minimal_loop/execution_progress.rs`は、成功したBashコマンドの
前後空白を除いた文字列をsession内で記憶する。

- 初出の相異なるコマンドかつexit 0: execution progress
- 同一コマンドの再実行: 2回目以降は非進捗
- 同一コマンドで`elapsed_ms`等の出力だけが変化: 非進捗
- 非零exit: 非進捗。後続の同一コマンド成功を消費しない
- Read / Glob / Grep: 既存のread-only判定を変更しない
- 機械生成されたdeterministic verifier代替コマンド:
  モデルのexecution progressへ数えない

`src/minimal_loop/loop_run.rs`の配線は、新規成功実行でrepair pressureと
artifact non-edit streakをリセットし、既に必要成果物が揃った実行専用stepを
`required_artifacts_satisfied_after_tool`で完了できるようにする。

commit: `f0f4761 Recognize successful command progress`

変更量は257 insertions / 5 deletions。うちRustは242 insertions /
5 deletionsで、leaf 101行、最小配線33行、module宣言1行、
focused integration fixture 107行だった。

### 1.2 fixture

実測系列と非退行を次の両側で固定した。

- `ingest_successful_pipeline_execution_without_diff_completes_step`:
  elev-001の`python3 pipeline/main.py` exit 0・既存成果物・diffなしを再現し、
  1回で`RequiredArtifactsSatisfiedAfterTool`
- `repeated_successful_command_still_exhausts_as_no_progress`:
  同一`true`反復は従来どおり
  `model_stagnation:no_progress_recorded`
- `failed_command_does_not_consume_later_success`:
  非零exitは進捗でなく、後続成功だけが初回進捗
- `implement_read_only_stagnation_exhausts_with_honest_classification`:
  Read-only反復は従来どおり
  `model_stagnation:read_only_loop`

class
`phase_execution:successful_command_no_diff_stagnation`をmachine、
`first_seen=uat-test0726-ingest-elev-001`として登録し、
noteへINGEST-2解消を記録した。

### 1.3 production実測

| run | `python3 pipeline/main.py` exit 0観測 | no-progress誤停滞 | read-only feedback |
|---|---:|---:|---:|
| `list_cloud_001` | 3 | 0 | 3 |
| `list_cloud_002` | 2 | 0 | 2 |
| `list_cloud_003` | 2 | 0 | 1 |
| `table_cloud_001` | 2 | 0 | 2 |
| `table_cloud_002` | 2 | 0 | 0 |
| `table_cloud_003` | 7 | 0 | 1 |
| **合計** | **18** | **0** | **9** |

`list_cloud_002`の原文系列:

```json
{"event":"runtime_bash_policy","command_summary":"python3 pipeline/main.py","reason":"runtime Bash is not deterministic verifier evidence"}
{"event":"tool_execute","name":"Bash","status":"ok"}
{"event":"loop_stop","reason":"required_artifacts_satisfied_after_tool"}
```

同runはこの後もphaseを進み、旧死因
`model_stagnation:no_progress_recorded`を発火しなかった。

read-only側の実測では、Glob / Readの連続に対して
`read_only_stagnation_feedback`が発火し、書込み後に回復したrunを含む。
read-only検知の停止閾値や分類は変更していない。

## 2. Suiteとpreflight

### 2.1 実効構成

- suite: `ingest-create-elevated`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama` 6本
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev2`
- 正式campaign: `ingest-create-elevated-20260727-123737`

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input sha256一致とzero-exit precheckは6/6。6枚の自動検収シートで
effective executor/providerを照合し、
`gemma4:31b-cloud / ollama`一致は6/6、planner一致も6/6だった。

### 2.2 preflightと受理suite

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `f0f4761 Recognize successful command progress` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 f0f4761 2026-07-27T12:40:14Z` |
| `NODE_ENV` | `production` |

手動の権限付き受理suite:

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets`:
  **1837 passed / 0 failed / 30 ignored**（41 test suites）

## 3. Run行列

`—`はfinal acceptance未到達によるN未実行。

| run | family | verdict | assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `selector_not_kind_value` / model | 929 |
| `list_cloud_002` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `selector_not_kind_value` / model | 891 |
| `list_cloud_003` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `selector_not_kind_value` / model | 802 |
| `table_cloud_001` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `selector_not_kind_value` / model | 591 |
| `table_cloud_002` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `selector_not_kind_value` / model | 788 |
| `table_cloud_003` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | `selector_not_kind_value` / model | 1355 |

全件のharness statusは`completed`、product exitは1。
理由なし終端、panic、environment interruption、retry、terminal切替は0件。

全件の終端主因:

```text
ingest_phase_structure:selector_not_kind_value;
failure_kind=verify_repair_progress_unchanged
```

各runはbounded repairを2回行ったが、モデルは
`output/inspection.json`をReadするだけで修正せず、正直終端した。

## 4. N系実物監査

### 4.1 N evidence実在

| evidence | 実在run |
|---|---:|
| candidate selector freeze | 0/6 |
| ingest probe (N1) | 0/6 |
| source binding (N2) | 0/6 |
| candidate accounting (N3) | 0/6 |
| format schema (N4) | 0/6 |
| rerun consistency (N5) | 0/6 |
| ingest assurance | 0/6 |

従って要求された実物値は次のとおり。

- セレクタ宣言とN1前の凍結: live evidence 0件
- `detected = accepted + excluded`: live evidence 0件
- 不備2件の理由付き除外 / silent drop拒否: live evidence 0件
- 和暦→ISOのフィールド別変換過程: live evidence 0件
- N2 violation / nearest_miss: 0件

N1〜N5到達runが0なので、N2/N3のpass/failを推定しない。

### 4.2 final gateの実物

`list_cloud_001`の`output/inspection.json`原文:

```json
{
  "candidate_selector": ".event-item",
  "candidate_accounting": [
    {
      "source_file": "data/snapshots/index.html",
      "candidate_index": 0,
      "status": "accepted",
      "reason": ""
    },
    {
      "source_file": "data/snapshots/index.html",
      "candidate_index": 1,
      "status": "accepted",
      "reason": ""
    },
    {
      "source_file": "data/snapshots/index.html",
      "candidate_index": 2,
      "status": "excluded",
      "reason": "Missing required date field"
    }
  ]
}
```

正準形は次の形であり、上記の文字列は明確に異なる。

```json
{"candidate_selector":{"kind":"html_tag","value":"article"}}
```

6runすべてでselectorは文字列だった。manifestは
`candidate_selector (kind line_prefix or html_tag, plus value)`を明記し、
terminal StepPlanも`candidate_selector as kind/value`を明記していたため、
gate判定は正当で、監査帰属はmodelとする。

生成物をN evidenceへ読み替えない条件でのworkspace観測:

| 観測 | run |
|---|---:|
| `pipeline/main.py`実在 | 6/6 |
| `output/inspection.json`実在 | 6/6 |
| `output/records.json`実在 | 6/6 |
| `output/report.md`実在 | 6/6 |
| records配列長0 | 6/6 |
| selectorが正準`kind/value` | 0/6 |
| selectorがstring | 6/6 |

`table_cloud_003`は日付正規化規則を宣言したが、recordsは空で、
N2のフィールド別変換evidenceは生成されていない。

```json
{
  "normalization_rules": {
    "date": {
      "closed_rule": "Convert various Japanese date formats (e.g., 2024年10月1日, 2024/10/01) to YYYY-MM-DD. If date is missing or invalid, the candidate is excluded."
    }
  }
}
```

従って和暦→ISOの成功にもviolationにも数えない。

## 5. 死因・自動分類

`classify_runs`は6 logical runを重複なしで分類した。

- known: 6
- UNKNOWN: 0
- registry class: `process_failure` 6
- registry attribution: `model` 6
- 精密監査帰属: model 6 / machine 0

新しいlive停止形として
`ingest_phase_structure:selector_not_kind_value`を6/6で収穫した。
これはtop-level `process_failure`の内訳であり、新規registry classは追加しない。

INGEST-2で解消対象だった
`phase_execution:successful_command_no_diff_stagnation`は0/6。
前campaignのmachine候補4件は再発せず、次の正当なmodel停止面まで
進んだと判定する。

## 6. 検収シート・較正・assurance

- `sheet_generated=true`: 6/6
- シート自給率: 100%
- effective executor/provider一致: 6/6
- effective planner一致: 6/6
- N2/N3 nearest_miss: 0（N未到達）
- 較正collector追加: 0

collectorは現時点でE2 / I2 / C2 / C3形を扱い、ingest形は未対応である。
今回はN evidence自体が0なので流出はない。対応gapは記録のみとする。

6件の`run_stop`はすべて:

```json
{
  "status": "failed",
  "failure_kind": "process_failure",
  "final_acceptance_status": "not_checked",
  "assurance_level": "static",
  "assurance_reason": "ingest_probe_not_run"
}
```

未実行×partial/full投影は0。off profileのfull相当capはfull相当run 0のため
未計測。

## 7. コスト

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| preflight開始 | 1785155857 | 2026-07-27 21:37:37 |
| run開始 | 1785156032 | 2026-07-27 21:40:32 |
| 最終run終端 | 1785161388 | 2026-07-27 23:09:48 |
| 一次監査終了 | 1785161827 | 2026-07-27 23:17:07 |

- preflight: 174秒
- 6 run合計: 5356秒
- preflight開始→最終run終端: 5531秒
- preflight開始→一次監査終了: 5970秒
- family合計: list 2622秒 / table 2734秒

## 8. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | completed、exit 1、具体的stop reasonを6/6保持 |
| P0-b 契約assurance準拠 | **PASS** | N1未実行を6/6 static。未実行×partial再発0 |
| P0-b off capのfull相当時挙動 | **NOT MEASURED** | full相当0/6 |
| P0-c 偽成功ゼロ | **PASS** | verdict failed 6/6 |
| P1-a 到達runでN1〜N5実行 | **NOT MEASURED** | final acceptance到達0、N evidence 0 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- full相当率: `0/6`
- N2/N3 live成績: 未到達
- 新規自動class: 0
- 新live停止形:
  `ingest_phase_structure:selector_not_kind_value` 6/6 (model)

## 9. Scrub・一次資料

- run別scrub: 6/6 `ok=true`
- campaign全体: `{"ok": true, "findings": []}`
- `.env`、credential、token、raw secretのcommit対象混入: 0
- クラウドexecutor資格情報の値をレポート・summaryへ転記: 0

一次資料hash:

- external `uat-meta.json`:
  `43f52d6eb3e30bcb4828b5fe2785aa536a8fef2f6ef58f6027fe6bfe881a19f4`
- external `report-skeleton.md`:
  `b7412f8bd850f25c4499e9174dd63f6fe4cbfebae1b7c6118a3becccb03f487b`
- repository machine summary:
  `evidence/campaign-summary.json`

raw logs、外部workspace、credentialはcommitしない。
