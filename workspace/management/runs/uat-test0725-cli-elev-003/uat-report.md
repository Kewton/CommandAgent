# uat-test0725-cli-elev-003: CLI-3 runtime配線後 elevated再計測

実施日: 2026-07-26 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

正式計測revision: `917acef305102a8ea3ae77a19c330cca38285cd6`
(`develop`)

## 結論

**CLI-3のproduction配線と起動実在保証は成立した。UATは
P0-a/b/c、P1-a/bがすべてPASS。製品結果はfailed 6/6、full 0/6。**

正式campaignは6/6を正直終端させ、effective executor
`gemma4:31b-cloud`、local planner `qwen3.6:27b-coding-nvfp4`を
`run_start`で6/6確認した。empty workspace無垢性、検収シート生成、
資格情報scrubはいずれも6/6成立した。環境中断・campaign再実行・人手
terminal切替は0件だった。

final acceptance到達は`filter_cloud_002`の1/6。このrunでは
`profile_behavior_probe`が発火し、C1〜C4の4 evidenceがproduction経路
から生成された。C1 polarityとC2方向2が失敗し、C3は
`claims_absent`、C4はpass。terminal assuranceは契約§4どおり
`failed (cli_assurance_failed)`になった。残る5runはfinal acceptance前に
停止し、`static (cli_probe_not_run)`である。偽成功とterminal fullは0件。

実弾で新たに3つのmachine gapを採取した。

1. READMEの先頭usage templateが具体例より先に束縛され、C1正常系を
   非実行可能なplaceholder引数にした。
2. C2の未知option単独投与はargparseの必須引数エラーに遮られ、
   unknown rejectionを観測できなかった。
3. C失敗後のfinal repair fallbackがCLI workspaceにNext.js artifact群を
   要求した。

またC2 `nearest_miss`は1件生成されたが、較正collectorがCLI evidenceの
`bindings` / `output_claims`形を読まないため、durable corpus追記は0件
だった。いずれも今回の計測では修正せず、レビュー入力として記録する。

## 1. CLI-3コミット1

コミット `917acef305102a8ea3ae77a19c330cca38285cd6`
(`Wire CLI runtime into final acceptance`)。

未配線地点は`src/planner/runner.rs:5471`だった。従来はfinal acceptanceの
profile behavior境界から`domain_profile(profile).behavior_probe(...)`を
直接呼び、`profile=cli`はgeneric fallbackへ落ちていた。新しいleaf
`src/planner/profile_behavior.rs:13`を経由させ、canonical profileが
`cli`なら`python_cli::runtime::run_manifest_checks(root)`を呼ぶ。
`python-cli`等の既存profileは従来のdomain behaviorへ流し、既存セルの
経路を維持した。

起動実在テストはtest double境界でなく、実際の
`ultra_final_acceptance_report(profile=cli)`を通す。実Python subprocess、
`profile_behavior_probe` event、次の4 evidenceを確認する。

- `evidence/cli-case-binding.json`
- `evidence/cli-probe.json`
- `evidence/help-binding.json`
- `evidence/cli-assurance.json`

正例はC1正常exit 0、不正exit nonzero、C1〜C4全passとfinal acceptance
passを確認する。握りつぶしCLIの負例はC1不正exit 0を検出し、final
acceptance failureでも4 evidenceが全て残ることを確認する。このテストは
README structural verifyの正準化とは別のproduction acceptance経路を
直接固定する。

registryには次を追加した。

```toml
id = "cli_runtime_dispatch:c_checks_not_run_after_final_acceptance"
attribution = "machine"
first_seen = "uat-test0725-cli-elev-002"
ledger_ref = "CLI-3"
note = "profile=cliのfinal acceptanceがE-3b C1〜C4 runtimeを起動しなかった。CLI-3でproduction経路へ配線済み"
```

台帳には「配線の実在病の反復(D-3a-2円環→CLI-3)」として、conformanceは
部品、production経路テストは起動を検証する二層を記録した。

