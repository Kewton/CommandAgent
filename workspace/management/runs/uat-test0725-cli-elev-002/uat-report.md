# uat-test0725-cli-elev-002: CLI-2 elevated再計測

実施日: 2026-07-25〜26 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

正式計測revision: `fca064bcc574bf031f54ae206d49a14f09a56e6d`
(`develop`)

## 結論

**README verifyの構造化は実戦で成立した。UATはP0-a/b/cとP1-bがPASS、
P1-aがFAIL。製品結果はfailed 4/6、completed static 2/6、full 0/6。**

正式campaignは6/6を正直終端させ、effective executor
`gemma4:31b-cloud`、local planner `qwen3.6:27b-coding-nvfp4`を
`run_start`で6/6確認した。empty workspace無垢性、検収シート生成、
資格情報scrubはいずれも6/6成立した。

英語READMEにgoal由来の日本語字義`合計`がなくても、構造checkへ正準化
されたphaseはgreenになった。前campaignのREADME字義過制約は再発して
いない。

一方、final acceptanceへ到達したfilter 2本でもE-3bのCLI C1〜C4
runtimeは起動しなかった。CLI-1のcompletion投影はこの欠落を
`static (cli_probe_not_run)`へ正しく写像し、generic acceptance内部の
`full_success`をterminal fullへ漏らさなかった。したがって契約§4と
偽成功ゼロは成立するが、「到達runでC1実行」のP1-aはFAILである。

## 1. CLI-2コミット1・2

コミット1 `8d4691a3f2a6c475869c983527934f9f68eaaca3`
(`Canonicalize CLI README verification`)は、CLI createのモデル生成
UltraPlan README phaseから汎用StepPlan plannerへgoal語彙が継承され、
生成verifyへ字義assertとして入る地点を特定した。`profile=cli`にはdataの
step-policyチョークポイントもNext.jsのdeterministic templateもなかった。

生成verifyを内部check `anvil-cli-check:readme_structure`へ束縛し、次の
言語中立な構造だけを検証する。

- `README.md`が通常ファイルとして実在する。
- Markdown level 2以上の使用例見出しが存在する。
- その見出し配下に、fenced code blockまたはcommand lineとして
  `cli/main.py`のPython起動例が存在する。

goal由来の自然言語字義と出力値をREADME verifyへ入れない。数値例と実出力
の忠実性はacceptance C3の管轄であることを実装コメントへ固定した。
fixtureは英語README（`合計`なし）の正例、README不在、起動例不在を含む
6形を通した。

同属監査では、dataは`step_policy::canonicalize_step_plan`のcatalog-check
チョークポイント、Next.jsはmanifest由来のdeterministic StepPlan template
で生成verifyを固定している。goal語彙字義が実行assertへ直通する経路は
現存せず、CLIの汎用planner経路だけが無防備だった。

コミット2 `fca064bcc574bf031f54ae206d49a14f09a56e6d`
(`Correct CLI process failure attribution`)は、前2 campaignの
`process_failure`をdeath anatomyに基づきmodelからmachineへ訂正し、
`classes.toml`のmodel既定を仮置き・解剖必須と明記した。local armの
read-only停滞2件はmodel帰属を維持した。

台帳には「DATA-1族第3属——goal語彙のverify字義束縛。構造検証への置換で
根治し、内容忠実性をC3へ分離」と記録した。

## 2. 権限付きverificationとpreflight

production-code変更後に次を実行した。

| check | 結果 |
|---|---|
| focused README structural unit | 6 passed / 0 failed |
| corpus regression | green（新fixtureを含む） |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| `cargo test` | 1791 passed / 30 ignored / 0 failed |
| Python classify focused unittest | 3 passed / 0 failed |
| guardrail | baseline変更なし、green |

bench所有preflight実測:

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD / minimum ancestor | `fca064b` / `704d7f9` verified |
| `cargo test` | exit 0 |
| release build | exit 0 |
| binary | `commandagent 0.1.0 fca064b 2026-07-25T13:17:27Z` |
| `NODE_ENV` | `production` |

## 3. Campaign境界

- id: `cli-create-elevated-20260725-131601`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0725_cli_elev2`
- suite: `cli-create-elevated`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`
- admission: admitted
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gemma4:31b-cloud` / `ollama` 6本
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

empty無垢性は全runで`created=true / checked=true / empty=true /
entry_count=0`だった。

## 4. Run行列

`—`はfinal acceptance未到達、`NE`は到達したがnot executedを表す。

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / 帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---:|
| `stats_cloud_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 573 |
| `stats_cloud_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 607 |
| `stats_cloud_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 557 |
| `filter_cloud_001` | filter | completed (static assurance) | static (`cli_probe_not_run`) | NE | NE | NE | NE | completed + C dispatch missing / machine | 1198 |
| `filter_cloud_002` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 420 |
| `filter_cloud_003` | filter | completed (static assurance) | static (`cli_probe_not_run`) | NE | NE | NE | NE | completed + C dispatch missing / machine | 830 |

