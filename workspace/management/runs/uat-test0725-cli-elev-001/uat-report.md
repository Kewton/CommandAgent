# uat-test0725-cli-elev-001: cli×create elevated初計測

実施日: 2026-07-25 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

正式計測revision: `760d473f154fcb919c21e959181315e02364ad48`
(`develop`)

## 結論

**計測プロトコルとE-0装備はPASS、UAT総合はP0-b FAIL。製品結果は
6/6 failed、full相当0/6。**

正式campaignは6/6を正直終端させ、effective executor
`gemma4:31b-cloud`、local planner `qwen3.6:27b-coding-nvfp4`を
run_startで6/6確認した。empty workspace無垢性、検収シート生成、
資格情報scrubはいずれも6/6成立した。verdictを成功へ読み替えたrunは
0件である。

admission昇格後の実挙動は、旧`static (profile_not_admitted)`キャップが
6/6で解除された。一方、全runがfinal profile acceptance前に停止し、
C1〜C4 evidenceは0件だったにもかかわらず、terminal projectionは
6/6を`partial (acceptance_not_full_success)`と表示した。固定契約§4は
C1未実行を`static`と定めるため、これは機械側のassurance投影不整合で
ありP0-b FAILとする。

## 1. 先行値とコミット1

前回`7087babcd375d3d0be0afd6f28869e1a46999456`のpushに対する確定値:

| workflow | run id | status | conclusion | final update (UTC) |
|---|---:|---|---|---|
| CI | `30150268770` | completed | success | `2026-07-25T08:03:04Z` |
| acceptance | `30150268768` | completed | success | `2026-07-25T08:02:37Z` |

コミット1 `704d7f9`では次を確定した。

- local arm `uat-test0724-cli-001-v3`を
  `workspace/management/runs/band_summary_cli.md`へ集計:
  honest terminal 6/6、full 0/6、C到達0/6。
- 既存5バンドのSHA-256は生成前の凍結値と全件一致。
- runtime manifest、management scaffold、terminal admission lookupを
  `admitted`へ昇格。
- KPIのシート自給率を初計測1 campaign・6 runの100%へ更新。
- conformanceはfull正例1件と負例6件がgreen。

初回elevated campaignで`--profile cli` aliasがadmission lookupから
漏れていることを実測したため、`cli`と`python-cli`を同じadmitted
manifestへ束縛した。この修正はコミット1へ含め、guardrail baselineを
変更せず焦点テストとfull suiteを再実行した。

## 2. Campaign境界とpreflight

正式campaign:

- id: `cli-create-elevated-20260725-085827`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0724_cli_elev`
- suite: `cli-create-elevated`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`
- admission: admitted
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gemma4:31b-cloud` / `ollama` 6本

bench所有のpreflight実測:

| 項目 | 結果 |
|---|---|
| git status | clean |
| HEAD / minimum ancestor | `760d473` / `704d7f9` verified |
| `cargo test` | 1782 passed / 30 ignored / 0 failed |
| release build | exit 0 |
| binary | `commandagent 0.1.0 760d473 2026-07-25T08:59:51Z` |
| `NODE_ENV` | `production` |
| dry-run | 6/6 command表示、empty無垢性計画、exit 0 |

empty無垢性は全runで`created=true / checked=true / empty=true /
entry_count=0`だった。

## 3. 除外campaignと再実行

除外campaign `cli-create-elevated-20260725-083300`は、旧revision
`a31538c2ca833738ef53e70ea1044758adb87870`で開始した。

- `stats_cloud_001`: completed、product exit 1、963秒、
  `static (profile_not_admitted)`
- `stats_cloud_002`: alias defect確定後に
  `interrupted(environment)`、146秒
- 残り4本: pendingのまま

run1のterminal eventは`effective_profile=cli`だったが、admission
lookupは`python-cli`だけを認識していた。無効条件で残りを消費しないため
正直に中断し、alias修正・full suite green後に許可された新規campaign
1回を使用した。既存campaignは上書きしていない。

## 4. Run行列

`C1`〜`C4`の`—`は未実行を表す。全runがfinal profile acceptance前に
停止し、CLI C evidenceを生成していない。

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / 帰属 | 秒 |
|---|---|---|---|---|---|---|---|---|---:|
| `stats_cloud_001` | stats | failed | partial (`acceptance_not_full_success`) | — | — | — | — | `process_failure` / model | 982 |
| `stats_cloud_002` | stats | failed | partial (`acceptance_not_full_success`) | — | — | — | — | `process_failure` / model | 313 |
| `stats_cloud_003` | stats | failed | partial (`acceptance_not_full_success`) | — | — | — | — | `process_failure` / model | 315 |
| `filter_cloud_001` | filter | failed | partial (`acceptance_not_full_success`) | — | — | — | — | `process_failure` / model | 425 |
| `filter_cloud_002` | filter | failed | partial (`acceptance_not_full_success`) | — | — | — | — | `process_failure` / model | 432 |
| `filter_cloud_003` | filter | failed | partial (`acceptance_not_full_success`) | — | — | — | — | `process_failure` / model | 842 |

admissionキャップ解除の原文形:

```text
Task status: failed
Assurance: partial (acceptance_not_full_success)
```

`profile_not_admitted`は正式campaignの6runで0件だった。これはstatic
キャップの解除確認としては6/6成立するが、C1未実行からpartialを得た点は
契約適合ではない。

## 5. 実効モデル監査

各runの`run_start`から次の6行を機械抽出した。

```text
filter_cloud_001 gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
filter_cloud_002 gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
filter_cloud_003 gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
stats_cloud_001  gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
stats_cloud_002  gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
stats_cloud_003  gemma4:31b-cloud ollama qwen3.6:27b-coding-nvfp4 ollama cli
```

executor、provider、planner、profileは期待値と6/6一致した。

## 6. E-0装備の実戦検収

### 6.1 自動分類

logical run単位ではknown 6 / UNKNOWN 0。6本すべて登録済み
`process_failure`に一致し、attributionはmodelだった。

report skeletonの物理行はknown 12 / UNKNOWN 0である。分類器が
`workspaces/`原本と`artifacts/` copyの双方を走査するため、各logical
runを2回表示した。run集計はrun名で6本へ重複排除した。

新しいterminal death classは0件。一方、C1未実行のCLIをpartialへ投影する
cross-cuttingなmachine defectはterminal stop pattern分類の対象外であり、
登録簿の捕捉範囲に対するfollow-up候補である。

### 6.2 検収シート自給

acceptance sheetは6/6生成され、`sheet_generated=true`は全件だった。
ただし6枚すべてで所要・完成定義・検証実録が「記録なし」となった。
生成率100%と内容充足度を分けて扱う。

### 6.3 較正コーパス

C2/C3、`nearest_miss`とも0件で、較正コーパスへの追加は0件。
worktreeへの自動追加もなかった。全runがC系check前に停止したためであり、
未到達を`claims_absent`とは数えていない。

## 7. C1〜C4 evidence実物監査

campaignの`artifacts/`と`workspaces/`をhidden path込みで検索したが、
次の4 evidenceは全て0件だった。

| evidence | 件数 | 判定 |
|---|---:|---|
| `evidence/cli-case-binding.json` | 0 | C1未到達 |
| `evidence/cli-probe.json` | 0 | 極性実行なし |
| `evidence/help-binding.json` | 0 | C2未到達 |
| `evidence/cli-assurance.json` | 0 | C1〜C4集約未実行 |

したがって:

- ケース束縛凍結: 0/6
- 正常／不正極性両側: 0/6
- help照合方向別結果: 0/6
- C3出力主張対照表: 0/6
- C2/C3 nearest_miss: 0件

C2 help照合の実戦初成績は今回も**未計測**である。モデル自身のsmoke test
やREADMEをCLI profile evidenceへ代用していない。

## 8. 死因、族差、cloud値札

全6件のterminal death classは`process_failure` / modelだった。

- stats族: full `0/3`、平均536.7秒
- filter族: full `0/3`、平均566.3秒
- cloud列全体: full `0/6`、平均551.5秒

代表停止原文:

```text
phase create-documentation failed: step run-verification failed verification
after bounded repair: README should contain '使い方'
```

```text
phase create-cli-tool failed: step verify-cli-behavior failed verification
after bounded repair: Smoke test failed
```

両族とも成果物生成途中の自己検証不一致をbounded repairで解消できず停止
した。executorは1種類のためexecutor間比較は未測定である。

## 9. コスト記録

`date +%s`基準:

| 境界 | epoch | JST |
|---|---:|---|
| preflight開始 | 1784969907 | 2026-07-25 17:58:27 |
| run列開始 | 1784970007 | 2026-07-25 18:00:07 |
| 最終run終端 | 1784973316 | 2026-07-25 18:55:16 |
| 監査記録終端 | 1784979670 | 2026-07-25 20:41:10 |

- preflight開始→最終run終端: 3409秒
- 6 run所要合計: 3309秒
- preflight開始→監査記録終端: 9763秒

## 10. 合否

| 基準 | 結果 | 根拠 |
|---|---|---|
| P0-a 6/6正直終端 | **PASS** | harness completed 6/6、product exit 1を保持 |
| P0-b 契約§4準拠 | **FAIL** | C1〜C4未実行なのにpartial 6/6。契約はstaticを要求 |
| P0-c 偽成功ゼロ | **PASS** | verdict failed 6/6、full主張0 |
| P1-a 到達runでC1実行 | **NOT MEASURED** | profile acceptance到達run 0 |
| P1-b 検収シート6/6 | **PASS** | `sheet_generated=true` 6/6 |

記録値:

- full率: `0/6`（cli cloud列の初値札）
- C2 help照合: 到達`0/6`、pass/failとも0
- C3 claims binding: 到達`0/6`、pass/failとも0
- 新terminal class: 0
- シート自給率: `6/6`
- admission static cap解除: `6/6`

## 11. Scrubと監査境界

- 正式campaign各runのscrubは6/6
  `ok=true / findings=[] / allow=[]`。
- 除外campaignの実行済み2runもscrub 2/2 `ok=true`。
- cloud資格情報値、raw console log、runtime `.anvil/` stateはコミット
  対象にしていない。
- repositoryにはこの新規UAT directoryのscrub済みsummary/reportだけを
  追加し、過去記録を上書きしていない。

一次資料SHA-256:

- 正式`uat-meta.json`:
  `9933f40b63b62d73c882927de648dd326a9574307049bc63c3cdfeeb69a1d3e1`
- 正式`report-skeleton.md`:
  `4af3c7dd142a59e04452d78212c02680df4b8c1b66a4e82c94ff4b0e28377a82`
- 除外`uat-meta.json`:
  `041a8b3cb638415880edbd3f95abd995ceaad5670001f18e03504fcdacc863d1`

Follow-up: admitted CLIのearly terminal projectionへCLI契約§4を適用し、
C1未実行をstaticへ保つ。その修正後campaignでC1到達率を上げ、C2/C3の
実戦成績とnearest_miss較正を初採取する。

## 12. CLI-2レビュー裁定追補（2026-07-25）

本directoryの[`death-anatomy.md`](death-anatomy.md)に基づき、
`process_failure`群6件の帰属を**modelからmachine（README verify過制約）
へ訂正**する。代表runで成果物生成と先行phase完了を確認し、固定CLI契約が
要求しないgoal由来の自然言語字義assertが直接停止要因だったことを
レビュー裁定した。

本campaignには`model_stagnation_read_only`はない。local armに存在する
read-only停滞2件は別形であり、model帰属を維持する。既存本文は計測時点の
初期分類として保存し、本節を最終裁定とする。