E-1 scaffoldのgeneratorとchecked-in admission templateには
`every verification component has a production acceptance-path activation
test`を追加した。CLIは実装済みとしてchecked、demo / investigateは
未実装としてuncheckedのままである。

## 2. 権限付きverificationとpreflight

production-code変更後に次を実行した。

| check | 結果 |
|---|---|
| production activation focused | 2 passed / 0 failed |
| CLI conformance | 3 passed / 0 failed |
| generality guardrails | 9 passed / 0 failed |
| scaffold unittest | 3 passed / 0 failed |
| Ruff check / format check | exit 0 / exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| 権限付きfull `cargo test -- --format=terse` | 1793 passed / 30 ignored / 0 failed |
| runner growth guardrail | baseline変更なし、green |

起動実在テスト名:

```text
cli_final_acceptance_production_path_executes_manifest_c1_through_c4
cli_final_acceptance_failure_still_persists_c_evidence
```

bench所有preflight実測:

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD / minimum ancestor | `917acef` / `704d7f9` verified |
| `cargo test` | exit 0 |
| release build | exit 0 |
| binary | `commandagent 0.1.0 917acef 2026-07-25T15:57:46Z` |
| `NODE_ENV` | `production` |

## 3. Campaign境界

- id: `cli-create-elevated-20260725-155514`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0725_cli_elev3`
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

`—`はfinal acceptance未到達を表す。

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / 帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---:|
| `stats_cloud_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `model_stagnation_read_only` / model | 374 |
| `stats_cloud_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 400 |
| `stats_cloud_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 490 |
| `filter_cloud_001` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 246 |
| `filter_cloud_002` | filter | failed | failed (`cli_assurance_failed`) | fail | fail | claims absent | pass | `cli_final_repair:nextjs_fallback` / machine（終端） | 1197 |
| `filter_cloud_003` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 469 |

自動classifierは`filter_cloud_002`を形状既定の`process_failure / model`と
した。しかし一次eventではC失敗を正しく検出した後、repair targetが
CLIに無関係なNext.js artifactへfallbackして終端しているため、終端死因を
machineへレビュー裁定した。C失敗の入力側にはモデル生成READMEと
machine verifier双方の問題があり、下記で分離する。

admission由来の`profile_not_admitted`は0/6でstatic capは解除済み。
5件のstaticはfinal acceptance未到達を契約§4へ投影した
`cli_probe_not_run`であり、admission capではない。

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

## 6. C runtimeの起動実在

`filter_cloud_002`のproduction event原文:

```json
{"cycle_index":0,"event":"profile_behavior_probe","evidence_path":"evidence/cli-assurance.json","ok":false,"profile":"cli","reasons":["cli_probe_polarity_violation","help_binding:implementation_to_help:option=--anvil-invalid-probe"],"status":"failed"}
```

final acceptance集約:

```json
{"event":"ultra_final_acceptance","profile":"cli","profile_behavior_probe_status":"failed","profile_behavior_probe_evidence_path":"evidence/cli-assurance.json","final_acceptance_status":"incomplete","assurance_level":"partial","primary_reason":"cli_probe_polarity_violation; help_binding:implementation_to_help:option=--anvil-invalid-probe"}
```

terminal projection:

```json
{"event":"tui_command_stop","profile":"cli","status":"failed","task_status":"failed","final_acceptance_status":"incomplete","assurance_level":"failed","assurance_reason":"cli_assurance_failed"}
```

production event発火1/1、4 evidence生成1/1であり、CLI-3の起動実在は
実キャンペーンでも成立した。failure時にもevidenceは失われていない。

## 7. C1: argv probe

### 7.1 ケース束縛の凍結

`cli-case-binding.json`原文:

