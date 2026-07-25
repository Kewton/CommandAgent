# CLI-1 death anatomy: local / elevated `process_failure`

実施日: 2026-07-25 (JST)

対象契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

## 結論

代表2 runはいずれも、`cli/main.py`、`README.md`、sample dataを生成済みで、
phase 1・2を機械検証込みで完了した後、phase 3のREADME検証で停止した。
従って一次資料が示す形は「成果物を全く作れなかった」ではなく、
**「成果物を作ったが、phase内verifyが受け付けず、bounded repair 2回でも
差分を作れなかった」**である。

ただし、停止させたverifyには固定CLI契約が要求しない日本語の字義ラベルを
必須にする過制約候補がある一方、最終READMEの数値例と実ファイル／実出力にも
独立した不一致がある。既存classifierは2件とも
`process_failure / model`としたが、**最終的なmachine / model / mixed帰属は
本資料を入力にレビューで裁定する。ここでは帰属を確定せず、修正もしない。**

## 1. 対象と一次資料

| arm | run | executor | 既存終端 | 秒 |
|---|---|---|---|---:|
| local | `stats_gemma31_001` | `gemma4:31b` | `failed / process_failure` | 1631 |
| elevated | `stats_cloud_001` | `gemma4:31b-cloud` | `failed / process_failure` | 982 |

一次資料root:

- local:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_001/cli-create-20260725-061205/workspaces/stats_gemma31_001`
- elevated:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_elev/cli-create-elevated-20260725-085827/workspaces/stats_cloud_001`

主要SHA-256:

| arm | file | SHA-256 |
|---|---|---|
| local | `.anvil/runs/019f97f4-00ee-7200-a1ed-79fcf468e3b1/events.jsonl` | `9e15ee4e32ddaceda1dada1679d7f09deafeee05ab4f23c261707ee3d040da7e` |
| local | `cli/main.py` | `a0bd0e8d9121dc55dfbfffdc7f93e9f6eddc0e11e396ff1460d24775ff774d5d` |
| local | `README.md` | `15ddb734591519feaff76a23da9398e2d90beff3457789fc542f75ff3b519f4d` |
| local | `data/sample.csv` | `593ff81741fd69f07ea2aee71712bd881c4bff20233e61dca3f203339b668abc` |
| elevated | `.anvil/runs/019f9880-d5b2-7d93-93d6-a70a6a4f68e9/events.jsonl` | `72d17f9c285e9f5b0adc9f2778f981b8b6bd4f5067f737300df4f919b9ee5add` |
| elevated | `cli/main.py` | `f09e8d6d8f6478a3898a91a0021072482058c2e41cecd67844bb935f6b2bb6b9` |
| elevated | `README.md` | `9cb10a298f903d0f64d9ad2f2b4b2be4097c27fd3b52106eceb248617b2beca7` |
| elevated | `data/sample.csv` | `4488f08f70fa1d53b34542ab3076e300689fb715132ecd6c469d518b67d780ed` |

## 2. Local: `stats_gemma31_001`

### 2.1 phase進行

計画は4 phaseだった。

| index | phase | 観測終端 |
|---:|---|---|
| 1 | `create-sample-data` | `phase_verification_result ok=true`、`ultra_phase_complete ok=true` |
| 2 | `implement-cli-tool` | `phase_verification_result ok=true`、`ultra_phase_complete ok=true` |
| 3 | `create-readme` | `verify-readme-content`失敗、repair 2回とも`verify_repair_no_change`、`ultra_phase_failed` |
| 4 | `verify-artifacts` | 未開始（`ultra_phase_start`なし） |

停止段はphase 3/4の`create-readme`、停止stepは
`verify-readme-content`である。C1〜C4を実行するfinal profile acceptance
には到達しておらず、`evidence/cli-assurance.json`を含むCLI evidenceはない。

### 2.2 verify失敗原文

`step_verify_failure.primary_reason`:

```text
command failed: python -c "content=open('README.md').read(); assert '--column' in content and 'data/sample.csv' in content and '合計' in content" outcome: CommandFailed status: exit status: 1 elapsed_ms: 60 summary: AssertionError stdout: stderr: Traceback (most recent call last): File "<string>", line 1, in <module> AssertionError
```

bounded repair後の`ultra_phase_failed.reason`:

```text
step verify-readme-content failed verification after bounded repair: command failed: python -c "content=open('README.md').read(); assert '--column' in content and 'data/sample.csv' in content and '合計' in content" outcome: CommandFailed status: exit status: 1 elapsed_ms: 71 summary: AssertionError stdout: stderr: Traceback (most recent call last): File "<string>", line 1, in <module> AssertionError ; failure_kind=verify_repair_progress_unchanged
```

最終workspaceで同じverifyを再観測してexit 1を確認した。READMEには
`--column`と`data/sample.csv`が存在するが、日本語の字義`合計`はなく、
英語の`Sum`が使われている。

### 2.3 最終workspace

