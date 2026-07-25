# uat-test0724-cli-001 v3: cli×create初実走

実施日: 2026-07-25 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

対象revision: `0bc35fb441486418914bac4c66646e7f7ee77878` (`develop`)

先行記録:

- `workspace/management/runs/uat-test0724-cli-001/uat-report.md`
- `workspace/management/runs/uat-test0724-cli-001-v2/uat-report.md`

## 結論

**PASS（計測プロトコルとE-0装備の検収）／製品結果は6/6 failed**。

bench v0.3のempty workspace経路で6本を初回セッションのまま正直終端
させた。全runが生成途中で停止したため、full相当率は`0/6`、C1〜C4到達は
`0/6`だった。一方、非0終了を成功へ読み替えず、admission=offの上限を
全runで`static (profile_not_admitted)`と記録したため、偽成功は0件である。

E-0は自動分類、検収シート自給、scrubまで稼働した。C2/C3へ到達したrunが
ないため、nearest_missと較正コーパス追加は0件だった。この不存在を
`claims_absent`や部分成功へ読み替えていない。

## 1. Preflightと実行条件

- campaign: `cli-create-20260725-061205`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_001`
- suite: `cli-create`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`
- admission: off
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: qwen35 4本、gemma31 2本
- environment interruption: 0
- 新規run再実行: 0
- 人手ターミナル切替: 0

bench所有のpreflight実測:

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD / minimum ancestor | `0bc35fb` / `27d787b` verified |
| `cargo test` | 1784 passed / 30 ignored / 0 failed |
| release build | exit 0 |
| binary | `commandagent 0.1.0 0bc35fb 2026-07-25T06:14:29Z` |
| `NODE_ENV` | `production` |

empty無垢性検証は全runで次の同形記録となった。

```json
{
  "workspace_mode": "empty",
  "created": true,
  "checked": true,
  "empty": true,
  "entry_count": 0,
  "entries": []
}
```

## 2. Run行列

`C1`〜`C4`の`—`は未実行を表す。全runがfinal profile acceptance前に
停止したため、未実行をpassとは数えない。

| run | executor | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / 帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---:|
| `stats_qwen35_001` | qwen35 | failed | static (`profile_not_admitted`) | — | — | — | — | `model_stagnation_read_only` / model | 692 |
| `stats_gemma31_001` | gemma31 | failed | static (`profile_not_admitted`) | — | — | — | — | `process_failure` / model | 1631 |
| `stats_qwen35_002` | qwen35 | failed | static (`profile_not_admitted`) | — | — | — | — | `process_failure` / model | 852 |
| `filter_qwen35_001` | qwen35 | failed | static (`profile_not_admitted`) | — | — | — | — | `model_stagnation_read_only` / model | 864 |
| `filter_gemma31_001` | gemma31 | failed | static (`profile_not_admitted`) | — | — | — | — | `process_failure` / model | 911 |
| `filter_qwen35_002` | qwen35 | failed | static (`profile_not_admitted`) | — | — | — | — | `process_failure` / model | 888 |

summary.mdのassurance原文例:

```text
Task status: failed
Assurance: static (profile_not_admitted)
```

admission=offの実挙動は、生成物が途中まで動いていてもfull相当を表示せず、
probe未実行なら契約§4の`static`に留めるものだった。draftであることを
`full`や`partial`へ昇格させる別表示はなかった。

## 3. E-0装備の実戦検収

### 3.1 自動分類

logical run単位ではknown `6` / UNKNOWN `0`。新しいcliセルでも、停止形が
登録済みの汎用クラスに一致したためUNKNOWNは出なかった。新クラス収穫は
0件である。

report skeletonの物理行はknown `12` / UNKNOWN `0`だった。分類器が
`workspaces/`の原本と`artifacts/`のcopyを双方走査し、各logical runを
2回表示したためである。本報告のrun集計はrun名で6本へ重複排除した。

停止形の内訳:

