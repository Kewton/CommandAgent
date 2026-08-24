# fix intent 第4計測レポート（uat-test0717-fix-004）

## 結論

事前宣言したP0-a / P0-b / P0-cとP1-aはPASS、P1-bはFAILとなった。FIX-4を含む
`b99b624`をclean release buildして固定し、歴代live UATの実成果物から新規copyしたcompile
2本、restart hook 4本を指定順に各1回だけ実行した。6/6が具体的理由を伴って正直に
`failed`終端し、assuranceのインフレ・デフレ、偽full、panic、理由なき中断は0だった。
fullは0/6である。

FIX-4aは事前gateを満たした。#3のcompile 2本を止めた
`verify step instruction must not request file changes`は0/2となり、両runともPhase 2 executorが
実行された。保存StepPlanの実行済みinstructionにはF1診断由来のファイル・行・symbolが
2/2で入り、対応する`step_prompt_contract`、provider turn、tool executionまで連続している。
Run 2ではさらにverify失敗後のrepair objectiveへ`305:22`と`Bullet`型不一致が到達した。
ただしraw provider request bodyは製品が保存せず、`prompt_body_saved=false`である。このため
本レポートの「prompt原文」は、永続化されたStepPlan instructionと対応する実行eventの連鎖を
一次資料とし、HTTP payloadのbyte保存を主張しない。

FIX-4bのpredicate配線そのものは4/4で動いた。`contract_attribute_repair_guidance`はhook 4/4、
Phase 2の実行済みinspect instructionも4/4で
`src/app/page.tsx / selection_reason=contract_attribute`を持ち、#3のpredicate起点
`package.json` fallbackは解消した。一方、hook/B・qwenのPhase 2実行後、別の既存欠陥
`useSpaceInvadersGame`非exportを直そうとするprofile-invariant bounded repairが
`package.json / required_path`を筆頭に選んだ。事前基準の
「hook族のpackage.json筆頭フォールバックゼロ」を文字どおり適用すると1/4の再発であり、
P1-bはFAILとする。predicate-scoped配線PASSと全hook runでのstrict gate FAILを混同しない。

| Gate / audit | 判定 | 計測事実 |
|---|---:|---|
| P0-a 正直終端 | **PASS** | 具体理由付きfailed 6/6。`run_start` / `tui_command_stop` / `run_stop`各6件。panic・分類不能・理由なき中断0 |
| P0-b assurance契約 | **PASS** | F1は6/6で実行failure、F2/F3は0/6。全件`failed(after_not_executed)`で契約§4と一致 |
| P0-c 偽成功ゼロ | **PASS** | full claim 0、false-full 0。未実行F2/F3からの獲得0 |
| P1-a FIX-4a実効 | **PASS** | compile旧lint停止2→0。F1診断のlocation/symbolが実行済みexecutor instructionへ2/2到達 |
| P1-b FIX-4b実効 | **FAIL** | predicate配線はpage.tsx 4/4だが、後段profile-invariant repairでpackage.json筆頭が1/4 |
| FIX-2継続 | **PASS** | suggestion 6/6、意味的採用6/6、R関連性逸脱0 |
| intent解決監査 | **PASS** | 6/6が`value=fix / origin=cli / source=fix` |
| 総合 | **FAIL** | strict P1-b不成立。admission close / band更新は行わない |

機械可読値は[`gate-summary.json`](artifacts/analysis/gate-summary.json)、run別値は
[`run-matrix.json`](artifacts/analysis/run-matrix.json)、FIX-2 / FIX-4監査は
[`fix-effects.json`](artifacts/analysis/fix-effects.json)、#1〜#4合算は
[`combined-distribution.json`](artifacts/analysis/combined-distribution.json)に保存した。

## 対象と固定条件

- 選択work item: ユーザー指定UAT `uat-test0717-fix-004`（issue番号の指定なし）
- 実行日: 2026-07-17（Asia/Tokyo）
- repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- branch / HEAD: `develop` / `b99b62419af13091deeb0a79f601beb362c741f2`
  （FIX-4a `63532c6`、FIX-4b `b99b624`を含む）