admission昇格後の実挙動は、`profile_not_admitted`が0/6でありstatic capは
解除済みだった。今回のstatic理由は全件、契約§4に由来する
`cli_probe_not_run`である。

## 5. 実効モデル監査

各runの`run_start`原文から機械抽出した。

```text
stats_cloud_001  gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
stats_cloud_002  gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
stats_cloud_003  gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
filter_cloud_001 gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
filter_cloud_002 gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
filter_cloud_003 gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
```

executor、provider、planner、planner provider、profileは期待値と6/6一致した。

## 6. README構造verifyの実物監査

README構造checkへの正準化は5/6 run、計8件で発火した。残る
`filter_cloud_002`はREADME phase前に停止した。

`filter_cloud_001`の原文:

```json
{"disposition":"canonical","event":"verify_canonicalized","field":"verify","original":"test -f README.md","replacement":"anvil-cli-check:readme_structure","step_id":"implement-readme"}
{"disposition":"canonical","event":"verify_canonicalized","field":"verify","original":"python -c \"c=open('README.md').read(); assert '--pattern' in c and '--count' in c\"","replacement":"anvil-cli-check:readme_structure","step_id":"verify-readme-content"}
```

実行側の原文:

```json
{"event":"tool_call_raw","name":"Bash","arguments":{"argument_summaries":{"command":{"preview":"anvil-cli-check:readme_structure"}}}}
{"event":"runtime_bash_policy","command_summary":"anvil-cli-check:readme_structure","deterministic_verifier_evidence":true,"verifier_policy_ok":true}
```

`stats_cloud_001`の英語READMEには日本語字義`合計`がなく、実行例は次の
構造だった。

````markdown
## Usage

```bash
python3 cli/main.py <csv_file> --column <column_name>
```
````

このREADMEを含む`implement-cli-tool` phaseの終端原文:

```json
{"event":"ultra_phase_complete","final_phase":false,"ok":true,"phase_id":"implement-cli-tool","phase_index":1,"stage":"complete","total_phases":4}
```

前campaignを停止させた`README should contain '使い方'`型の字義assertは
0件だった。

## 7. C1〜C4 evidence実物監査

final acceptance到達はfilter 2/6。campaignの`artifacts/`と
`workspaces/`をhidden path込みで検索したが、CLI C evidenceは全て0件
だった。

| evidence | 件数 | 判定 |
|---|---:|---|
| `evidence/cli-case-binding.json` | 0 | C1ケース凍結なし |
| `evidence/cli-probe.json` | 0 | 正常／不正極性実行なし |
| `evidence/help-binding.json` | 0 | C2方向別照合なし |
| `evidence/claims-binding.json` | 0 | C3対照表なし |
| `evidence/cli-assurance.json` | 0 | C1〜C4集約未実行 |

`filter_cloud_003`のgeneric acceptanceとterminal projectionの対照:

```json
{"event":"ultra_final_acceptance","assurance_level":"full","final_acceptance_status":"full_success","profile":"cli","profile_behavior_probe_status":"pass"}
{"event":"tui_command_stop","assurance_level":"static","assurance_reason":"cli_probe_not_run","final_acceptance_status":"full_success","profile":"cli","task_status":"completed (static assurance)"}
```

CLI-1投影はC evidence欠落を検出し、generic内部fullをterminal fullへ
昇格させなかった。これは契約§4準拠であり、偽成功ではない。

到達runについて:

- ケース束縛凍結: 0/2
- 正常／未知option極性両側: 0/2
- C2 help→実装、実装→help方向別結果: 0/2
- C3 README出力主張×実出力対照表: 0/2
- C4同一ケース再実行一致: 0/2
- C2/C3 nearest_miss: 0件

stats族はfinal acceptance到達0/3だったため、READMEの数値例をC3が
裁いたrunは0件。人手実行結果をC3 evidenceへ代用せず、数値例照合は
今回も未計測とする。

### 未配線の機序

production sourceを監査すると、`canonical_profile_name`は
`python` / `python-cli` / `py-cli` / `py`を`python-cli`へ写像するが
`cli`を含まない。このため`domain_profile("cli")`は専用profileでなく
generic fallbackになる。さらにE-3bの
`python_cli::runtime::run_manifest_checks`はproductionからの呼出しが
0件で、conformance testからのみ直接実行されている。

新machine class候補を
`cli_runtime_dispatch:c_checks_not_run_after_final_acceptance`とする。
今回の指示どおり計測・帰属だけを記録し、修正やclass registry登録は
行わない。

