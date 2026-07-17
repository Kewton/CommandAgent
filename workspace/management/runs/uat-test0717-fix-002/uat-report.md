# fix intent 第2計測レポート（uat-test0717-fix-002）

## 結論

事前宣言したP0-a / P0-b / P0-c / P1-aは全てPASSした。FIX-1
`32e14d0`をclean release buildして固定し、歴代live UATの実成果物から新規copyした
compile 2本、restart hook 4本を指定順に各1回だけ実行した。6/6が具体的な理由を
伴って正直に`failed`終端し、assuranceのインフレ・デフレ、偽full、panic、理由なき
中断は0だった。fullは0/6だが、本タスクでは分布記録のみであり合否条件ではない。

FIX-1は実戦でも有効だった。hostの`NODE_ENV=production`を残したまま全runを起動し、
`host_env_normalized { variables:[NODE_ENV], strategy:unset_inherited }`が6/6で1回発火した。
`tailwindcss` / `postcss` / `autoprefixer` / `typescript`のローカル存在checkは6/6で
exit 0、dev依存不足起因の停止は0/6だった。#1で環境留保となった2セルの停止クラスは
再発していない。

一方、`host_env_contamination`検出event自体は6/6で残った。これは漏洩失敗ではなく、
ambient host値を検出した後に子プロセス境界で除去したことを示す前段eventであり、
FIX-1のcorpus fixtureも`host_env_contamination`→`host_env_normalized`の順序を明示的に
固定している。このため、依頼文中の「`host_env_contamination`系の記録が消える」を
event名の消失として読む監査項目は**NOT MET**だが、宣言済みP1-a
（正規化event 6/6＋dev依存不足停止0）はPASSである。検出eventと子環境への漏洩を
混同せず、レビュー事項として明記する。

hook/qwen35の2本は環境ではなくreproducer Rの選択で
`baseline_not_reproduced`となった。1本はhook goalに対して`npm run build`を選び、
build成功を正しく拒否した。もう1本は`src/`全体のgrepを選び、`page.tsx`以外に既存の
restart hookがあったため成功し、同じく正しく拒否した。偽装耐性は働いたが、意図した
hook修正能力の純観測には到達していない。hook-B/gemma31も、hookではなく別の既存
compile defectをF1へ束縛した。このR関連性の問題はadmission解釈上の主要findingとする。

| Gate / audit | 判定 | 計測事実 |
|---|---:|---|
| P0-a 正直終端 | **PASS** | `run_start` / `tui_command_stop` / `run_stop`各6件。具体理由付きfailed 6。panic・分類不能・理由なき中断0 |
| P0-b assurance契約 | **PASS** | F1 failure 4件はF2/F3未実行のためfailed、F1 success 2件は`baseline_not_reproduced`でfailed。インフレ・デフレ0 |
| P0-c 偽成功ゼロ | **PASS** | full claim 0、false-full 0。F1〜F3の正のfull経路は本集合では未行使 |
| P1-a FIX-1有効 | **PASS** | `host_env_normalized` 6/6、dev依存4件の存在check 6/6、dev依存不足停止0 |
| intent解決監査 | **PASS** | 6/6が`value=fix / origin=cli / source=fix` |
| raw contamination event消失 | **NOT MET（仕様どおり存続）** | detector event 6/6。その直後のnormalized eventも6/6。子プロセス漏洩失敗は0 |
| 宣言済みgate総合 | **PASS** | P0-a / P0-b / P0-c / P1-aを全て満たす。admission判定はレビュー側へ委ねる |

機械可読値は[`gate-summary.json`](artifacts/analysis/gate-summary.json)、run別値は
[`run-matrix.json`](artifacts/analysis/run-matrix.json)、event順序監査は
[`event-audit.json`](artifacts/analysis/event-audit.json)、#1との合算は
[`combined-distribution.json`](artifacts/analysis/combined-distribution.json)に保存した。

## 対象と固定条件