- `model_stagnation_read_only`: 2件
- `process_failure`: 4件
- attribution: model 6 / machine 0 / mixed 0 / environment 0

### 3.2 検収シート自給

acceptance sheetは`6/6`生成され、初回シート自給率は100%だった。
ただし全run未完了のため、6枚すべてで所要・完成定義・検証実録が
「記録なし」となった。生成率のPASSと内容充足度を分けて記録する。

### 3.3 較正コーパス

C2/C3 evidence、`nearest_miss`とも0件で、較正コーパス追加は0件。
worktreeにも自動追加はなかった。これは自動蓄積の故障ではなく、
全runがprofile acceptance前に停止した結果である。

## 4. C系evidence実物監査

CLI profileのevidenceファイルと対応event名をcampaign全体で検索したが、
該当は0件だった。

| 監査対象 | 実物監査結果 |
|---|---|
| ケース束縛の凍結記録 | なし。C1到達run 0 |
| 正常・不正の極性両側実行 | なし。argv probe未起動 |
| help照合の方向別結果 | なし。C2未起動 |
| C3束縛の対照表 | なし。C3未起動 |

転記可能な未到達の原文例:

```text
Stop reason: phase implement-cli-tool failed:
loop_progress_exhausted: model_stagnation:read_only_loop
Assurance: static (profile_not_admitted)
```

要求された4種類の原文evidenceは存在しないため、代替のsmoke test出力を
C evidenceとして扱っていない。

## 5. 死因と族・executor差

一次死因は登録簿の裁定に従い全6件model帰属だった。

- qwen35: full相当 `0/4`、平均824.0秒、
  read-only stagnation 2 / process failure 2
- gemma31: full相当 `0/2`、平均1271.0秒、process failure 2
- stats族: full相当 `0/3`、平均1058.3秒、
  read-only stagnation 1 / process failure 2
- filter族: full相当 `0/3`、平均887.7秒、
  read-only stagnation 1 / process failure 2

代表停止原文:

```text
phase create-readme failed: step verify-readme-content failed verification
after bounded repair
```

```text
phase implement-cli-tool failed:
model_stagnation:read_only_loop: write_required exhausted
```

両族の停止形分布は同じだった。executor差としてはqwen35だけで
read-only stagnationが発現し、gemma31は2本ともbounded repair後の
process failureだった。

## 6. コスト記録

`date +%s`基準:

| 境界 | epoch |
|---|---:|
| operator開始前 | 1784959902 |
| preflight開始 | 1784959925 |
| run列開始 | 1784960086 |
| 最終run終端 | 1784965924 |
| 監査開始 | 1784965939 |

- operator開始前→監査開始: 6022秒
- preflight開始→最終run終端: 5999秒
- 6 run所要合計: 5838秒

## 7. 合否基準

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | harness completed 6/6、product exit 1を全件保持 |
| P0-b 契約§4準拠 | **PASS** | probe未実行を6/6 `static (profile_not_admitted)` |
| P0-c 偽成功ゼロ | **PASS** | verdict failed 6/6、full/partial主張0 |
| P1-a C1が到達runで実行 | **NOT MEASURED** | profile acceptance到達run 0 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- full相当率: `0/6`
- help照合C2実戦成績: 到達`0/6`、pass/failとも0
- 新クラス: 0
- シート自給率: `6/6`

## 8. Regression, scrub, and follow-up

- `src/`、`tests/`、`docs/`の変更は0。
- 既存UAT記録の上書きは0。このv3 directoryだけを追加した。
- campaign各runのscrubは6/6 `ok=true`、campaign全体も
  `ok=true / findings=[]`。
- raw logとruntime stateはコミット対象にしていない。

Follow-up: admission=offを維持したまま、今回採取した
`model_stagnation_read_only` 2件と`process_failure` 4件を入力に
create完遂率を改善し、新規campaignで再計測する。その際はclassifierの
workspace/artifact二重表示と、未完了sheetの内容充足度も別KPIとして扱う。
