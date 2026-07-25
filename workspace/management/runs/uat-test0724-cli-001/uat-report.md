# uat-test0724-cli-001: cli×create初計測

実施日: 2026-07-25 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

対象revision: `27d787bb085d3347bd86576561d869ae57840182` (`develop`)

## 結論

**BLOCKED（suite仕様のレビュー裁定が必要）**。

契約どおりsourcesを持たない `cli-create` suiteを追加し、指定されたbench
コマンドを実行した。現行benchはsuiteロード時に `[[sources]]` を最低1件
要求するためexit 2で停止し、campaign、preflight、製品runはいずれも開始
されなかった。create intentの空workspaceを「調達・precheckなし」で表現
できないsuite仕様欠落である。指示に従いbench実装の修正、ダミーsourceの
追加、手動での製品run代替は行っていない。

## 0. E-3b受理残

E-3bの3コミット `227e9ce` / `46e4f1a` / `27d787b` pushに対する最終値:

| workflow | run id | head sha | status | conclusion |
|---|---:|---|---|---|
| CI | `30131079847` | `27d787bb085d3347bd86576561d869ae57840182` | `completed` | `success` |
| acceptance | `30131079862` | `27d787bb085d3347bd86576561d869ae57840182` | `completed` | `success` |

`docs/dev/integration-notes.md` の既知flake棚へ、E-3b full初回で発現し
単独1/1 passかつfull再走greenだった80msキャンセル猶予テストを1行追記した。

## 1. Suite定義

`workspace/management/bench/suites/cli-create.toml` に次を固定した。

- `profile=cli`, `intent=create`, `plan_preset=default`
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: qwen35 4本、gemma31 2本
- goal: stats族3本、filter族3本
- sources: なし（空workspaceから開始するcreate契約）
- minimum HEAD: `27d787b`

stats / filterのgoal本文と6 run名はsuite TOMLに機械束縛されている。

## 2. 手動UATチェック

### Preconditions

- Ollama daemon: `ollama ps` exit 0
- 必要モデル: planner、qwen35 executor、gemma31 executorを
  `ollama list` で確認
- 指定workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_001`
  は実行前に不存在
- preflight full greenとbinary version確認: bench所有。suiteロード停止のため未実行

### Steps

1. sourcesなしのsuiteを追加する。
2. `date +%s` を採取する。
3. 指定された `bench.py run` コマンドを権限付き実行する。
4. 終了直後に `date +%s` を採取する。
5. campaign・run・自動生成物の有無を監査する。

### Expected Result

benchがsourcesなしcreate suiteを受理し、full-green preflight後にfreshな
6 workspaceを作って6 runを正直終端させる。

### Actual Result

```text
bench: suite must define at least one [[sources]] table
```

exit `2`。開始epoch `1784954624`、終了epoch `1784954624`、秒差 `0`。
原記録は `evidence/suite-load.json` に保存した。workspace rootとcampaignは
作成されず、環境中断・再実行・人手ターミナル切替はいずれも0回だった。

## 3. Run行列

全行ともsuiteロードより後のrun recordが生成されていないため、
assuranceやC1〜C4を推定・代入しない。

| run | family / executor | harness status | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス | 秒 |
|---|---|---|---|---|---|---|---|---|---|---:|
| `stats_qwen35_001` | stats / qwen35 | not-created | 未発行 | 未発行 | 未到達 | 未到達 | 未到達 | 未到達 | suite-load blocker（機械） | — |
| `stats_gemma31_001` | stats / gemma31 | not-created | 未発行 | 未発行 | 未到達 | 未到達 | 未到達 | 未到達 | suite-load blocker（機械） | — |
| `stats_qwen35_002` | stats / qwen35 | not-created | 未発行 | 未発行 | 未到達 | 未到達 | 未到達 | 未到達 | suite-load blocker（機械） | — |
| `filter_qwen35_001` | filter / qwen35 | not-created | 未発行 | 未発行 | 未到達 | 未到達 | 未到達 | 未到達 | suite-load blocker（機械） | — |
| `filter_gemma31_001` | filter / gemma31 | not-created | 未発行 | 未発行 | 未到達 | 未到達 | 未到達 | 未到達 | suite-load blocker（機械） | — |
| `filter_qwen35_002` | filter / qwen35 | not-created | 未発行 | 未発行 | 未到達 | 未到達 | 未到達 | 未到達 | suite-load blocker（機械） | — |

契約§4のdraft上限実挙動は未計測である。admission=offのprofileが
full相当をどう表示するかは製品裁定まで到達しておらず、`static` を含む
assuranceをbench側から捏造していない。full相当率、族差、executor差は
いずれも分母未成立でN/A。

## 4. E-0装備の実戦検収

| 装備 | 実測 | 裁定 |
|---|---|---|
| `classify_runs` 自動分類 | known `0` / UNKNOWN `0` / planned-but-unclassified `6` | campaign不在で分類器未起動。UNKNOWNを推定登録しない |
| 検収シート自動生成 | `0/6` | run artifact不在。P1-b不成立 |
| 較正コーパス自動蓄積 | 追加0件 | report skeleton生成前に停止し、C2/C3 `nearest_miss` も不在 |

新セルのクラス収穫は分類器外の
`bench_suite_schema:create_empty_sources_unsupported` 1 campaign件である。
既知/UNKNOWNのrun分類へ混入させていない。

## 5. C系evidence実物監査

実行観測evidenceは0件であり、要求された4種の原文例は存在しない。
不存在を成功や `claims_absent` と読み替えていない。

| 監査対象 | 実物 |
|---|---|
| ケース束縛の凍結記録 | なし（C1未到達） |
| 正常・不正の極性両側実行 | なし（argv probe未起動） |
| help照合の方向別結果 | なし（C2未到達） |
| C3束縛の対照表 | なし（C3未到達） |

今回転記できる唯一の実行原文はsuite loaderの次の拒否である。

```text
bench: suite must define at least one [[sources]] table
```

## 6. 死因帰属と合否

一次死因は機械1 campaign、モデル0、複合0。モデル呼出し前のため、
CLI生成品質の死因分布には算入しない。

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **FAIL** | run record 0/6。bench自身はexit 2で正直終端 |
| P0-b 契約§4準拠 | **NOT MEASURED** | product verdict / assurance未発行、draft上限未観測 |
| P0-c 偽成功ゼロ | **PASS（観測範囲）** | 成功・full・partial主張0 |
| P1-a C1到達runで実行 | **NOT MEASURED** | C1到達0 run |
| P1-b 検収シート6/6 | **FAIL** | 0/6 |

## 7. Regression and safety

- `src/`、`tests/` の変更は0。
- `docs/` は明示指定されたintegration-notesの1行追記だけ。
- 既存run evidenceの上書きは0。この新規run directoryだけを追加した。
- 指定外の調達、ダミー入力、製品run、worker dispatchは0。
- scrub対象は本run directoryで、findings 0をコミット前に確認する。
- `cargo fmt --all -- --check`: exit 0。
- Python 3.12を明示したbench / acceptance-sheet / classify-runs focused test:
  18 tests、exit 0。先行2回はmodule pathとsystem Python 3.9解決の起動誤りで、
  製品・テストassertionの失敗ではない。

## 8. Follow-up prompt

レビュー裁定依頼: create intentに限り `[[sources]]` とrunの `set` を省略可能にし、
`procure_run` が空のfresh workspaceを作成してprecheckをスキップするbench
schema拡張を行うか裁定してください。裁定後は同じsuite定義と新規workspace
rootで6 runを再実行し、本レポートとは別の新規run directoryへ結果を保存します。