```json
{
  "entry": "cli/main.py",
  "cases": [
    {
      "id": "normal",
      "args": ["<file_path>", "--pattern", "<search_string>", "[--count]"],
      "expected_stdout": [],
      "source": "README.md:8"
    },
    {
      "id": "invalid",
      "args": ["--anvil-invalid-probe"],
      "expected_stdout": [],
      "source": "contract:deterministic-invalid-option"
    }
  ]
}
```

`binding_intact=true`。README line 8の一般usage
`python3 cli/main.py <file_path> --pattern <search_string> [--count]`が、
後続の具体例より先に機械束縛された。

### 7.2 極性両側とC4

`cli-probe.json`の全観測:

| case | argv | exit | stdout | stderr |
|---|---|---:|---|---|
| normal | `<file_path> --pattern <search_string> [--count]` | 2 | empty | `main.py: error: unrecognized arguments: [--count]` |
| invalid | `--anvil-invalid-probe` | 2 | empty | `main.py: error: the following arguments are required: file, --pattern` |
| normal-rerun | normalと同一 | 2 | empty | normalと同一 |

各観測は`outcome=exited`、`duration_ms=30`、出力truncationなし。

- C1 normal polarity: fail (`exit 2`, expected 0)
- C1 invalid polarity: pass (`exit 2`, expected nonzero)
- C1 aggregate: `c1_ok=false`
- C4: `c4_ok=true`
- failure kind: `cli_probe_polarity_violation`

新machine gap候補:
`cli_case_binding:first_usage_placeholder_masks_concrete_examples`。
READMEのrepresentative case抽出が先頭のtemplateを実値化せず字義実行し、
line 21 / 32の具体的な実行例へ到達しなかった。

## 8. C2: help binding（936行の実弾初戦）

`--help`はexit 0で、次を抽出した。

```text
usage: main.py [-h] --pattern PATTERN [--count] file

positional arguments:
  file               Path to the input text file

options:
  -h, --help         show this help message and exit
  --pattern PATTERN  The search string to filter lines
  --count            Display only the number of matching lines
```

抽出optionは`--count`, `--help`, `--pattern`, `-h`。

| direction | option | exit | 判定 | stderr要点 | nearest_miss |
|---|---|---:|---|---|---|
| help→implementation | `--count` | 2 | pass | required: `file, --pattern` | null |
| help→implementation | `--help` | 0 | pass | empty | null |
| help→implementation | `--pattern` | 2 | pass | expected one argument | null |
| help→implementation | `-h` | 0 | pass | empty | null |
| implementation→help | `--anvil-invalid-probe` | 2 | fail | required: `file, --pattern` | `--pattern`, distance 17 |

C2はhelp→implementation 4/4 pass、implementation→help 0/1 pass、
aggregate fail。方向2のscopeは
`unknown_option_rejection_only_v0`。未知optionは非0で拒否されたが、
argparseが必須引数不足を先に報告し、`unrecognized`形を観測できなかった。

新machine gap候補:
`cli_help_binding:required_args_mask_unknown_option_rejection`。
未知optionを束縛済み正常argvへ追加せず単独投与するため、必須引数を持つ
標準argparse CLIで方向2がfalse negativeになり得る。

## 9. C3: README出力主張×実出力

runtime evidenceは次のとおり。

```json
{
  "output_claims": [],
  "checks": {
    "cli_output_claims": "claims_absent"
  }
}
```

C3はexecutedだが`claims_absent`で、pass / violationの対照行は0件。
原因はC1と同じく、束縛されたREADME line 8にexpected stdoutがなく、
line 21以降の具体例と数値主張を抽出対象にしなかったこと。

C3 evidenceの代用にはしないが、見逃しの規模を一次workspaceで手動監査
した。

| README claim | README expected | 同梱sampleでの実出力 | 実一致 |
|---|---|---|---|
| `--pattern apple` | `I like apple.` / `Apple is red.` / `An apple a day keeps the doctor away.` | `I like apple pie.` | no |
| `--pattern banana --count` | `2` | `0` | no |

