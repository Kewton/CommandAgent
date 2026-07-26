# uat-test0725-cli-elev-004: CLI-4 C部品初陣較正後 elevated再計測

実施日: 2026-07-26 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

正式計測revision: `d854c0571375535ffd13686d98283d4b509efe19`
(`develop`)

## 結論

**CLI-4の較正は成立した。UATはP0-a/b/c、P1-a/bがすべてPASS。
製品結果はfailed 6/6、full 0/6。**

正式campaignは6/6を正直終端させ、effective executor
`gemma4:31b-cloud`、local planner `qwen3.6:27b-coding-nvfp4`を6/6確認した。
empty workspace無垢性、検収シート生成、資格情報scrubはいずれも6/6成立。
環境中断、追加再実行、人手terminal切替は0件だった。

final acceptance到達はfilter族2/6。到達2/2でC1〜C4 evidenceが生成され、
C1正常exit 0 / 不正exit 2、C2両方向pass、C4再実行一致を得た。C3は
README出力主張6件を実行照合し、不一致6/6をliveで拒否した。terminalは
2/2とも`failed (cli_assurance_failed)`で、偽成功は0件。

C3不一致後のrepair targetは2/2で`cli/main.py`と`README.md`だけだった。
Next.js artifact参照は0件で、profile境界の遮断もlive成立した。

## 1. CLI-4実装

### 1.1 C1ケース記法正規化

コミット `4305cd6` (`Normalize measured CLI usage cases`)。

- `[optional]` groupを決定的に除去する。
- `<file_path>`を同梱`data/` sample pathへ束縛する。
- `<search_string>`をsample内の決定的な実在tokenへ束縛する。
- column placeholderはCSV headerの先頭列へ束縛する。
- 正規化後argvに`[]<>`が残ることを禁止する。
- sampleで束縛不能、未知placeholder、曖昧な記法は
  `case_extraction_failed`で正直終端し、凍結evidenceを作らない。

elev-003実README原文:

```text
python3 cli/main.py <file_path> --pattern <search_string> [--count]
```

fixtureの正規化後argv:

```json
["data/sample.txt", "--pattern", "Apple"]
```

READMEと`cli/main.py`はelev-003 artifactのbyte copy、sample本文も同じ実測
原文（repository fixtureの末尾改行だけ正規化）で、合成fixtureは使って
いない。

### 1.2 C2方向2の解剖と是正

コミット `4133078` (`Anchor unknown-option probes to normal argv`)。

elev-003実測原文:

```json
{"direction":"implementation_to_help","option":"--anvil-invalid-probe","args":["--anvil-invalid-probe"],"exit_code":2,"stderr":"main.py: error: the following arguments are required: file, --pattern","ok":false,"nearest_miss":{"candidate":"--pattern","edit_distance":17}}
```

照合器は契約どおり`unrecognized`系を要求し、観測されたrequired-argument
errorを正しくfailにしていた。実CLIもargparse標準動作だった。機序は
machine側のprobe形で、未知optionを単独投与したため必須引数検証に
遮られていた。

比較器の厳格性は変えず、凍結済み正常argvへ
`--anvil-invalid-probe`を追加する形へ変更した。新scopeは
`unknown_option_rejection_with_bound_normal_argv_v1`。

### 1.3 C3出力例抽出

コミット `52f9a97` (`Extract measured CLI output examples`)。

次の決定的規則だけを追加した。

1. 起動例と同一console block内で、command直後に続く出力行。
2. 起動例block直後の`Output` / `Expected Output` / `出力` /
   `出力例` / `実行結果` labelと、その直後のfenced block。

各claimは元commandを隔離実行し、stdoutをclaim行と完全照合する。
照合器の厳格性は不変。

elev-003実README fixtureは2件を抽出し、2件ともviolationとして固定した。

| command | README claim | 実stdout | 判定 |
|---|---|---|---|
| `data/sample.txt --pattern apple` | `I like apple.` / `Apple is red.` / `An apple a day keeps the doctor away.` | `I like apple pie.` | violation |
| `data/sample.txt --pattern banana --count` | `2` | `0` | violation |

### 1.4 周辺gap

コミット `d854c05` (`Contain CLI repair and collect C calibration`)。

- 全profile共通default repair候補へNext.js artifactを無条件追加していた
  `src/planner/repair_targeting.rs`のfallbackを特定した。
