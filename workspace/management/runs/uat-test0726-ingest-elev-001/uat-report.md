# uat-test0726-ingest-elev-001: ingest×create elevated計測

実施日: 2026-07-27 (JST)

裁定契約: `docs/ingest-profile-contract.md` (fixed 2026-07-25)

計測revision: `d70e1381a16719f722639c66a74a79cafa510648`
(`develop`)

## 結論

**P0-a / P0-b / P0-c / P1-bはPASS。P1-aはfinal acceptance到達runが
0件のためNOT MEASURED。製品結果はfailed 6/6、full相当0/6だった。**

正式campaignは6/6をproduct exit 1と具体的stop reason付きで正直終端した。
全runはfinal acceptance前に停止し、契約§assuranceどおり
`failed / static (ingest_probe_not_run)`だった。N1〜N5 evidenceは0件で、
未実行をpartial/fullへ投影した偽成功は0件だった。

INGEST-1の2修正はproductionで発火した。

- モデル生成verifyは中間phaseで最終構造gateへ延期され、修正前診断で
  発生した早期`ingest_phase_structure:pipeline_missing`は正式6runで0件。
- 素の`python3 pipeline/main.py`は複数runで実行成功し、
  `dependency_setup_authority_required`への誤分類は0件。

一方、5runは`model_stagnation:no_progress_recorded`、1runは
`artifact_follow_through_exhausted`で停止した。一次資料監査では、
成功した実行専用stepをworkspace差分なしとして停滞扱いするmachine候補が
4runを阻害した。残る2runはmodel/planner近因だった。

admissionは`off`のまま。full相当へ到達したrunがないため、
`profile_not_admitted`によるdraft上限の実挙動は今回も未計測である。
観測されたstaticはN1未実行を示す`ingest_probe_not_run`だった。

## 1. INGEST-1実装

### 1.1 モデル自作verifyの廃止

要求の混入地点はingest manifest/presetではなく、
`src/planner/runner.rs`の生成StepPlan合成経路だった。plannerが
`smoke-check.py`、`verify_pipeline.py`、inline Python/Node assertionを
`expected_paths`と`verify`へ入れていた。

新しい`src/planner/profiles/ingest/phase_verify.rs`は、意味検証を行わない
言語中立な構造gateへ束縛する。

- `pipeline/main.py`実在
- `output/records.json`がJSONとしてparse可能
- `output/inspection.json`の`candidate_selector`が`kind/value`正準形
- `output/report.md`実在

束縛・勘定・schema・再実行はN1〜N5の管轄であり、phase gateは検証しない。
初計測の実planを
`tests/fixtures/ingest-phase-structure/table_qwen35_002-plan.yaml`へ固定した。

修正前の診断campaignで、中間分析phaseにもfull構造gateが付き、
後続phaseで作る予定の`pipeline/main.py`を早期要求する配線gapを検出した。
通常の単一planとUltraPlan最終phaseだけに構造gateを置き、中間phaseでは
モデルverifyを除去してterminal gateへ延期する境界を同じコミットへ
折り込んだ。focused fixtureは6/6 green。

production原文（正式`list_cloud_001`中間phase）:

```json
{
  "event": "verify_canonicalized",
  "step_id": "verify-outputs",
  "original": "kind=verify; expected_paths=; verify=python -c \"import json; json.load(open('output/records.json'))\"",
  "replacement": "deferred to terminal ingest phase structure gate",
  "disposition": "canonical"
}
```

最深到達`table_cloud_003`の最終phaseでは3つのモデルverifyが
`anvil-ingest-check:phase_structure`へ束縛された。ただし最初の
`execute-pipeline` stepで停止したため、構造gate自体のlive実行前だった。

### 1.2 Python command分類

`src/planner/verify.rs`の成果物not-found分類が、先頭が`python3`というだけで
workspace内実在scriptの実行をdependency setupへ寄せることが機序だった。
新しいleaf `src/planner/verify/dependency_classification.rs`で判定を分離した。

- `python` / `python3` + workspace内に実在する相対`.py`:
  command execution