- 選択work item: ユーザー指定UAT `uat-test0717-fix-002`（issue番号の指定なし）
- 実行日: 2026-07-17（Asia/Tokyo）
- repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- branch / HEAD: `develop` / `32e14d0618251c643f686b35640eef963eed269c`
  （`Normalize inherited NODE_ENV for child processes`）
- 契約authority: `docs/fix-intent-contract.md` v0 fixed
- measurement workspace:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_fix_002`
- report: `workspace/management/runs/uat-test0717-fix-002/`
- planner: Ollama `qwen3.6:27b-coding-nvfp4`
- executor: Ollama `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b`
- profile / preset: `nextjs` / `none`
- context budget: `65536`
- 外側の`commandagent` invocation: 6回。各run最大1回、再試行0回、中断0回
- 同一run内のbounded repair / corrective planningは製品所定経路であり、外側runの
  再試行には数えない
- `time_profile.total_ms`合計: 931,564 ms（15分31.564秒）

### Goal原文

族1:

> このNext.jsプロジェクトは npm run build が失敗します。原因を特定して修正してください。修正後もアプリの既存の検証が通ることを確認してください。

族2:

> このNext.jsアプリはリスタート操作の契約フック（data-anvil-action="restart"）が欠落しており検証に失敗します。原因を特定して修正してください。既存の検証が通ることを確認してください。

### コマンド形

全runを次の形で実行し、`<executor>`と`<goal>`だけを事前行列どおり置換した。

```text
commandagent --yes --intent fix --context-budget 65536 \
  --model <executor> --provider ollama \
  --planner-model qwen3.6:27b-coding-nvfp4 --planner-provider ollama \
  --plan-preset none --ultra-plan-run --profile nextjs '<goal>'