- 新leaf `src/planner/repair_targeting/cli.rs`で、canonical
  `profile=cli`は実在する`cli/main.py`、`README.md`、`USAGE`だけを候補に
  する。
- measured final-repair fixtureで、CLI target選択とNext.js候補0件を固定。
- calibration collectorへC2 `bindings[].nearest_miss`とC3
  `output_claims[].nearest_miss`を追加。
- elev-003のC2流出1件を遡及追記し、再実行`appended 0`を確認。

台帳には「C部品初陣の較正一括——E2（49偽陽性）/I2（認識錨）と同じ
通過儀礼。C4とC2方向1は初陣成立」と記録した。

## 2. 実装verification

正式計測前の権限付き結果:

| check | 結果 |
|---|---|
| C1 focused | 5 passed / 0 failed |
| C2 focused | 5 passed / 0 failed |
| CLI conformance | 4 passed / 0 failed |
| repair targeting | 10 passed / 0 failed |
| corpus regression | green |
| generality guardrails | 9 passed / 0 failed |
| Python focused | 20 passed / 0 failed |
| 測定後処理Python focused | 22 passed / 0 failed |
| Ruff check / format check | exit 0 / exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --all-targets -- -D warnings` | exit 0 |
| 権限付きfull `cargo test -- --format=terse` | 1800 passed / 30 ignored / 0 failed |

bench所有preflight:

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD / minimum ancestor | `d854c05` / `704d7f9` verified |
| `cargo test` | exit 0 |
| release build | exit 0 |
| binary | `commandagent 0.1.0 d854c05 2026-07-26T08:07:35Z` |
| `NODE_ENV` | `production` |

## 3. Campaign境界

- id: `cli-create-elevated-20260726-080611`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0725_cli_elev4`
- suite: `cli-create-elevated`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`
- admission: admitted
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gemma4:31b-cloud` / `ollama` 6本
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

empty無垢性は6/6で`created=true / checked=true / empty=true /
entry_count=0`。

## 4. Run行列

