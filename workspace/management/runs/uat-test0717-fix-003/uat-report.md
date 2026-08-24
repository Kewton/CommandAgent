# fix intent 第3計測レポート（uat-test0717-fix-003）

## 結論

事前宣言したP0-a / P0-b / P0-cとP1-aはPASS、P1-bはFAILとなった。FIX-2 / FIX-3を含む
`e0f3f67`をclean release buildして固定し、歴代live UATの実成果物から新規copyしたcompile
2本、restart hook 4本を指定順に各1回だけ実行した。6/6が具体的理由を伴って正直に
`failed`終端し、assuranceのインフレ・デフレ、偽full、panic、理由なき中断は0だった。
fullは0/6である。

FIX-2は実効した。`fix_reproducer_suggested`は6/6で発火し、提示Rは6/6で意味的に採用された。
byte一致は5/6で、残る1本もJavaScriptの空白だけが異なる同一route-bound predicateだった。
#2で3本あったR関連性逸脱は0本、literalな`baseline_not_reproduced`は2本から0本へ減少した。

FIX-3は限定的な配線確認には成功したが、実行効果のgateを満たさなかった。compile 2本では
F1出力から正しいファイル・行・エラー種別を抽出し、Phase 2の保存済みStepPlanへ
`diagnosis_mapped` targetとともに注入できた。しかしdiagnostic blockがPhase 2の全step、
verify stepにも付加され、その中の`write-pressure target`文言をplanner lintがfile change要求と
判定した。このため2本ともexecutorへ渡る前に
`verify step instruction must not request file changes`で停止した。実行済みのrepair prompt原文は
存在せず、停止後に保存されたrecovery promptにはdiagnostic自体が残っていない。

hook 4本のF1はcompile error / Python tracebackを含まないpredicate failureだったため、FIX-3の
diagnostic抽出対象にならなかった。うち3本はgenericな`package.json`をwrite圧力targetとして
read-only stagnationし、残る1本は既存のrelative-import profile invariantで停止した。
Phase 2 read-only停滞は#2の3本から今回も3本で減少していない。したがって、事前基準の
「F1失敗runのPhase 2 promptに診断抜粋が実在」を厳格に適用すると2/6に留まり、さらにその
2本も実行promptへ到達していないためP1-bはFAILとする。

| Gate / audit | 判定 | 計測事実 |
|---|---:|---|
| P0-a 正直終端 | **PASS** | 具体理由付きfailed 6/6。`run_start` / `tui_command_stop` / `run_stop`各6件。panic・分類不能・理由なき中断0 |
| P0-b assurance契約 | **PASS** | F1は6/6で実行failure、F2/F3は0/6。全件`failed(after_not_executed)`で契約§4と一致 |
| P0-c 偽成功ゼロ | **PASS** | full claim 0、false-full 0。未実行F2/F3からの獲得0 |
| P1-a FIX-2実効 | **PASS** | suggestion 6/6、意味的採用6/6、R関連性逸脱3→0、literal baseline拒否2→0 |
| P1-b FIX-3実効 | **FAIL** | 保存済みPhase 2 planへの診断注入は適用対象2/2だが、全F1失敗runでは2/6。2本ともlintでexecutor prompt前に停止。停滞3→3 |
| intent解決監査 | **PASS** | 6/6が`value=fix / origin=cli / source=fix` |
| 総合 | **FAIL** | P1-b不成立。admission closeは行わず、一次資料と後続issue promptを提出 |

機械可読値は[`gate-summary.json`](artifacts/analysis/gate-summary.json)、run別値は
[`run-matrix.json`](artifacts/analysis/run-matrix.json)、FIX-2 / FIX-3監査は
[`fix-effects.json`](artifacts/analysis/fix-effects.json)、#1〜#3合算は
[`combined-distribution.json`](artifacts/analysis/combined-distribution.json)に保存した。

## 対象と固定条件