CLI実装はcase-sensitiveで、sampleは`Apple`, `Apples`, `Banana`,
`Bananas`を含む。READMEの具体出力は2/2不一致だが、今回のC3はそれを
裁かなかった。新machine gap候補を
`cli_claims_binding:concrete_examples_unbound_after_template`とする。

## 10. CLI assurance集約

`cli-assurance.json`原文:

```json
{
  "status": "failed",
  "assurance": "failed",
  "evidence": {
    "probe_attempted": true,
    "binding_intact": true,
    "checks": {
      "cli_output_claims": "claims_absent",
      "cli_probe": "failed",
      "cli_rerun_consistency": "pass",
      "help_binding": "failed"
    }
  },
  "reasons": [
    "cli_probe_polarity_violation",
    "help_binding:implementation_to_help:option=--anvil-invalid-probe"
  ]
}
```

契約§4の「C1実行・極性違反または照合違反→failed」がterminalへ反映
され、generic final acceptance event内のpartialをterminal fullへ
漏らしていない。

## 11. Final repair死因

C failureまでは正しいhonest failureだった。その後のbounded repair開始
原文:

```json
{"attempt":1,"event":"final_acceptance_repair_start","repair_target":"test_or_evidence","selection_reason":"fallback","selected_target":"src/app/page.tsx","selected_targets":["src/app/page.tsx","src/app/page.jsx","src/app/page.ts","src/app/page.js","app/page.tsx","app/page.jsx","app/page.ts","app/page.js","pages/index.tsx","pages/index.jsx","pages/index.ts","pages/index.js","src/pages/index.tsx","src/pages/index.jsx","src/pages/index.ts","src/pages/index.js","package.json","tsconfig.json","postcss.config.js","postcss.config.mjs","tailwind.config.js","tailwind.config.ts"]}
```

終端原文:

```text
artifact_follow_through_exhausted: missing expected paths:
src/app/page.ts, src/app/page.js, app/page.tsx, app/page.jsx, ...
package.json, tsconfig.json, postcss.config.js, postcss.config.mjs,
tailwind.config.js, tailwind.config.ts
```

CLI C失敗にNext.js artifact fallbackを選んだため、終端死因はmachine。
新class候補:
`cli_final_repair:nextjs_artifact_fallback_after_c_failure`。
今回のproduction配線、C evidence、failed投影の成立とは分離して記録し、
修正は行わない。

## 12. E-0装備

### 12.1 自動分類

logical run単位はknown 6 / UNKNOWN 0。

- `model_stagnation_read_only`: 1
- `process_failure`: 5

classifierはworkspace原本とartifact copyを走査するため、物理行は
known 12 / UNKNOWN 0。`process_failure`のmodel帰属はregistry上の仮置き
なので、`filter_cloud_002`だけは一次eventに基づき上記machine終端へ
レビュー裁定した。

### 12.2 検収シート

acceptance sheetは6/6生成され、全runで`sheet_generated=true`。
シート自給率は100%（1 campaign、6 run）。

### 12.3 較正コーパス

C2 nearest_missはlogical 1件
(`candidate=--pattern`, `edit_distance=17`)。workspaceとartifact copyには
同一recordが各1件存在する。

しかし`workspace/management/calibration/`への追加は0件で、campaign idを
含むrecordも0件。自動collectorはtop-level `claims`だけを収集し、
CLI C2の`bindings[].nearest_miss`とC3の`output_claims`を受け付けない。
新machine class候補:
`bench_calibration_corpus:cli_evidence_shape_unsupported`。

## 13. 早期停止5runの死因

一次workspaceの実装、sample、model生成verifyを対照した。

- `stats_cloud_001`: sampleは`10.5,20.0,15.2,30.8,25.0`だが、
  smokeは`10,20,30,40,50`由来のsum 150 / average 30をassertした。
  repairは`smoke_check.py`を読んだまま変更せず
  `model_stagnation:read_only_loop`。model帰属。