`—`はfinal acceptance未到達。

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / 帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---:|
| `stats_cloud_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 377 |
| `stats_cloud_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 656 |
| `stats_cloud_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `cli_verify:canonical_command_dropped_positional_input` / machine | 863 |
| `filter_cloud_001` | filter | failed | failed (`cli_assurance_failed`) | pass | pass | fail (3/3 reject) | pass | `cli_claims_binding:readme_output_claim_fabricated` / model | 693 |
| `filter_cloud_002` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | 558 |
| `filter_cloud_003` | filter | failed | failed (`cli_assurance_failed`) | pass | pass | fail (3/3 reject) | pass | `cli_claims_binding:readme_output_claim_fabricated` / model | 592 |

static 4件はadmission capではなく、final acceptance未到達を契約§4へ
投影した`cli_probe_not_run`。C3違反2件は契約§4どおりfailedで、partial /
fullへの漏出は0件。

## 5. 実効モデル

6枚の自動検収シートと`run_start`はすべて次で一致した。

```text
executor          gemma4:31b-cloud / ollama
planner           qwen3.6:27b-coding-nvfp4 / ollama
profile           cli
```

実効モデル監査は6/6、executor差し替わり0件。

## 6. C1とC4のlive evidence

### filter_cloud_001

ケース束縛:

```json
{"id":"normal","args":["--pattern","2023-10-01","data/sample.txt"],"source":"README.md:8"}
```

極性と再実行:

| case | argv | exit | stdout |
|---|---|---:|---|
| normal | `--pattern 2023-10-01 data/sample.txt` | 0 | sampleの該当10行、508 bytes |
| invalid | `--anvil-invalid-probe` | 2 | empty |
| normal-rerun | normalと同一 | 0 | normalとbyte一致 |

`c1_ok=true`、`c4_ok=true`、`binding_intact=true`。

### filter_cloud_003

```json
{"id":"normal","args":["--pattern","apple"],"source":"README.md:8"}
```

normal exit 0、invalid exit 2、normal-rerun exit 0。stdout / stderrは
normalとrerunで同一。正規化後argvに`[]<>`は残らなかった。

campaign全体ではC1 eligible 2 / executed 2 / pass 2、C4 eligible 2 /
executed 2 / pass 2。`case_extraction_failed`は0件。

## 7. C2両方向のlive evidence

到達2runはいずれもhelp option
`--count`, `--help`, `--pattern`, `-h`を抽出した。

filter_cloud_001の方向別原文要点:

| direction | option | argv | exit | 判定 |
|---|---|---|---:|---|
| help→implementation | `--count` | `--count` | 2 | pass（required error、unrecognizedなし） |
| help→implementation | `--help` | `--help` | 0 | pass |
| help→implementation | `--pattern` | `--pattern` | 2 | pass（value required、unrecognizedなし） |
| help→implementation | `-h` | `-h` | 0 | pass |
| implementation→help | `--anvil-invalid-probe` | `--pattern 2023-10-01 data/sample.txt --anvil-invalid-probe` | 2 | pass |

方向2 stderr:

```text
main.py: error: unrecognized arguments: --anvil-invalid-probe
```

filter_cloud_003も正常argv
`--pattern apple --anvil-invalid-probe`へ投与しexit 2、
`unrecognized arguments`を観測した。

- help→implementation: 8/8 pass
- implementation→help: 2/2 pass
- C2 aggregate: 2/2 pass
- C2 nearest_miss: 0

elev-003で失敗した必須引数による遮蔽はlive 0件。

## 8. C3によるREADME捏造拒否

### filter_cloud_003実物

| source | argv | README claim | 実stdout | 判定 |
|---|---|---|---|---|
| `README.md:22->26` | `data/sample.txt --pattern apple` | `I like apple.` / `Green apple is sour.` / `Apple pie is delicious.` | `apple: This is a red apple.` / `apple: Another apple here.` | violation |
| `README.md:34->38` | `data/sample.txt --pattern error --count` | `2` | `3` | violation |
| `README.md:43->47` | `--help` | optional `--pattern`、省略形help | required `--pattern`の実help | violation |

### filter_cloud_001実物

| source | README claim | 実stdout | 判定 |
|---|---|---|---|
| `README.md:24->28` | 存在しない2件のERROR log | empty | violation |
| `README.md:35->39` | `Match count: 5` | `0` | violation |
| `README.md:44->48` | README記載help全文 | 実argparse help全文（不一致） | violation |

各claimは独立の実行観測を保持し、exit code、stdout / stderr、duration、
truncationをevidence化した。

- C3 eligible / executed: 2/2
- extracted claims: 6
- matched: 0
- violations: 6
- README捏造のlive拒否: 6/6

C3違反がterminal failedを生んでおり、偽成功は0件。

## 9. Assuranceとrepair境界

到達2runの`cli-assurance.json`は同形:

```json
{
  "status": "failed",
  "assurance": "failed",
  "evidence": {
    "probe_attempted": true,
    "binding_intact": true,
    "checks": {
      "cli_output_claims": "failed",
      "cli_probe": "pass",
      "cli_rerun_consistency": "pass",
      "help_binding": "pass"
    }
  },
  "reasons": ["cli_output_claims:observed_stdout_mismatch"]
}
```

final repair開始は2/2とも次の境界だった。

```json
{"selection_reason":"fallback","selected_target":"cli/main.py","selected_targets":["cli/main.py","README.md"]}
```

Next.js / package / tailwind / postcss artifactは選択0件。repairはC3不一致を
消せず`model_stagnation:no_progress_recorded`で正直終端した。

## 10. 自動分類と新クラス

測定後classifierをlogical run単位へ正規化し、workspace原本とartifact
copyの二重表示を除去した。

- known: 6
- UNKNOWN: 0
- model: 5
- machine: 1

新規クラス:

```toml
id = "cli_claims_binding:readme_output_claim_fabricated"
attribution = "model"
first_seen = "uat-test0725-cli-elev-004"
```

これは較正前の抽出欠落machine classと、較正後にC3が正当に検出した
モデル生成内容不一致を分離する。

```toml
id = "cli_verify:canonical_command_dropped_positional_input"
attribution = "machine"
first_seen = "uat-test0725-cli-elev-004"
```

`stats_cloud_003`では直前の実command
`python cli/main.py data/sample.csv --column amount`が成功した一方、
verify正準化後は`python cli/main.py --column price`となりCSV位置引数を
欠落させた。artifactが既に要件を満たす可能性をruntime自身も
`verify_command_false_negative`として記録した。修正は今回の範囲外。

他の早期停止:

- `stats_cloud_001`: modelが存在しない`amount`列をverifyへ選択。
- `stats_cloud_002`: model生成smokeの期待表記と実出力表記が不一致。
- `filter_cloud_002`: modelがheredoc入りverify commandを3回生成し、
  policyが正当に拒否。

## 11. 較正コーパス

bench自動collectorは到達2runのC3 nearest_missを収集した。初回実測で
workspace原本とartifact copyを別recordとする二重蓄積が判明したため、
logical campaign / run / evidence / claim indexをrecord IDとする
非緩和の重複正規化を追加した。

実演:

```text
appended 6 records
appended 0 records
```

current corpus:

```text
c2 / violation: 1
c3 / violation: 6
e2 / matched: 8
e2 / violation: 3
i2 / matched: 20
i2 / violation: 6
total: 44
```

6件は全てlive C3 violationで、matchedへの読み替えやclaimの統合はして
いない。

## 12. シート、scrub、コスト

- acceptance sheet: 6/6
- sheet self-supply: 100%（1 campaign、6 run）
- per-run scrub: 6/6 `ok=true / findings=[]`
- campaign scrub: `ok=true / findings=[]`
- cloud credential値、raw console log、runtime `.anvil/`、生成workspaceは
  repositoryへコミットしない。

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| run列開始 | 1785053273 | 2026-07-26 17:07:53 |
| 最終run終端 | 1785057012 | 2026-07-26 18:10:12 |
| 監査記録 | 1785057409 | 2026-07-26 18:16:49 |

- 6run所要合計 / run列wall: 3739秒
- stats族平均: 632.0秒
- filter族平均: 614.3秒
- 全体平均: 623.2秒
- stats full: 0/3
- filter full: 0/3
- elevated cloud full: 0/6

## 13. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | harness completed 6/6、product exit 1を6件保持 |
| P0-b 契約§4準拠 | **PASS** | 未到達4件static、C3違反2件failed |
| P0-c 偽成功ゼロ | **PASS** | terminal full 0、failed 6/6 |
| P1-a 到達runでC1〜C4実行 | **PASS** | 到達2/2で4 evidence実在 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- honest terminal: 6/6
- full率: 0/6
- failed率: 6/6
- false full: 0/6
- C1 pass: 2/2 reached
- C2 pass: 2/2 reached
- C3 violation detection: 6/6 claims
- C4 pass: 2/2 reached
- sheet self-supply: 6/6
- effective model audit: 6/6
- admission static cap解除: 6/6
- new model class: 1
- new machine class: 1

## 14. 一次資料SHA-256

- `uat-meta.json`:
  `366e5bfa2e134f1434bee632308fb346c6b68346a3c19d827917fecc36d78278`
- `report-skeleton.md`:
  `5157ca50e1ff766ef717ae132b4d32d3417cba7d4c7866e47041e4fa41671a68`
- `failure-classes.md`:
  `7cfe2fb02ee3879ad892336814ecedab60f99e4a6e80063892fc18b12175596e`
- `filter_cloud_001/cli-case-binding.json`:
  `caf812d5eadbe0392159b99833ad32bc9225b04211c815cf812079a5cd1dad4c`
- `filter_cloud_001/cli-probe.json`:
  `3457c9a25fdde9f0a7b870f31002104cb2f5ec65d3b2da9ed2dc94fca63a9f5a`
- `filter_cloud_001/help-binding.json`:
  `43629f057eb17315aa61d51b869eabf14df0ecc63beb85a7785779531e94d274`
- `filter_cloud_001/cli-assurance.json`:
  `920fd009279aca27b747bd4ad1732c14e19742c2f1045164e7ce26149824062a`
- `filter_cloud_003/cli-case-binding.json`:
  `81d1be0dc60003f47f13d273d0341d722624d8d22d452b93a813dd4242302ca7`
- `filter_cloud_003/cli-probe.json`:
  `985515cfdeaef238458655e2feb9fa518818ddaed2b59632c01155280c903beb`
- `filter_cloud_003/help-binding.json`:
  `cfcb29d771d1e0d49f708f87a0b36e82fa7382f4a84e6efd2d997ba8f01209e1`
- `filter_cloud_003/cli-assurance.json`:
  `920fd009279aca27b747bd4ad1732c14e19742c2f1045164e7ce26149824062a`