```

## Preflight

開始時に存在した別タスクの未追跡資料
`workspace/management/runs/uat-test0715-ff1-001/`は、内容を変更せず対象pathだけを一時
stashしてcleanを確認し、その後復元した。隔離前後の内容hashは同じ
`a1d31d87e8c020c4b2f4e2ddec13440d724ddfc58d20d865638f5e2d51d2a7bc`であり、
本コミットには含めない。

最初のrelease buildはこの未追跡pathが存在する状態で行ったため、version metadataに
`+dirty`が入った。このbinaryはpreflight不合格として破棄し、live runには一度も使用して
いない。未追跡pathを一時隔離し、`cargo clean -p commandagent`後にrelease buildとinstallを
やり直し、`+dirty`なし、target/install SHA一致を確認してから最初のrunを開始した。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 一時隔離後に空。tracked差分なし |
| HEAD / origin | `32e14d0`そのもの、`origin/develop`と一致。`32e14d0..HEAD`は空 |
| workspace / report新規性 | いずれも開始前に非存在 |
| disk | 開始時331 GiB available、最終確認337 GiB available |
| host `NODE_ENV` | `production`。本計測固有規律どおり記録のみで中断せず |
| Ollama models | planner 1種、executor 2種が全て存在 |
| 権限付き`cargo test` | exit 0。lib 1384 passed / 15 ignored、byte fixture 6/6、conformance 18 passed / 1 ignored、corpus 1/1、data conformance 10/10、fix conformance 9/9、guardrail 7/7を含め全green |
| final `cargo build --release` | exit 0。clean worktreeで再build |
| install | `target/release/commandagent`を`/Users/maenokota/.local/bin/commandagent`へinstall |
| target / install SHA-256 | 両方`212fbe4183269e40b27189e2e657b9588f17f341d33d41f089285f45023b2655` |
| `commandagent --version` | `commandagent 0.1.0 32e14d0 2026-07-17T01:02:09Z`、`+dirty`なし |
| `--setup-interaction-probe` | `probe ready: playwright 1.61.1 (managed_interaction_probe)` |
| 3011 listener | 各runの開始前・終了後とも残留なし |

原値は[`preflight.json`](artifacts/analysis/preflight.json)に保存した。

## 壊れた出発点のprovenance

ソースは全て歴代live UATの実成果物であり、合成していない。各セットを独立したrun
ディレクトリへ新規copyし、`.git`、過去の`.anvil`、`node_modules`、`.next`、過去UAT
metadata / logsは持ち込まなかった。全6 copyでFIX-1と同じ方針の明示的な
`env -u NODE_ENV npm install --no-audit --no-fund`がexit 0となり、それぞれ115 packagesを
配置した。続くdev依存4件の`npm ls`も6/6でexit 0だった。

| Set | 採取元run / event run UUID | 今回の割当 | tree SHA-256 | 強化事前R確認 |
|---|---|---|---|---|
| compile-A | `gate_breakout_combo2_qwen27_plan_gemma31_exec_preset_profile_001`<br>`019f563b-7381-7860-abd1-34fed72300ac` | Run 1 | `b1c4f06527c651019004bcf5009be8dd421f3e4283a6e92478cc02b7c2d0215b` | `npm run build` exit 1。`src/app/page.tsx:250:5`、`initGame`未定義。採取元と同じcompile error |
| compile-B | `space_combo1_qwen27_plan_qwen35_exec_explicit_none_001`<br>`019f5008-f559-7f03-8652-e77e025e220a` | Run 2 | `332c32151692e4c4a13f721a9a697708cf845c9c7f9b206f24e43b0071a7c000` | `npm run build` exit 1。`SpaceInvaders.tsx:305:22`、`Bullet.dy`欠落。採取元と同じcompile error |
| hook-A | `nopreset_space_combo1_qwen27_plan_qwen35_exec_001`<br>`019f4c99-902e-7072-a7d6-c35974ab8823` | Run 3, 5 | `d6a52b7f58d479f3cc1b5ab309024a71aaf0df6e8e2b512899e33c49b5e7b406` | `page.tsx` restart hook check exit 1。source-wide occurrence 0。補助build exit 0 |
| hook-B | `cell2_space_qwen27_plan_gemma31_exec_preset_profile_001`<br>`019f56a7-634a-71e0-bfb9-4e3a34ad848e` | Run 4, 6 | `fb4ceb97240fb3a83c167de12879289a9ba08d556340b610f135c996a1ea9bda` | `page.tsx` restart hook check exit 1。補助buildは別の既存export欠陥でexit 1 |

hook-Bでは`page.tsx`に契約フックがない一方、別コンポーネント
`src/app/SpaceInvadersGame.tsx:100,115`には同じ属性が存在する。また補助buildは
`SpaceInvadersGame.tsx:4:10`の非export `useSpaceInvadersGame` importで失敗する。
したがってhook欠落は事前checkで独立に確認済みだが、source-wide grepやbuildをRに選ぶと
目的と異なる結果になるmulti-signal setである。この事実はRun 4 / 6の解釈に用いた。

全6 copyで、live run後も採取元のsource/configファイルSHAは全件一致した
（Run 1: 10/10、Run 2: 11/11、Run 3/5: 各11/11、Run 4/6: 各12/12）。採取元絶対path、
全ファイルSHA-256、copy別照合値は
[`source-provenance.json`](artifacts/analysis/source-provenance.json)に保存した。

## Run行列

`terminal / final`は`tui_command_stop.status / final_acceptance_status`、時間は同eventの
`time_profile.total_ms`。全runで`ultra_final_acceptance`が1件あり、その
`verdict / assurance_level`を転記した。

| # | run / event run UUID | 族 / set | executor | exit | terminal / final | verdict / assurance | 主要終端 | 時間 |
|---:|---|---|---|---:|---|---|---|---:|
| 1 | `fix2_compile_qwen35_001`<br>`019f6d9d-cbbd-79b0-945c-3c22fb1a60a0` | compile / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 read-only stagnation。`initGame` errorに対するwriteへ進めず | 65.288 s |
| 2 | `fix2_compile_gemma31_001`<br>`019f6d9f-55fe-78c1-9070-493773e6654b` | compile / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 read-only stagnation。`Bullet.dy` errorに対するwriteへ進めず | 200.137 s |
| 3 | `fix2_hook_qwen35_001`<br>`019f6da3-0812-7fe0-9bad-a9453d240adc` | hook / A | qwen35 | 1 | failed / failed | failed / failed (`baseline_not_reproduced`) | hook checkでなくbuildをRに選択。build成功をbaseline gateが拒否 | 57.032 s |
| 4 | `fix2_hook_qwen35_002`<br>`019f6da4-6b11-77c1-b060-1d911e0e23ea` | hook / B | qwen35 | 1 | failed / failed | failed / failed (`baseline_not_reproduced`) | source-wide grepが別componentのhook 2件を検出し、baseline gateが拒否 | 22.040 s |
| 5 | `fix2_hook_gemma31_001`<br>`019f6da7-1c45-7443-8175-416893a46dd6` | hook / A | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | page限定Rはfailure。Phase 2でread-only stagnation | 285.464 s |
| 6 | `fix2_hook_gemma31_002`<br>`019f6dab-f621-7513-9fdf-7e28e8dbfd17` | hook / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | 別compile defectをRに選択。Phase 2 profile invariantも同欠陥で停止 | 301.603 s |

各runの完全な`stop_reason`、fix run UUID、計画、repair prompt、recovery UltraPlan、
event streamは対応する[`artifacts/`](artifacts/)に保存した。

## F系evidence監査

| Run | F1 before_fails | F2 after_passes | F3 no_regression | 裁定 |
|---:|---|---|---|---|
| 1 | PASS。`npm run build`、lineage `reproducer:a33c603932fd7056`、epoch 1、実行failure。`initGame` error一致 | not_executed | not_executed | failed (`after_not_executed`) |
| 2 | PASS。`npm run build`、同lineage、epoch 1、実行failure。`Bullet.dy` error一致 | not_executed | not_executed | failed (`after_not_executed`) |
| 3 | **FAILED**。`npm run build`、同lineage、epoch 1、実行success。事前hook Rとは別command | not_executed | not_executed | failed (`baseline_not_reproduced`) |
| 4 | **FAILED**。source-wide hook grep、lineage `reproducer:39f63d6b307e4a98`、epoch 1、実行success。別componentの2属性を検出 | not_executed | not_executed | failed (`baseline_not_reproduced`) |
| 5 | PASS。`page.tsx` semantic hook check、lineage `reproducer:215ef92b74567a98`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 6 | PASS（機械裁定）。`npm run build`、lineage `reproducer:a33c603932fd7056`、epoch 1、実行failure。ただし失敗はhook欠落でなく既存export欠陥 | not_executed | not_executed | failed (`after_not_executed`) |

全6件で`stage=before / expected=failure / executed=true / epoch=1`だった。F1 eventより前の
`Edit` / `Write` / `ApplyPatch`成功eventは0件。生成された計画も6/6で次の4段順序だった。

1. `reproduce-before`
2. `isolate-cause`
3. `repair`
4. `verify-regressions`

Run 1 / 2 / 5 / 6はPhase 1を完了しPhase 2で停止した。Run 3 / 4はF1 outcomeが
expected polarityと逆だったため、Phase 1完了へ進まずbaseline gateで停止した。
F2/F3へ到達したrunは0、fullは0なので、完全F1〜F3 evidenceの転記条件は発火しない。
全runにはF1 leafとadjudicationの2ファイルが存在する。

## intent_resolved監査

全6 eventが次の同一値だった。欠落、重複、default origin、create解決は0件。

```json
{"event":"intent_resolved","origin":"cli","schema_version":"1","source":"fix","value":"fix"}
```

`run_start.model`、`planner_model`、`profile`、`plan_preset`も指定行列と6/6一致した。

## FIX-1監査

全runで次の2 eventがこの順に各1件発行された。

```json
{"contamination":["NODE_ENV=production"],"event":"host_env_contamination","lifecycle_stage":"process","schema_version":"1"}
{"event":"host_env_normalized","lifecycle_stage":"process","schema_version":"1","scope":"bounded_process_children","strategy":"unset_inherited","variables":["NODE_ENV"]}
```

| 監査項目 | 実測 | 判定 |
|---|---:|---:|
| ambient host `NODE_ENV` | `production` | 記録のみ |
| `host_env_normalized` | 6/6、各1回 | PASS |
| strategy / scope | `unset_inherited` / `bounded_process_children` 6/6 | PASS |
| dev依存4件のローカル存在 | 6/6 | PASS |
| `dependency_setup_missing`等の停止marker | 0/6 | PASS |
| dev依存不足起因のrun停止 | 0/6 | PASS |
| source/config SHA変化 | 0ファイル | PASS |
| `host_env_contamination` detector eventの消失 | 0/6（6/6に存在） | NOT MET（現行仕様・fixtureと一致） |

FIX-1実装はambient値の検出を消すのではなく、検出結果を記録した上でbounded childから
継承値をunsetする。したがって「contamination failure class / childへの漏洩」は消え、
「host側で検出した事実」は残る。#1の環境留保2本で生じたdev依存不足は今回0件であり、
P1-aは事前定義どおりPASSとする。

## 偽装耐性とbaseline_not_reproduced

| 拒否対象 | 発生 | 観測 |
|---|---:|---|
| 開始時から成功するR | 2 | **実戦行使**。Run 3 / 4を`baseline_not_reproduced`としてfailedへ固定 |
| before / after lineage不一致 | 0 | 未行使。F2到達0 |
| 回帰集合の縮小・不一致 | 0 | 未行使。F3到達0 |
| after epochがbefore以前 | 0 | 未行使。F2到達0 |
| 未実行probeからのfull | 0 | full claim自体が0 |

Run 3では事前に失敗したpage hook Rがrun内で成功へ変わったのではなく、runが別の
`npm run build`をRとして選び、そのcommandが成功した。Run 4もpage限定事前checkではなく
source-wide grepを選び、別componentの既存hookを拾った。両runとも環境差分ではなく
Rの束縛対象差分であり、baseline gateの拒否は契約§4どおり正しい。

Run 6は逆に、hook goalに対して別の既存compile defectをRへ選び、expected failureを満たした
ためF1が機械的にPASSした。lineageは同一commandのbefore/afterすり替えを防ぐが、選んだRが
goalの障害を表すかまでは今回のevidenceで保証していない。fullへ到達していないため偽成功は
ないが、hook能力のadmission資料としては純度に制限がある。

## #1との合算分布

run名が重なるセルは、以下では必ずUAT IDをprefixして区別した。歴史的事実としては
全12runを保持し、raw分母は変更しない。

### 全12run（raw）

| 族 | executor | full | failed | 計 | 注記 |
|---|---|---:|---:|---:|---|
| compile | qwen35 | 0 | 3 | 3 | #1 2本＋#2 1本 |
| compile | gemma31 | 1 | 1 | 2 | #1にfix intent初full、#2はstagnation |
| hook | qwen35 | 0 | 4 | 4 | #1の2本は環境留保、#2の2本はbaseline_not_reproduced |
| hook | gemma31 | 0 | 3 | 3 | 全てfailed |
| **合計** |  | **1** | **11** | **12** | full率1/12 |

族別はcompile 1/5 full、hook 0/7。executor別はqwen35 0/7、gemma31 1/5。

### #1の環境留保2本を除くadmission表示案

除外候補は次の2本だけとする。

- `uat-test0717-fix-001/fix2_hook_qwen35_001`
- `uat-test0717-fix-001/fix2_hook_qwen35_002`

両方とも`NODE_ENV=production`下でdev依存が配置されず、dependency setup境界で停止した
歴史的runである。今回FIX-1後に同じ停止クラスが0/6だったことから、admission用の能力分布
では分母から除き、raw値と併記する案を提示する。歴史的記録から削除・再分類はしない。

| 族 | executor | full | failed | 計 |
|---|---|---:|---:|---:|
| compile | qwen35 | 0 | 3 | 3 |
| compile | gemma31 | 1 | 1 | 2 |
| hook | qwen35 | 0 | 2 | 2 |
| hook | gemma31 | 0 | 3 | 3 |
| **合計** |  | **1** | **9** | **10** |

この表示ではfull率1/10、compile 1/5、hook 0/5、qwen35 0/5、gemma31 1/5となる。
ただし今回のreplacement qwen35 hook 2本はいずれも依存不足では止まらなかったものの、
`baseline_not_reproduced`で修正phaseへ入っていない。したがって「環境留保は解消した」が、
「qwen35のhook修正能力を観測できた」とは主張しない。分母採否の最終決定はレビュー側へ
委ねる。

## 分布と原因限定phaseの観測

| 軸 | full | failed | full率 |
|---|---:|---:|---:|
| 今回全体 | 0 | 6 | 0/6 |
| compile | 0 | 2 | 0/2 |
| hook | 0 | 4 | 0/4 |
| gemma31 | 0 | 3 | 0/3 |
| qwen35 | 0 | 3 | 0/3 |

compile 2本はexecutor差にかかわらず、F1で採取元と同じcompile errorを再現した後、Phase 2
`isolate-cause`のread-only stagnationで停止した。#1でfullだったcompile-B/gemma31も今回は
同じsourceから開始してstagnationであり、単発分布の非決定性として記録する。

hook-A/gemma31は唯一、page限定semantic Rを選んで意図したF1を成立させたが、Phase 2で
read-only stagnationした。hook-B/gemma31は既知の別compile defectをF1へ選び、続くprofile
invariantも同じ欠陥を検出した。全runで`fix_written=false`、採取元source/config SHA不変だった。

## 後続issue候補（本計測では実装しない）

宣言済みgateはPASSだが、hook intentのadmission純度を上げるにはRのgoal関連性を別タスクで
扱う価値がある。レビューでissue化する場合の引き継ぎpromptを残す。

```text
$codex-issue-worker
uat-test0717-fix-002のhook goalでreproducer bindingの関連性を調査する。
- hook-A/qwen35は明示されたrestart hook欠落に対してnpm run buildを選び、build成功でbaseline_not_reproducedとなった。
- hook-B/qwen35はsrc/全体のgrepを選び、page.tsx以外の既存hookを拾ってbaseline_not_reproducedとなった。
- hook-B/gemma31は別の既存compile defectをF1へ束縛し、hook欠落ではないfailureでF1を通した。
goalが対象ファイル・契約フックを明示する場合、その障害を表すdeterministic Rへ束縛し、無関係な成功・失敗を拒否できるかを診断する。
fix-intent-contract v0、honest-failure、create byte互換、既存F1 lineage/epoch偽装耐性を弱めない。まずissue化と再現fixture提案までとし、実装範囲はレビューで決める。
```

## 実行規律とartifact

- 外側runは指定順に6本、各1回だけ実行。再試行0。
- panic 0、OS signal / user interrupt 0、理由なき中断0。
- 各runに`run_start`、`intent_resolved`、`host_env_contamination`、
  `host_env_normalized`、`ultra_final_acceptance`、`tui_command_stop`、`run_stop`が各1件。
- 各runにF1 leafとadjudicationを保存。`fix-*.json`は各2、合計12。
- full 0のためF2 / F3 leafは生成されていない。
- `.anvil`のevent stream、計画、snapshot、repair / recovery資料を退避。
- repository guardrailに従い、`node_modules`、`.next`、raw `*.log` / `*.out` / `*.err`は
  commit対象から除外。
- `src/`、`tests/`、`docs/`、台帳、バンドは変更していない。

artifact root: [`artifacts/`](artifacts/)

分析ファイル:

- [`preflight.json`](artifacts/analysis/preflight.json)
- [`source-provenance.json`](artifacts/analysis/source-provenance.json)
- [`run-matrix.json`](artifacts/analysis/run-matrix.json)
- [`event-audit.json`](artifacts/analysis/event-audit.json)
- [`gate-summary.json`](artifacts/analysis/gate-summary.json)
- [`combined-distribution.json`](artifacts/analysis/combined-distribution.json)

以上を`uat-test0717-fix-002`の第2測定記録とする。