| 対象 | 状態 |
|---|---|
| `cli/main.py` | 存在。`argparse`、位置引数`file`、必須`--column`、CSV count/sum/average実装あり |
| `README.md` | 存在。Usage、2実行例、2出力例あり |
| `data/sample.csv` | 存在。header + 5 data rows |
| CLI `--help` | 事後監査でexit 0、`file`と`--column`を表示 |
| READMEの正常例 | `python cli/main.py data/sample.csv --column price` |
| 正常例の事後実行 | exit 0、`Count:   5 / Sum:     1000.00 / Average: 200.00` |
| standalone不正optionの事後実行 | exit 2。ただし凍結C1 probeではなく、assurance creditには不使用 |

成果物は揃っているが、README中のquantity sampleは最終
`data/sample.csv`と異なる（READMEのMelon quantityは2、実ファイルは1）。
その結果、READMEはquantityの`Sum: 28.0 / Average: 5.6`を主張する一方、
実データからは27 / 5.4になる。またprice例も実CLIの空白・小数桁と
README出力例が同形ではない。phase 3の直接死因は`合計`字義不在だが、
後続C3で問題になる独立した出力主張不一致候補も残っている。

## 3. Elevated: `stats_cloud_001`

### 3.1 phase進行

計画は4 phaseだった。

| index | phase | 観測終端 |
|---:|---|---|
| 1 | `create-sample-data` | `phase_verification_result ok=true`、`ultra_phase_complete ok=true` |
| 2 | `implement-cli-tool` | `phase_verification_result ok=true`、`ultra_phase_complete ok=true` |
| 3 | `create-documentation` | `run-verification`失敗、repair 2回とも`verify_repair_no_change`、`ultra_phase_failed` |
| 4 | `final-verification` | 未開始（`ultra_phase_start`なし） |

停止段はphase 3/4の`create-documentation`、停止stepは
`run-verification`である。localと同じくCLI profile acceptance未到達で、
C1〜C4 evidenceはない。

### 3.2 verify失敗原文

`step_verify_failure.primary_reason`:

```text
command failed: python tests/verify_readme.py outcome: CommandFailed status: exit status: 1 elapsed_ms: 50 summary: Verification failed: README should contain '使い方' stdout: Checking README.md content... Verification failed: README should contain '使い方' stderr:
```

bounded repair後の`ultra_phase_failed.reason`:

```text
step run-verification failed verification after bounded repair: command failed: python tests/verify_readme.py outcome: CommandFailed status: exit status: 1 elapsed_ms: 74 summary: Verification failed: README should contain '使い方' stdout: Checking README.md content... Verification failed: README should contain '使い方' stderr: ; failure_kind=verify_repair_progress_unchanged
```

最終workspaceで同じverifyを再観測してexit 1と同じ原文を確認した。
`tests/verify_readme.py`はREADMEに`使い方`、`実行例`、`出力例`を要求し、
CLI出力には`件数`、`合計`、`平均`を要求する。最初の`使い方` assertionで
停止したため、後続assertionは当該runでは実行観測されていない。

### 3.3 最終workspace

| 対象 | 状態 |
|---|---|
| `cli/main.py` | 存在。`argparse`、任意`--file`、必須`--column`、CSV count/sum/average実装あり |
| `README.md` | 存在。Usage、2実行例、2出力例あり |
| `data/sample.csv` | 存在。header + 5 data rows |
| `tests/test_cli.py` / `tests/verify_readme.py` | 存在 |
| CLI `--help` | 事後監査でexit 0、`--file`と`--column`を表示 |
| READMEの正常例 | `python cli/main.py --file data/sample.csv --column score` |
| 正常例の事後実行 | exit 0、`Count: 5 / Sum: 420.0 / Average: 84.0` |
| standalone不正optionの事後実行 | exit 2。ただし凍結C1 probeではなく、assurance creditには不使用 |

READMEのscore例は`Sum: 375.0 / Average: 75.0`だが、最終sampleと
実CLIは`420.0 / 84.0`である。salary例もREADMEの`210000.0 /
42000.0`に対し、sampleは5,000,000・4,000,000・6,000,000・
7,000,000・4,500,000なので一致しない。従って、最初に発火した
日本語heading assertionとは独立に、固定契約C3の出力主張不一致候補がある。

## 4. 帰属裁定の材料

| 観測 | model側候補 | machine側候補 |
|---|---|---|
| core 3成果物 | 2/2で生成済み。phase 1・2もpass | 「生成不能」を死因とする材料はない |
| phase 3 repair | READMEを読んだが2回ともwriteせず`verify_repair_no_change` | bounded repairは正直に不変を検出し停止 |
| 直接verify | 必須字義に合わせられなかった | `使い方`・`合計`等の日本語字義は固定CLI契約の要求ではなく、semanticに成立する英語READMEも拒否する過制約候補 |
| README実値 | local quantity、elevated score/salaryでsample・実出力と不一致 | 直接verifyはこれらの本質的C3不一致より先に字義不足で停止 |
| C1〜C4 | 未到達なので獲得assuranceなし | profile probeを代用せず未実行のまま保持 |

現時点の最も狭い事実表現は、**両runともモデルがCLI一式を作成し、
machine verifyがphase 3で拒否した**、である。ただし「拒否が契約上妥当か」
と「README実値不一致を一次死因へどう重みづけるか」は別問題である。
最終帰属はレビュー裁定とし、本レポートではproduction、suite、workspace、
既存UAT記録のいずれも修正していない。