- 契約authority: `docs/fix-intent-contract.md` v0 fixed
- measurement workspace:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_fix_004`
- report: `workspace/management/runs/uat-test0717-fix-004/`
- planner: Ollama `qwen3.6:27b-coding-nvfp4`
- executor: Ollama `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b`
- profile / preset: `nextjs` / `none`
- context budget: `65536`
- 外側の`commandagent` invocation: 6回。各run最大1回、再試行0回、中断0回
- 同一run内のbounded recovery / repair turnは製品所定経路であり、外側runの再試行には数えない
- `time_profile.total_ms`合計: 1,928,108 ms（32分8.108秒）

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
| HEAD / origin | `b99b624`そのもの、`origin/develop`と一致。`b99b624..HEAD`は空 |
| workspace / report新規性 | いずれも開始前に非存在 |
| disk | 開始時333 GiB available、run後333 GiB available |
| host `NODE_ENV` | `production`。本計測規律どおり記録のみで中断せず |
| Ollama models | planner 1種、executor 2種が全て存在 |
| 権限付き`cargo test` | exit 0。lib 1403 passed / 15 ignored、byte fixture 6/6、conformance 18 passed / 1 ignored、fix conformance 9/9、corpus 1/1、data conformance 10/10、guardrail 7/7、doc tests 2/2を含め全green |
| `cargo build --release` | `cargo clean -p commandagent`後にexit 0 |
| install | `target/release/commandagent`を`/Users/maenokota/.local/bin/commandagent`へinstall |
| target / install SHA-256 | 両方`895e283a36cd87947a31b412377a953a1ba66dae4532f79995c4fc15d0855745` |
| `commandagent --version` | `commandagent 0.1.0 b99b624 2026-07-17T06:05:31Z`、`+dirty`なし |
| `--setup-interaction-probe` | `probe ready: playwright 1.61.1 (managed_interaction_probe)` |
| 3011 listener | 最初のrun前・最後のrun後とも残留なし |

全値は[`preflight.json`](artifacts/analysis/preflight.json)に保存した。

## 壊れた出発点のprovenance

ソースは全て歴代live UATの実成果物であり、合成していない。#2 / #3と同じ採取元から各run用に
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
multi-signal setでもFIX-2は2/2でpage限定Rを提示・採用し、別componentやbuildへR自体は
逸脱しなかった。

live run後に採取元との`rsync -aicn --delete`を全6 copyへ実施した。出力はdirectory、
`package-lock.json`、`package.json`のmtime差だけで、checksum差は0だった。したがって
source/config内容は全件baseline一致、`fix_written=false`も6/6である。採取元絶対path、file count、
copy照合値は[`source-provenance.json`](artifacts/analysis/source-provenance.json)に保存した。

## Run行列

`terminal / final`は`tui_command_stop.status / final_acceptance_status`、時間は
`time_profile.profile.total_ms`。全runで`ultra_final_acceptance`が1件あり、その
`verdict / assurance_level`を転記した。

| # | run / event run UUID | 族 / set | executor | exit | terminal / final | verdict / assurance | 主要終端 | 時間 |
|---:|---|---|---|---:|---|---|---|---:|
| 1 | `fix4_compile_qwen35_001`<br>`019f6eb5-ff59-7aa0-9d2c-84efd8f82311` | compile / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | 旧lintは通過。Phase 2 scaffold inspectでpackage.jsonへのread-only stagnation | 228.171 s |
| 2 | `fix4_compile_gemma31_001`<br>`019f6eb9-c789-71a3-827b-7de383bae41d` | compile / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | 旧lintは通過。compile診断fileへ到達後read-only stagnation | 269.514 s |
| 3 | `fix4_hook_qwen35_001`<br>`019f6ebe-68e6-7f62-b6ea-3fcf781659c7` | hook / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2完了後、repair step instruction length超過 | 354.638 s |
| 4 | `fix4_hook_qwen35_002`<br>`019f6ec4-3257-7ae0-9ec3-962368c2330a` | hook / B | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | page predicateを調査後、別のrelative-import invariantで停止。bounded repairにpackage.json fallback | 284.021 s |
| 5 | `fix4_hook_gemma31_001`<br>`019f6ec8-c049-71a1-8f96-9149b36597fa` | hook / A | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | 正しいpage.tsx contract target上でread-only stagnation | 394.417 s |
| 6 | `fix4_hook_gemma31_002`<br>`019f6ece-ed38-7ef3-bcc1-c4026c564dc9` | hook / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | page predicateを調査後、既存relative-import invariantで停止 | 397.347 s |

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
| 6 | PASS。同じsemantic variant、同lineage、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |

全6件で`stage=before / expected=failure / executed=true / epoch=1`だった。F1 eventより前の
successful Write/Edit eventは0件。生成された計画も6/6で次の4段順序だった。

1. `reproduce-before`
2. `isolate-cause`
3. `repair`
4. `verify-regressions`

全runがPhase 1を完了した。Run 3だけはPhase 2も完了したがPhase 3 plan lintで停止し、他5本は
Phase 2で停止した。F2 / F3へ到達したrunは0、fullは0なので、完全F1〜F3 evidenceの転記条件は
発火しない。全runにF1 leafとadjudication JSONが存在する。

## intent_resolved / FIX-1監査

全6 eventが次の同一値だった。欠落、重複、default origin、create解決は0件。

```json
{"event":"intent_resolved","origin":"cli","schema_version":"1","source":"fix","value":"fix"}
```

hostの`NODE_ENV=production`は6/6で検出され、続く
`host_env_normalized { variables:[NODE_ENV], strategy:unset_inherited,
scope:bounded_process_children }`も6/6で各1回発火した。dev依存不足起因の停止は0である。

## FIX-2継続監査

| Run | basis | suggestion | モデルのR | 採用判定 |
|---:|---|---|---|---:|
| 1 | `goal_failure_kind:build_or_compile` | `profile_catalog:next_build_verify => npm run build` | `npm run build` | exact |
| 2 | 同上 | 同上 | `npm run build` | exact |
| 3 | `goal_contract_attribute:data-anvil-action=restart` | `hook_attribute_present(action,restart,path=src/app/page.tsx)` | 提示されたroute-bound predicate | exact |
| 4 | 同上 | 同上 | 提示されたroute-bound predicate | exact |
| 5 | 同上 | 同上 | insignificant whitespaceを含む同一predicate | semantic variant |
| 6 | 同上 | 同上 | insignificant whitespaceを含む同一predicate | semantic variant |

完全な`suggestion` commandは各runの`events.jsonl`に保存した。意味的採用6/6、byte完全採用4/6、
R関連性逸脱0、literal `baseline_not_reproduced` 0である。#2で観測したbuild選択、全域grep、
別欠陥束縛は再発していない。

## FIX-4a監査

### 旧planner lint停止

compile 2本の全`.anvil`資料に対し
`verify step instruction must not request file changes`を検索した結果は0件だった。
`planner_error`もcompile 0/2であり、両runともPhase 2 step executionへ進んだ。

| Run | 保存Phase 2 plan | 実行された診断含有step | 診断要点 | 実行結果 |
|---:|---|---|---|---|
| 1 | [`plan-019f6eb9-1a0c-7e31-8144-4c2b28d822a4.yaml`](artifacts/fix4_compile_qwen35_001/.anvil/plans/plan-019f6eb9-1a0c-7e31-8144-4c2b28d822a4.yaml) | `inspect-page-source` | `src/app/page.tsx`、line 250、`initGame` | qwen executorが同stepで`Read src/app/page.tsx`を実行 |
| 2 | [`plan-019f6ebc-b01f-7682-aed0-4105219b2d74.yaml`](artifacts/fix4_compile_gemma31_001/.anvil/plans/plan-019f6ebc-b01f-7682-aed0-4105219b2d74.yaml) | `inspect-space-invaders` / `verify-build-failure` | `SpaceInvaders.tsx`、line 305、TypeScript diagnostic | gemma executorがReadとbuildを実行し、repair turnへ進行 |

Run 1のinstruction原文:

```text
Read src/app/page.tsx to examine the context around line 250 where 'initGame' is called,
check its imports, and determine if it is defined locally, imported from another module,
or completely missing.
```

Run 2のverify instruction原文:

```text
Run npm run build to confirm the exact TypeScript error matches the diagnostic at
src/app/components/SpaceInvaders.tsx:305.
```

Run 2ではbuild verifierが同じ`305:22` errorを返し、その後のexecutor/repair objectiveが次を含んだ。

```text
Repair step `verify-build-failure`. Verification failed:
implementation_compile_error: src/app/components/SpaceInvaders.tsx:305:22
Type error: Argument of type '{ x: number; y: number; }' is not assignable to
parameter of type 'Bullet'.
```

`step_prompt_contract`は両runの該当stepに存在し、直後に`provider_turn_duration`とtool executionが
ある。このevent自体は`prompt_body_saved=false`なので、raw request bodyのbyte archivalではなく、
保存されたStepPlan instructionがexecutor prompt builderへ入力され、同じstep idが実行された
連鎖を証拠とする。#3の「planはあるがexecutor開始前に停止」とは異なり、到達2/2である。

ただし修復成功にはつながらなかった。Run 1は後続scaffold inspectでgeneric
`package.json / required_path`へ逸れ、Run 2は正しいdiagnostic fileをwrite圧力targetにした後も
Readだけを繰り返した。FIX-4aの事前gateはlint解消とprompt到達であり、write実効は記録事項として
残す。

## FIX-4b監査

### predicate-scoped配線

hook 4本で次が全て成立した。

- `contract_attribute_repair_guidance` event: 4/4
- event値: `path=src/app/page.tsx`、`attribute=data-anvil-action="restart"`
- Phase 2 planのinspect instructionに
  `Fix F1 profile contract predicate (runtime-bound)` block: 4/4
- 同blockのwrite圧力: `src/app/page.tsx (selection_reason=contract_attribute)`: 4/4
- 同block内の`Contract attribute repair guidance`、欠落属性、位置directive、1行例: 4/4
- guidanceを持つinspect stepのexecutor実行: 4/4

一次資料:

- Run 3:
  [`plan-019f6ec1-ed37-7f71-8093-7db75ca1df21.yaml`](artifacts/fix4_hook_qwen35_001/.anvil/plans/plan-019f6ec1-ed37-7f71-8093-7db75ca1df21.yaml)
- Run 4:
  [`plan-019f6ec8-293e-7201-89d6-81cae7e6b0f5.yaml`](artifacts/fix4_hook_qwen35_002/.anvil/plans/plan-019f6ec8-293e-7201-89d6-81cae7e6b0f5.yaml)
- Run 5:
  [`plan-019f6ecc-c0af-70d2-b111-520f0c3e138b.yaml`](artifacts/fix4_hook_gemma31_001/.anvil/plans/plan-019f6ecc-c0af-70d2-b111-520f0c3e138b.yaml)
- Run 6:
  [`plan-019f6ed2-adb1-7e30-ad68-fbddf44083a2.yaml`](artifacts/fix4_hook_gemma31_002/.anvil/plans/plan-019f6ed2-adb1-7e30-ad68-fbddf44083a2.yaml)

代表block:

```text
Fix F1 profile contract predicate (runtime-bound):
- capability: hook_attribute_present
- write-pressure target: src/app/page.tsx (selection_reason=contract_attribute)