- 選択work item: ユーザー指定UAT `uat-test0717-fix-003`（issue番号の指定なし）
- 実行日: 2026-07-17（Asia/Tokyo）
- repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- branch / HEAD: `develop` / `e0f3f67a5c03dc4aa57a34d25327ba27daa9ef17`
  （FIX-2 `e24f542`、FIX-3 `e0f3f67`を含む）
- 契約authority: `docs/fix-intent-contract.md` v0 fixed
- measurement workspace:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_fix_003`
- report: `workspace/management/runs/uat-test0717-fix-003/`
- planner: Ollama `qwen3.6:27b-coding-nvfp4`
- executor: Ollama `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b`
- profile / preset: `nextjs` / `none`
- context budget: `65536`
- 外側の`commandagent` invocation: 6回。各run最大1回、再試行0回、中断0回
- 同一run内のbounded recovery planningは製品所定経路であり、外側runの再試行には数えない
- `time_profile.total_ms`合計: 1,449,538 ms（24分9.538秒）

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
`workspace/management/runs/uat-test0715-ff1-001/`は、内容を変更せず一時隔離してcleanを確認し、
release build後に復元した。隔離前後の内容hashは同じ
`a1d31d87e8c020c4b2f4e2ddec13440d724ddfc58d20d865638f5e2d51d2a7bc`であり、本コミットには
含めない。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 一時隔離後に空。tracked差分なし |
| HEAD / origin | `e0f3f67`そのもの、`origin/develop`と一致。`e0f3f67..HEAD`は空 |
| workspace / report新規性 | いずれも開始前に非存在 |
| disk | 開始時334 GiB available、run後332 GiB available |
| host `NODE_ENV` | `production`。本計測固有規律どおり記録のみで中断せず |
| Ollama models | planner 1種、executor 2種が全て存在 |
| 権限付き`cargo test` | exit 0。lib 1395 passed / 15 ignored、byte fixture 6/6、conformance 18 passed / 1 ignored、fix conformance 9/9、corpus 1/1、data conformance 10/10、guardrail 7/7、doc tests 2/2を含め全green |
| `cargo build --release` | `cargo clean -p commandagent`後にexit 0 |
| install | `target/release/commandagent`を`/Users/maenokota/.local/bin/commandagent`へinstall |
| target / install SHA-256 | 両方`f5b9792ecbbf8c1ae5c6c5caa7789ad26dead1f67c119065061d38dceb2e9030` |
| `commandagent --version` | `commandagent 0.1.0 e0f3f67 2026-07-17T04:13:15Z`、`+dirty`なし |
| `--setup-interaction-probe` | `probe ready: playwright 1.61.1 (managed_interaction_probe)` |
| 3011 listener | 最初のrun前・最後のrun後とも残留なし |

全値は[`preflight.json`](artifacts/analysis/preflight.json)に保存した。

## 壊れた出発点のprovenance

ソースは全て歴代live UATの実成果物であり、合成していない。#2と同じ採取元から各run用に
新規copyし、`.git`、過去の`.anvil`、`node_modules`、`.next`、evidence、過去UAT資料は
持ち込まなかった。全6 copyで
`env -u NODE_ENV npm install --no-audit --no-fund`がexit 0となり、それぞれ115 packagesを
配置した。続く`tailwindcss` / `postcss` / `autoprefixer` / `typescript`の`npm ls`も
6/6でexit 0だった。

| Set | 採取元run / event run UUID | 今回の割当 | tree SHA-256 | 強化事前R確認 |
|---|---|---|---|---|
| compile-A | `gate_breakout_combo2_qwen27_plan_gemma31_exec_preset_profile_001`<br>`019f563b-7381-7860-abd1-34fed72300ac` | Run 1 | `b1c4f06527c651019004bcf5009be8dd421f3e4283a6e92478cc02b7c2d0215b` | `npm run build` exit 1。`src/app/page.tsx:250:5`、`initGame`未定義。採取元と同じcompile error |
| compile-B | `space_combo1_qwen27_plan_qwen35_exec_explicit_none_001`<br>`019f5008-f559-7f03-8652-e77e025e220a` | Run 2 | `332c32151692e4c4a13f721a9a697708cf845c9c7f9b206f24e43b0071a7c000` | `npm run build` exit 1。`SpaceInvaders.tsx:305:22`、`Bullet.dy`欠落。採取元と同じcompile error |
| hook-A | `nopreset_space_combo1_qwen27_plan_qwen35_exec_001`<br>`019f4c99-902e-7072-a7d6-c35974ab8823` | Run 3, 5 | `d6a52b7f58d479f3cc1b5ab309024a71aaf0df6e8e2b512899e33c49b5e7b406` | `page.tsx` restart hook check exit 1。source-wide occurrence 0。補助build exit 0 |
| hook-B | `cell2_space_qwen27_plan_gemma31_exec_preset_profile_001`<br>`019f56a7-634a-71e0-bfb9-4e3a34ad848e` | Run 4, 6 | `fb4ceb97240fb3a83c167de12879289a9ba08d556340b610f135c996a1ea9bda` | `page.tsx` restart hook check exit 1。補助buildは別の既存export欠陥でexit 1 |

hook-Bでは`page.tsx`に契約フックがない一方、別component
`src/app/SpaceInvadersGame.tsx:100,115`には同じ属性が存在する。また補助buildは
`SpaceInvadersGame.tsx:4:10`の非export `useSpaceInvadersGame` importで失敗する。この
multi-signal setでも今回FIX-2が4/4でpage限定Rを提示し、別componentやbuildへ逸脱しなかった。

全6 copyでlive run後も採取元のsource/configファイル内容は全件一致した。mtimeだけが変わった
`package-lock.json`等はあったがbyte差分は0である。採取元絶対path、file count、copy照合値は
[`source-provenance.json`](artifacts/analysis/source-provenance.json)に保存した。

## Run行列

`terminal / final`は`tui_command_stop.status / final_acceptance_status`、時間は同eventの
`time_profile.total_ms`。全runで`ultra_final_acceptance`が1件あり、その
`verdict / assurance_level`を転記した。

| # | run / event run UUID | 族 / set | executor | exit | terminal / final | verdict / assurance | 主要終端 | 時間 |
|---:|---|---|---|---:|---|---|---|---:|
| 1 | `fix3_compile_qwen35_001`<br>`019f6e4c-b6d0-7d93-af0a-f8c288243b1a` | compile / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | FIX-3診断はpage.tsxへ解決。Phase 2 verify instructionをlintが拒否 | 186.031 s |
| 2 | `fix3_compile_gemma31_001`<br>`019f6e4f-caa9-7c81-af73-862192292d3a` | compile / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | FIX-3診断はSpaceInvaders.tsxへ解決。同じPhase 2 lint拒否 | 191.970 s |
| 3 | `fix3_hook_qwen35_001`<br>`019f6e52-f2a1-7813-9e80-2bba1e0e998c` | hook / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | page限定Rを採用。診断なし、package.jsonへのread-only stagnation | 190.073 s |
| 4 | `fix3_hook_qwen35_002`<br>`019f6e56-1a4d-7980-8e5c-84cdd6ac9d9c` | hook / B | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | page限定Rを採用。診断なし、package.jsonへのread-only stagnation | 220.981 s |
| 5 | `fix3_hook_gemma31_001`<br>`019f6e59-b831-70c2-9771-e2b10c0b3205` | hook / A | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | 意味同一のpage限定R。診断なし、package.jsonへのread-only stagnation | 290.605 s |
| 6 | `fix3_hook_gemma31_002`<br>`019f6e5e-5605-7021-bd0e-ee0aa1e1f7ec` | hook / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | page限定Rを採用。既存の非export relative-import invariantで停止 | 369.878 s |

各runの完全な`stop_reason`、fix run UUID、計画、recovery資料、event streamは対応する
[`artifacts/`](artifacts/)に保存した。

## F系evidence監査

| Run | F1 before_fails | F2 after_passes | F3 no_regression | 裁定 |
|---:|---|---|---|---|
| 1 | PASS。`npm run build`、lineage `reproducer:a33c603932fd7056`、epoch 1、実行failure。`initGame` error一致 | not_executed | not_executed | failed (`after_not_executed`) |
| 2 | PASS。`npm run build`、同lineage、epoch 1、実行failure。`Bullet.dy` error一致 | not_executed | not_executed | failed (`after_not_executed`) |
| 3 | PASS。route-bound page restart-hook check、lineage `reproducer:215ef92b74567a98`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 4 | PASS。同じroute-bound check、同lineage、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 5 | PASS。意味的に同一のroute-bound check、lineage `reproducer:dbbb1950f9187d26`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 6 | PASS。route-bound page restart-hook check、lineage `reproducer:215ef92b74567a98`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |

全6件で`stage=before / expected=failure / executed=true / epoch=1`だった。F1 eventより前の
successful write eventは0件。生成された計画も6/6で次の4段順序だった。

1. `reproduce-before`
2. `isolate-cause`
3. `repair`
4. `verify-regressions`

全runがPhase 1を完了しPhase 2で停止した。F2 / F3へ到達したrunは0、fullは0なので、
完全F1〜F3 evidenceの転記条件は発火しない。全runにF1 leafとadjudication JSONが存在する。

## intent_resolved監査

全6 eventが次の同一値だった。欠落、重複、default origin、create解決は0件。

```json
{"event":"intent_resolved","origin":"cli","schema_version":"1","source":"fix","value":"fix"}
```

`run_start.model`、`planner_model`、`profile`、`plan_preset`も指定行列と6/6一致した。

hostの`NODE_ENV=production`は6/6で検出され、続く
`host_env_normalized { variables:[NODE_ENV], strategy:unset_inherited,
scope:bounded_process_children }`も6/6で各1回発火した。dev依存不足起因の停止は0である。

## FIX-2監査

| Run | basis | suggestion | モデルのR | 採用判定 |
|---:|---|---|---|---:|
| 1 | `goal_failure_kind:build_or_compile` | `profile_catalog:next_build_verify => npm run build` | `npm run build` | exact |
| 2 | 同上 | 同上 | `npm run build` | exact |
| 3 | `goal_contract_attribute:data-anvil-action=restart` | `hook_attribute_present(action,restart,path=src/app/page.tsx)` | 提示されたroute-bound semantic predicate | exact |
| 4 | 同上 | 同上 | 提示されたroute-bound semantic predicate | exact |
| 5 | 同上 | 同上 | insignificant whitespaceを含む同一predicate | semantic variant |
| 6 | 同上 | 同上 | 提示されたroute-bound semantic predicate | exact |

完全な`suggestion` commandは各runの`events.jsonl`に保存した。hook提示は正規表現を用いて
quoted / JSX expression / template literal形を認識するprofile catalog predicateであり、
`src/app/page.tsx`へroute-boundされている。

| 指標 | #2 | #3 | 変化 |
|---|---:|---:|---:|
| `fix_reproducer_suggested`発火 | 機構導入前 | 6/6 | 新規配線を全該当goalで確認 |
| 意味的採用率 | — | 6/6 | 100% |
| byte完全採用率 | — | 5/6 | Run 5のみ空白差 |
| R関連性逸脱 | 3 | 0 | **3件減** |
| literal `baseline_not_reproduced` | 2 | 0 | **2件減** |

#2 Run 3型の「hook goalでbuild選択」、Run 4型の「全域grep」、Run 6型の「別欠陥build束縛」は
いずれも再発しなかった。P1-aをPASSとする。

## FIX-3監査

### compile診断の注入

compile 2本ではF1のbuild outputから次の値が保存済みPhase 2 StepPlanへ決定的に注入された。

| Run | location / kind | excerpt要点 | 解決target | selection_reason | 結果 |
|---:|---|---|---|---|---|
| 1 | `src/app/page.tsx:250:5` / `Type error` | `initGame();` / `Cannot find name 'initGame'` | `src/app/page.tsx` | `diagnosis_mapped` | lint前のplanへ存在、実行前停止 |
| 2 | `src/app/components/SpaceInvaders.tsx:305:22` / `Type error` | `s.bullets.push(...)` / `Bullet`不足 | `src/app/components/SpaceInvaders.tsx` | `diagnosis_mapped` | lint前のplanへ存在、実行前停止 |

一次資料:

- Run 1:
  [`plan-019f6e4f-9879-78e1-8330-d839e9b3e8d0.yaml`](artifacts/fix3_compile_qwen35_001/.anvil/plans/plan-019f6e4f-9879-78e1-8330-d839e9b3e8d0.yaml)
- Run 2:
  [`plan-019f6e52-c34c-7a91-92e0-38d91dc56726.yaml`](artifacts/fix3_compile_gemma31_001/.anvil/plans/plan-019f6e52-c34c-7a91-92e0-38d91dc56726.yaml)

両planでは次の形のblockがinspect stepだけでなくverify stepのinstructionにも付加された。

```text
Fix F1 failure diagnostic (runtime-derived):
- location: <diagnosed file:line:column>
- error kind: Type error
- message: <compiler message>
- write-pressure target: <diagnosed file> (selection_reason=diagnosis_mapped)
- excerpt: <bounded compiler excerpt>
```

その結果、Phase 2 executor開始前のlintで次を返した。

```text
verify step instruction must not request file changes
```

したがって「targetが計画上正しく解決された」は2/2で確認できたが、実際の
`write_pressure_target_selected`相当の実行eventは0/2であり、診断を受け取ったexecutorの
修復挙動は観測できていない。

### repair prompt原文の監査

Phase 2の保存済みStepPlanがlintで拒否されたため、そのinstructionをexecutorへ渡した
実行promptは存在しない。停止後に生成されたrecovery prompt原文は次の2ファイルである。

- Run 1:
  [`repair-phase-isolate-cause-019f6e4f-987a-7ea0-8973-71e5628cef50.md`](artifacts/fix3_compile_qwen35_001/.anvil/repairs/repair-phase-isolate-cause-019f6e4f-987a-7ea0-8973-71e5628cef50.md)
- Run 2:
  [`repair-phase-isolate-cause-019f6e52-c34d-73b3-bbb2-e104fe8c5dd2.md`](artifacts/fix3_compile_gemma31_001/.anvil/repairs/repair-phase-isolate-cause-019f6e52-c34d-73b3-bbb2-e104fe8c5dd2.md)

このrecovery promptの`Failure evidence`はlint errorだけで、F1 diagnostic location / excerpt /
targetは含まれない。よって、依頼された「repair prompt原文からの診断抜粋注入確認」を
実行prompt到達という意味で満たしたとは扱わない。

### hook predicate failureと停滞

hook 4本はroute-bound predicateのexit 1だけをF1 payloadとして持ち、compile診断やPython
tracebackを持たない。このためFIX-3抽出は0/4だった。

| Run | diagnostic | write圧力target / reason | Phase 2終端 |
|---:|---|---|---|
| 3 | なし | `package.json` / `required_path` | read-only stagnation |
| 4 | なし | `package.json` / `required_path` | read-only stagnation |
| 5 | なし | `package.json` / `required_path` | read-only stagnation |
| 6 | なし | target選択前 | hook-B既存の非export relative-import invariant |

| 指標 | #2 | #3 | 変化 |
|---|---:|---:|---:|
| compile / traceback適用対象へのdiagnostic注入 | 0/2 | 2/2 | 配線確認 |
| 全F1 failure runへのdiagnostic注入 | 0/6 | 2/6 | strict gate未達 |
| Phase 2 read-only stagnation | 3 | 3 | **減少なし** |
| compile runのread-only stagnation | 2 | 0 | lint failureへクラス移動 |
| 新規Phase 2 planner lint failure | 0 | 2 | 新規停止クラス |

以上から、compile / tracebackに限定した抽出単体は2/2だが、P1-bの実行効果はFAILとする。

## 偽装耐性の実戦観測

| 拒否対象 | 発生 | 観測 |
|---|---:|---|
| 開始時から成功するR | 0 | FIX-2により全6 Rが意図したfailureを再現 |
| before / after lineage不一致 | 0 | 未行使。F2到達0 |
| 回帰集合の縮小・不一致 | 0 | 未行使。F3到達0 |
| after epochがbefore以前 | 0 | 未行使。F2到達0 |
| 未実行probeからのfull | 0 | full claim自体が0。未実行F2/F3を獲得せず |

negative guardが発火する入力は今回生じなかった。これは拒否機構が失われたことを意味せず、
R関連性が改善してF1のexpected failureを6/6で観測できた結果である。

## #1〜#3合算分布

歴史的事実として全18runを保持し、raw分母は変更しない。

### 全18run（raw）

| 族 | executor | full | failed | 計 | 注記 |
|---|---|---:|---:|---:|---|
| compile | qwen35 | 0 | 4 | 4 | #1 2本＋#2 1本＋#3 1本 |
| compile | gemma31 | 1 | 2 | 3 | #1にfix intent初full |
| hook | qwen35 | 0 | 6 | 6 | #1の2本は環境留保 |
| hook | gemma31 | 0 | 5 | 5 | 全てfailed |
| **合計** |  | **1** | **17** | **18** | full率1/18 |

族別はcompile 1/7 full、hook 0/11。executor別はqwen35 0/10、gemma31 1/8。

### #1の環境留保2本を除くadmission表示

レビュー裁定に従い、次の2本だけを能力分布の分母から除外し、raw値と併記する。

- `uat-test0717-fix-001/fix2_hook_qwen35_001`
- `uat-test0717-fix-001/fix2_hook_qwen35_002`

両方とも`NODE_ENV=production`継承によるdev依存欠落で停止した歴史的runであり、記録からは
削除・再分類しない。FIX-1後の#2 / #3で同停止クラスは0/12だった。

| 族 | executor | full | failed | 計 |
|---|---|---:|---:|---:|
| compile | qwen35 | 0 | 4 | 4 |
| compile | gemma31 | 1 | 2 | 3 |
| hook | qwen35 | 0 | 4 | 4 |
| hook | gemma31 | 0 | 5 | 5 |
| **合計** |  | **1** | **15** | **16** |

admission表示のfull率は1/16。族別はcompile 1/7、hook 0/9。executor別はqwen35 0/8、
gemma31 1/8となる。ただし今回P1-bがFAILでF2/F3観測が増えていないため、この分布をもって
admission closeやfix×nextjs band確定とはしない。最終判断はレビュー側へ委ねる。

## 分布と新規クラス

| 軸 | full | failed | full率 |
|---|---:|---:|---:|
| 今回全体 | 0 | 6 | 0/6 |
| compile | 0 | 2 | 0/2 |
| hook | 0 | 4 | 0/4 |
| qwen35 | 0 | 3 | 0/3 |
| gemma31 | 0 | 3 | 0/3 |

今回のfailure classは次の分布だった。

- Phase 2 planner lint (`verify step instruction must not request file changes`): 2
- Phase 2 read-only stagnation: 3
- 既存hook-B relative-import profile invariant: 1

`baseline_not_reproduced`は0になった一方、FIX-3注入とplanner lintの相互作用による新規停止
クラスが2本現れた。全runで`fix_written=false`、source/config byte不変だった。

## UAT受入シナリオの照合

| シナリオ | Expected | 実測 / evidence | 判定 |
|---|---|---|---:|
| clean preflight | e0f3f67以降、全test green、clean release、probe ready | revision / SHA / version / test countをpreflight JSONへ固定 | PASS |
| 合成なしbaseline | 歴代実runを新規copyし同種R failureを確認 | 4 source set、6 copy、checksum差0、事前R全件同種failure | PASS |
| 実行規律 | 6本を指定順、各最大1回 | invocation 6、retry 0、interrupt 0 | PASS |
| fix intent | cli/fix 6/6 | `intent_resolved` 6/6 exactly once | PASS |
| F1冒頭配置 | before probeをwrite前に実行 | F1 6/6、write-before-F1 0 | PASS |
| FIX-2 | suggestion発火・関連性逸脱減少 | 発火6/6、採用6/6、逸脱3→0 | PASS |
| FIX-3 | F1 diagnosticをPhase 2実行promptへ接続 | plan payloadは適用対象2/2だがlint前停止、hook 0/4、全体2/6 | FAIL |
| assurance | evidenceと矛盾しない | F1 pass / F2-F3 not_executed → failed 6/6 | PASS |
| false success | fullならF1〜F3完全evidence | full 0、false-full 0 | PASS |

## 後続issue prompt

P1-bがFAILしたため、admission close前に次を別作業として扱う。ここではproduction codeを
変更しない。

```text
$codex-issue-worker
uat-test0717-fix-003のFIX-3実行退行を一次資料から再現し、修正方針とfocused fixtureを提案する。