- `pip install`等の明示的installer:
  従来どおりsetup authority必須

実run原文`python3 verify_pipeline.py`のfixtureと、`pip install -e .`の
非退行fixtureを持つ。class
`verify_dependency_classifier:python_script_misclassified_as_setup`は
`workspace/management/classes.toml`へmachine、
`first_seen=uat-test0726-ingest-001`として登録した。

正式campaignでは次が複数runで成立した。

```json
{
  "command_summary": "python3 pipeline/main.py",
  "blocked": false,
  "event": "runtime_bash_policy",
  "reason": "runtime Bash is not deterministic verifier evidence"
}
{"event":"tool_execute","name":"Bash","status":"ok"}
```

`dependency_setup_authority_required`発火は0件だった。

## 2. Suiteとpreflight

### 2.1 実効構成

- suite: `ingest-create-elevated`
- profile / intent: `ingest / create`
- workspace mode: `sourced`
- planner: `qwen3.6:27b-coding-nvfp4 / ollama`
- executor: `gemma4:31b-cloud / ollama` 6本
- admission: `off`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0726_ingest_elev`
- 正式campaign: `ingest-create-elevated-20260727-104919`

source hashはlocal初計測と同じ。

| family | asset | sha256 |
|---|---|---|
| list | `events-list.html` | `dadcc23ffc94494d7d167e2733e05ec0e6ea339b6791a65d91b7e55832eeee07` |
| table | `events-table.html` | `394b03ccd7ac141a2677c55d9a2059034ea2c7a92656b466201ee40b7050cddd` |

input sha256一致とzero-exit precheckは6/6。6枚の自動検収シートで
effective model/providerを照合し、`gemma4:31b-cloud / ollama`一致は6/6。
planner一致も6/6だった。

### 2.2 preflight

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD | `d70e138 Measure elevated ingest creation` |
| minimum ancestor | `78b95ce` verified |
| bench内`cargo test` | exit 0 |
| release build | exit 0 |
| installed binary | `commandagent 0.1.0 d70e138 2026-07-27T10:51:52Z` |
| `NODE_ENV` | `production` |

手動の権限付き受理suiteも次のとおりgreen。

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --all-targets`
  — 1822 passed / 0 failed / 30 ignored
- Python bench unittest 16/16、classifier unittest 4/4

## 3. Run行列

`—`はfinal acceptance未到達によるN未実行。

| run | family | verdict | assurance | N1 | N2 | N3 | N4 | N5 | 停止形 / 監査帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `list_cloud_001` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | rewrite未編集後`model_stagnation` / model | 716 |
| `list_cloud_002` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | 成功実行をno-progress判定 / machine候補 | 465 |
| `list_cloud_003` | list | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | 成功実行をno-progress判定 / machine候補 | 809 |
| `table_cloud_001` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | 成功実行をno-progress判定 / machine候補 | 840 |
| `table_cloud_002` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | future report未作成 / model-planner | 377 |
| `table_cloud_003` | table | failed | static (`ingest_probe_not_run`) | — | — | — | — | — | final phase成功実行をno-progress判定 / machine候補 | 760 |

全件のharness statusは`completed`、product exitは1。
`blocked`、理由なし終端、panic、正式campaign中のenvironment interruptionは
0件だった。

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

- セレクタ宣言・実行前凍結: live evidence 0件
- `detected = accepted + excluded`: live evidence 0件
- 不備2件の理由付き除外: live evidence 0件
- 和暦→ISOの宣言済み変換過程: live evidence 0件
- N2 violation / nearest_miss: 0件

N1〜N5の到達runが0なので、N2/N3のpass/failや不備2件の処理を推定しない。

### 4.2 モデルworkspace最終状態（N evidenceではない）

生成物の存在をacceptanceへ読み替えない条件で監査した。

| 観測 | run |
|---|---:|
| `pipeline/main.py`実在 | 6/6 |
| `output/inspection.json`実在 | 6/6 |
| `output/records.json`実在 | 6/6 |
| `output/report.md`実在 | 5/6 |
| records配列長0 | 6/6 |
| selectorが正準`kind/value` | 0/6 |
| selectorがstring | 6/6 |