Contract attribute repair guidance:
- classification: contract_attribute_missing
- missing attribute: `data-anvil-action="restart"`
- target source file: `src/app/page.tsx`
...
- data-anvil-action="restart"
```

### package.json筆頭のstrict監査

| Run | predicate target / reason | 後続write圧力 | package.json筆頭 | 終端 |
|---:|---|---|---:|---|
| 3 | `page.tsx / contract_attribute` | target event前にPhase 2完了 | なし | Phase 3 instruction length超過 |
| 4 | `page.tsx / contract_attribute` | 別relative-import invariantのbounded repairで`package.json / required_path` | **あり** | profile invariant failed |
| 5 | `page.tsx / contract_attribute` | `page.tsx / contract_attribute` | なし | read-only stagnation |
| 6 | `page.tsx / contract_attribute` | target選択前に別relative-import invariant停止 | なし | profile invariant failed |

Run 4ではPhase 2のpredicate inspectとverifyを終えた後、既知のhook-B別欠陥をprofile invariantが
検出した。その修復objectiveに対する`read_only_stagnation_feedback`が最終的に次を記録した。

```json
{
  "target_path":"package.json",
  "selected_targets":["package.json","tsconfig.json","postcss.config.js",
    "tailwind.config.ts","src/app/layout.tsx","src/app/page.tsx",
    "src/app/globals.css","src/app/global.d.ts"],
  "selection_reason":"required_path",
  "stage":"write_required"
}
```

これはF1 predicateから直接解決されたtargetではない。しかし事前P1-bは「hook族の
package.json筆頭フォールバックゼロ」とscopeを限定していないため、厳格判定は1/4でFAILとする。

### 新規・残存クラス

Run 3ではPhase 2を完了し、Phase 3のimplement instructionへcontract guidanceが付いたが、
profile contract本文との合算でinstruction length上限を超えた。

```text
phase repair failed: step add-restart-hook instruction is too long
```

これは#3になかった新規停止クラスである。Run 5は正しい`page.tsx / contract_attribute` targetに
到達したがwriteせず、read-only stagnationした。FIX-4bはtarget選択を改善したが、モデルのwrite
実効とinstruction budgetは未解決である。

## Phase 2停滞の残存形

terminal classが`model_stagnation:read_only_loop`だったのはRun 1 / 2 / 5の3本である。Run 4も
terminal表示はprofile invariantだが、そのbounded repair内部でread-only loopと
package.json write_requiredが発火している。したがって数え方を分ける。

| 指標 | #3 | #4 | 備考 |
|---|---:|---:|---|
| compile旧planner lint | 2 | 0 | FIX-4aで解消 |
| terminal read-only stagnation | 3 | 3 | compile 2 + hook-A gemma 1 |
| embedded stagnationを含むrun | 3 | 4 | Run 4のprofile-invariant repairを加算 |
| predicate起点package.json筆頭 | 3 | 0 | FIX-4b scoped改善 |
| hook全経路package.json筆頭 | 3 | 1 | Run 4の後段invariant repair |
| workspace write実行run | 0 | 0 | 改善なし |
| 新規instruction-too-long | 0 | 1 | hook-A qwen Phase 3 |

## 偽装耐性の実戦観測

| 拒否対象 | 発生 | 観測 |
|---|---:|---|
| 開始時から成功するR | 0 | 全6 Rが意図したfailureを再現 |
| before / after lineage不一致 | 0 | 未行使。F2到達0 |
| 回帰集合の縮小・不一致 | 0 | 未行使。F3到達0 |
| after epochがbefore以前 | 0 | 未行使。F2到達0 |
| 未実行probeからのfull | 0 | full claim自体が0。未実行F2/F3を獲得せず |

negative guardが発火する入力は今回生じなかった。F1のexpected failureは6/6で実測され、
F2/F3未実行からassuranceを獲得しない既存の偽装耐性は機能した。

## #1〜#4合算分布

歴史的事実として全24runを保持し、raw分母は変更しない。

### 全24run（raw）

| 族 | executor | full | failed | 計 | 注記 |
|---|---|---:|---:|---:|---|
| compile | qwen35 | 0 | 5 | 5 | #4 Run 1を追加 |
| compile | gemma31 | 1 | 3 | 4 | #1にfix intent初full |
| hook | qwen35 | 0 | 8 | 8 | #1の2本は環境留保 |
| hook | gemma31 | 0 | 7 | 7 | 全てfailed |
| **合計** |  | **1** | **23** | **24** | full率1/24 |

族別はcompile 1/9 full、hook 0/15。executor別はqwen35 0/13、gemma31 1/11。

### #1の環境留保2本を除くadmission表示

レビュー済みの扱いに従い、`uat-test0717-fix-001`のqwen35 hook 2本だけを表示分母から除く。
歴史的runは削除せず、raw表に残す。

| 族 | executor | full | failed | 計 |
|---|---|---:|---:|---:|
| compile | qwen35 | 0 | 5 | 5 |
| compile | gemma31 | 1 | 3 | 4 |
| hook | qwen35 | 0 | 6 | 6 |
| hook | gemma31 | 0 | 7 | 7 |
| **合計** |  | **1** | **21** | **22** |

表示full率は1/22。族別はcompile 1/9、hook 0/13。executor別はqwen35 0/11、gemma31 1/11。

## Admission / band判定

本計測でP0群とP1-aはPASSし、FIX-4bのpredicate target配線も4/4で確認できた。しかしstrict
P1-bはpackage.json筆頭1件によりFAILし、6本ともF2/F3へ到達していない。したがって本UAT単独で
admissionをcloseすること、またはfix×nextjs初バンドを確定することは提案しない。
タスク指定どおり台帳・バンドは変更していない。

観測された次の支配クラスは、契約を弱めず別タスクとして扱う必要がある。

1. Phase 2 inspect stepからgeneric required pathへwrite圧力を発生させるcompile/qwen経路。
2. 正しいcompile targetを与えてもReadだけを返すexecutor経路。
3. contract guidanceとprofile contract本文の合算によるimplement instruction length超過。
4. predicateとは別のprofile invariant repairでcontract targetが失われ、package.jsonへ戻る経路。

## Artifacts index

- [`preflight.json`](artifacts/analysis/preflight.json)
- [`source-provenance.json`](artifacts/analysis/source-provenance.json)
- [`run-matrix.json`](artifacts/analysis/run-matrix.json)
- [`event-audit.json`](artifacts/analysis/event-audit.json)
- [`fix-effects.json`](artifacts/analysis/fix-effects.json)
- [`combined-distribution.json`](artifacts/analysis/combined-distribution.json)
- [`gate-summary.json`](artifacts/analysis/gate-summary.json)
- [`fix4_compile_qwen35_001`](artifacts/fix4_compile_qwen35_001/)
- [`fix4_compile_gemma31_001`](artifacts/fix4_compile_gemma31_001/)
- [`fix4_hook_qwen35_001`](artifacts/fix4_hook_qwen35_001/)
- [`fix4_hook_qwen35_002`](artifacts/fix4_hook_qwen35_002/)
- [`fix4_hook_gemma31_001`](artifacts/fix4_hook_gemma31_001/)
- [`fix4_hook_gemma31_002`](artifacts/fix4_hook_gemma31_002/)