観測1: compile Run 1/2ではF1診断（location/error kind/excerpt）とdiagnosis_mapped targetが
Phase 2 StepPlanへ正しく注入されたが、同じdiagnostic blockがverify stepにも付加され、
"write-pressure target"文言によりplanner lint
"verify step instruction must not request file changes"でexecutor実行前に停止した。
planner lintやhonest-failureを弱めず、診断/target guidanceを変更可能stepにだけ安全に渡すか、
verify instructionを非変更表現のdiagnostic contextとして保持する境界を設計する。

観測2: hook_attribute_presentのroute-bound predicate failureはcompile/Python診断を持たず、
hook Run 3/4/5がpackage.json(required_path)へwrite圧力を解決してread-only stagnationした。
FIX-2のcatalog suggestionに既に含まれるroute-bound path=src/app/page.tsxを、契約predicate
failureのdiagnosis/target lineageとしてPhase 2へ安全に接続できるか調査する。

必須制約: docs/fix-intent-contract.md v0 fixed、F1〜F3、baseline/lineage/epoch/regression-set gate、
create byte互換、event schemaを弱めない。まずissue化、根因、最小修正案、Run 1型とRun 3型の
focused regression fixture、既存fix conformance 9本とcreate byte fixtureの検証計画まで提示し、
実装はレビュー承認後に行う。
```

## 実行規律とartifact

- 外側runは指定順に6本、各1回だけ実行。再試行0。
- panic 0、OS signal / user interrupt 0、理由なき中断0。
- 各runに`run_start`、`intent_resolved`、`host_env_contamination`、
  `host_env_normalized`、`fix_reproducer_suggested`、`fix_evidence_recorded`、
  `ultra_final_acceptance`、`tui_command_stop`、`run_stop`が各1件。
- 各runにF1 leafとadjudicationを保存。`fix-*.json`は各2、合計12。
- full 0のためF2 / F3 leafは生成されていない。
- `.anvil`のevent stream、計画、snapshot、repair / recovery資料を退避。
- repository guardrailに従い、`node_modules`、`.next`、`.env`、raw `*.log` / `*.out` /
  `*.err`はcommit対象から除外。
- `src/`、`tests/`、`docs/`、台帳、バンドは変更していない。

artifact root: [`artifacts/`](artifacts/)

分析ファイル:

- [`preflight.json`](artifacts/analysis/preflight.json)
- [`source-provenance.json`](artifacts/analysis/source-provenance.json)
- [`run-matrix.json`](artifacts/analysis/run-matrix.json)
- [`event-audit.json`](artifacts/analysis/event-audit.json)
- [`fix-effects.json`](artifacts/analysis/fix-effects.json)
- [`gate-summary.json`](artifacts/analysis/gate-summary.json)
- [`combined-distribution.json`](artifacts/analysis/combined-distribution.json)

以上を`uat-test0717-fix-003`の第3測定記録とする。