例:

```json
{
  "candidate_selector": "tr.event-row",
  "candidate_accounting": {
    "total_found": 3,
    "accepted": 2,
    "excluded": [
      {"index": 2, "reason": "Missing date information"}
    ]
  }
}
```

入力の機械候補数は10だが、このモデル宣言は3へ縮小している。N3未起動なので
live violation数には加えないが、final acceptanceへ到達しても
candidate set検証の対象になる形だった。和暦変換のN2記録は存在しない。

## 5. 死因の機械/モデル帰属

自動`classify_runs`は6 logical runを重複なしで分類した。

- known: 6
- UNKNOWN: 0
- class: `process_failure` 6
- registry attribution: `model` 6

一次資料監査による精密帰属はmachine候補4 / model-planner 2。

### 5.1 machine候補 4件

`list_cloud_002`、`list_cloud_003`、`table_cloud_001`、
`table_cloud_003`は、step instructionがpipelineの実行であり、
`python3 pipeline/main.py`は`tool_execute status=ok`だった。
その直後、runtimeは次で停止した。

```json
{
  "event": "loop_stop",
  "reason": "model_stagnation:no_progress_recorded",
  "missing_paths": [],
  "missing_obligations": [],
  "verify_attempts": 0
}
```

実行専用stepは成功してもファイル差分を必須にし得ない。この4件は
`phase_execution:successful_command_no_diff_stagnation`を新class候補として
記録する。今回は計測タスクの範囲に従い、runtimeやclassesへ追加修正しない。

### 5.2 model-planner 2件

- `list_cloud_001`: phase instructionは`Rewrite pipeline/main.py`だったが、
  モデルは実行のみで編集しなかった。差分要求が正当なためmodel近因。
- `table_cloud_002`: UltraPlanはreport生成を後続phaseへ置いた一方、
  phase 1 StepPlanは`output/report.md`を期待pathに含め、モデルは作らなかった。
  `artifact_follow_through_exhausted`で正直停止した。

## 6. 検収シート・較正・assurance

- `sheet_generated=true`: 6/6
- シート自給率: 100%
- effective executor/provider一致: 6/6
- N2/N3 nearest_miss: 0（N未到達）
- 較正collector追加: 0

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
| preflight開始 | 1785149359 | 2026-07-27 19:49:19 |
| run開始 | 1785149530 | 2026-07-27 19:52:10 |
| 最終run終端 | 1785153498 | 2026-07-27 20:58:18 |
| 一次監査終了 | 1785153737 | 2026-07-27 21:02:17 |

- preflight: 171秒
- 6 run合計: 3967秒
- preflight開始→最終run終端: 4139秒
- preflight開始→一次監査終了: 4378秒
- family合計: list 1990秒 / table 1977秒

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
- 新class候補（記録のみ）:
  `phase_execution:successful_command_no_diff_stagnation`

## 9. Scrub・一次資料・除外campaign

- run別scrub: 6/6 `ok=true`
- campaign全体: `{"ok": true, "findings": []}`
- `.env`、credential、token、raw secretのcommit対象混入: 0
- 正式campaign中のenvironment interruption / retry / terminal切替: 0

一次資料hash:

- external `uat-meta.json`:
  `2d82c9aac1f6a6ea65b4be199a5b197a7254151a6d7bfaccb1709d38a8381f37`
- external `report-skeleton.md`:
  `9567f2a1a7a9e9acfc59ba3df27941eac4d9778ca0d90fa5c0ff92dea31e831c`
- repository machine summary:
  `evidence/campaign-summary.json`

精密化前の診断campaign
`ingest-create-elevated-20260727-093142`と
`ingest-create-elevated-20260727-094755`は正式母集団から除外した。
後者が中間phaseへの構造gate早期付与を発見し、コミット1へ修正を
折り込む入力になった。raw logsと外部workspaceはcommitしない。