## 8. E-0装備

### 8.1 自動分類

logical run単位はknown 4 / UNKNOWN 2。known 4はfailed runの
`process_failure`、UNKNOWN 2はfailure kindを持たないcompleted runである。
物理行はworkspace原本とartifact copyを走査するためknown 8 /
UNKNOWN 4だった。

completed runのUNKNOWNはdeath classifierが成功終端classを持たないことに
よる。C runtime未配線はterminal stop patternの外側であり、上記machine
class候補として人手監査で採取した。

### 8.2 検収シート

acceptance sheetは6/6生成され、全runで`sheet_generated=true`。
シート自給率は100%（1 campaign、6 run）だった。

### 8.3 較正コーパス

C2/C3と`nearest_miss`は0件で、較正コーパスへの自動追加は0件。
worktreeへの自動追加もなかった。C runtime未起動のためであり、
未実行をviolationまたは`claims_absent`へ読み替えていない。

## 9. 死因と帰属

failed 4本はREADME過制約の再発ではない。一次workspaceとverifyを対照し、
モデル生成phase間のinterface / fixture / smoke expectation不一致として
model帰属を裁定した。

- `stats_cloud_001`: 実装はpositional CSV pathだが後続verifyが
  `--input`を要求。
- `stats_cloud_002`: 実装は`--input`だが後続verifyがpositional pathを投与。
- `stats_cloud_003`: sampleにない`amount`列を後続verifyが要求。
- `filter_cloud_002`: sampleは大文字`ERROR`を3行含むがsmokeは小文字
  `error`を検索し、2件を期待。

代表停止原文:

```text
main.py: error: unrecognized arguments: --input
```

```text
Error: Column 'amount' not found in data/sample.csv.
```

```text
Error: Expected 2 lines, got 0
Error: Expected count '2', got '0'
```

harnessは不一致をbounded repair後も成功へ読み替えず、product exit 1を
保持した。completed 2本にdeathはないが、C dispatch欠落はmachine帰属で
P1-aを落とす。

## 10. 族差、cloud値札、コスト

- stats族: completed `0/3`、full `0/3`、平均579.0秒
- filter族: completed static `2/3`、full `0/3`、平均816.0秒
- cloud列全体: completed static `2/6`、full `0/6`、平均697.5秒
- C2 help照合: eligible 2、executed 0、pass 0、fail 0
- C3 claims binding: eligible 2、executed 0、pass 0、fail 0

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| preflight開始 | 1784985361 | 2026-07-25 22:16:01 |
| run列開始 | 1784985463 | 2026-07-25 22:17:43 |
| 最終run終端 | 1784989648 | 2026-07-25 23:27:28 |
| 監査記録終端 | 1784992656 | 2026-07-26 00:17:36 |

- preflight開始→最終run終端: 4287秒
- 6 run所要合計: 4185秒
- preflight開始→監査記録終端: 7295秒

## 11. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | harness completed 6/6、exit 1を4件保持 |
| P0-b 契約§4準拠 | **PASS** | C1未実行6/6をstatic (`cli_probe_not_run`)へ投影 |
| P0-c 偽成功ゼロ | **PASS** | terminal full 0、completed 2件もstaticを明記 |
| P1-a 到達runでC1実行 | **FAIL** | final acceptance到達2、C1実行0 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- honest terminal: `6/6`
- full率: `0/6`
- completed static率: `2/6`
- false full: `0/6`
- C1〜C4到達可能run: `2/6`
- C1〜C4実行run: `0/6`
- シート自給率: `6/6`
- admission cap解除: `6/6`
- 新terminal death class: 0
- 新machine class候補: 1

## 12. Scrubと監査境界

- 正式campaign各runのscrubは6/6
  `ok=true / findings=[] / allow=[]`。
- cloud資格情報値、raw console log、runtime `.anvil/` state、生成workspace
  はコミット対象にしていない。
- repositoryには本UATのscrub済みsummary/reportだけを追加し、過去記録を
  上書きしていない。

一次資料SHA-256:

- `uat-meta.json`:
  `83f81a9effa5ce47b5dbcc753e97224dbe251fbb4f8fe345eae44cd96b43ca11`
- `report-skeleton.md`:
  `cc2bc2ec349ec143d07c24f25aea387e688b30625d4c1a0744119365a3b8dcaa`
- `failure-classes.md`:
  `8714a4beb7c7ef2154d594789e3e9fcc9a7c4f5615e2780f8ba34ac97c7d1d3a`

Follow-up候補は、`profile=cli`のfinal behavior dispatchをE-3b
`run_manifest_checks`へ束縛し、到達runでC1〜C4 evidenceを必ず生成する
こと。今回のCLI-2範囲では実装しない。