- `stats_cloud_002`: 実装は`--file` option、testはpositional file。
  sampleは`100..500`だがtestは`10..50`を期待し、さらに余分な
  末尾backtick付き`cli/main.py`ファイルも生成した。model帰属。
- `stats_cloud_003`: sample / smokeは`score`列で整合していたが、後続
  verifyが存在しない`value`列を投与した。harnessはこれを
  `dependency_setup_authority_required`へ分類したが、入力不一致の起点は
  model生成verify。model帰属。
- `filter_cloud_001`: sampleには`apple`が0行なのにverifyは2行を期待。
  model帰属。
- `filter_cloud_003`: sampleは`Apple` / `Apple Pie`、実装とsmokeは
  case-sensitiveな`apple`を使い、smokeは空stdoutで失敗。model帰属。

全件、bounded repair後も失敗を成功へ読み替えずproduct exit 1を保持した。

## 14. 族差、cloud値札、コスト

- stats族: full `0/3`、平均421.3秒
- filter族: full `0/3`、平均637.3秒
- cloud列全体: full `0/6`、平均529.3秒
- C1: eligible 1、executed 1、pass 0、fail 1
- C2: eligible 1、executed 1、pass 0、fail 1
- C3: eligible 1、executed 1、claims absent 1、claim対照0
- C4: eligible 1、executed 1、pass 1、fail 0

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| preflight開始 | 1784994914 | 2026-07-26 00:55:14 |
| run列開始 | 1784995084 | 2026-07-26 00:58:04 |
| 最終run終端 | 1784998260 | 2026-07-26 01:51:00 |
| 監査記録終端 | 1784998600 | 2026-07-26 01:56:40 |

- preflight開始→最終run終端: 3346秒
- 6 run所要合計: 3176秒
- preflight開始→監査記録終端: 3686秒

## 15. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | harness completed 6/6、product exit 1を6件保持 |
| P0-b 契約§4準拠 | **PASS** | 未到達5件はstatic、C1/C2違反1件はfailed |
| P0-c 偽成功ゼロ | **PASS** | terminal full 0、failed 6/6 |
| P1-a 到達runでC1〜C4実行 | **PASS** | 到達1/1でevent発火、4 evidence実在 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- honest terminal: `6/6`
- full率: `0/6`
- failed率: `6/6`
- false full: `0/6`
- C1〜C4到達可能run: `1/6`
- C1〜C4実行run: `1/1`
- シート自給率: `6/6`
- admission cap解除: `6/6`
- new terminal machine class候補: 1
- new verifier / E-0 machine gap候補: 4

## 16. Scrubと監査境界

- 正式campaign各runのscrubは6/6
  `ok=true / findings=[] / allow=[]`。
- コミット対象の本report / summary bundleもbench scrub
  `ok=true / findings=[]`。
- cloud資格情報値、raw console log、runtime `.anvil/` state、生成workspace
  はコミット対象にしていない。
- repositoryには本UATのscrub済みsummary/reportだけを追加し、過去記録を
  上書きしていない。

一次資料SHA-256:

- `uat-meta.json`:
  `fca3a90cf2d0ea3ac7d7c32a7d85efb59f7923d2e97de350b1d53b0a700d088f`
- `report-skeleton.md`:
  `22b6620b2339782c81c5d62d3fb5b925d9ce7fcc6b776b7de21e1bdff7d84f3c`
- `failure-classes.md`:
  `d82db3dab0024739b4fb3043e31883a45a834a340249072d54c74de87a60af48`
- `cli-case-binding.json`:
  `8c129b10c91d2ca162e41a175a284aed78fc521523a714fc55aae4b4d1030795`
- `cli-probe.json`:
  `a78684a49d6e601c9a646c82d328fe19eb14818c48612dd17966a95188dc11b8`
- `help-binding.json`:
  `0d2132503ddc6c9da261b0478a2f4d50ef26f8febec08df346137bec0b63cc99`
- `cli-assurance.json`:
  `da1f5d588f553008e321980662d354319ba67125af57080307079fa0e4bd1437`
