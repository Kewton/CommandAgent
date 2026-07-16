# fix intent 初計測レポート（uat-test0717-fix-001）

## 結論

**事前宣言したP0-a / P0-b / P0-c / P1-a / P1-bは全てPASS。**
`d955032`をrelease buildして固定し、歴代live UATの実成果物から調達した
コンパイルエラー2セット、restart契約フック欠落2セットを用いて、指定6セルを
各1回だけ実行した。6/6が分類済みの理由を伴って正直終端し、F1
`before_fails`は全runで最初の`reproduce-before` phaseに実行された。
`intent_resolved`も6/6で`value=fix / origin=cli / source=fix`だった。

fullは1/6（compile-B / gemma31）。このrunは、開始時に失敗した
`npm run build`をF1 `epoch=1`で記録し、同一lineageの同一コマンドをF2
`epoch=2`で成功させた。その後、run開始時に束縛済みの回帰集合
`profile_contract` / `profile_verify_1`を縮小せず、`epoch=3/4`で全件実行成功した。
したがって偽fullは0件である。他の5本はF2/F3を実行できなかったため、全て
`failed(after_not_executed)`を維持した。

full率は契約どおり記録のみとし、能力判定には用いない。分布はcompile 1/3、
hook 0/3、gemma31 1/2、qwen35 0/4だった。ただし全runで
`host_env_contamination: NODE_ENV=production`が記録され、hook/qwen35の2本は
ローカル`node_modules`のdev依存3件不足で停止した。この2セルはhook修正能力を
純粋には表さないため、分布解釈を留保する。実行規律に従い再試行・補正runは
行っていない。

| Gate | 判定 | 計測事実 |
|---|---:|---|
| P0-a 正直終端 | **PASS** | `run_start` / `tui_command_stop` / `run_stop`各6件。completed 1、具体理由付きfailed 5。panic・分類不能・理由なき中断0 |
| P0-b assurance契約 | **PASS** | full 1件だけがF1〜F3全成立。残り5件はF1成立、F2/F3未実行のままfailed。インフレ・デフレ0 |
| P0-c 偽成功ゼロ | **PASS** | full 1件にF1、同一lineage F2、固定回帰2件の実evidenceが存在。false-full 0 |
| P1-a intent解決 | **PASS** | 6/6が`value=fix / origin=cli / source=fix` |
| P1-b F1冒頭実行 | **PASS** | 6/6でPhase 1 `reproduce-before`中に`stage=before / expected=failure / executed=true / outcome=failure / epoch=1`。F1前のwrite 0 |
| 総合 | **PASS** | 宣言済み5条件を全て満たす。full率は記録のみ |

機械可読値は[`gate-summary.json`](artifacts/analysis/gate-summary.json)、
run別値は[`run-matrix.json`](artifacts/analysis/run-matrix.json)、event順序監査は
[`event-audit.json`](artifacts/analysis/event-audit.json)に保存した。

## 対象と固定条件

- 実行日: 2026-07-17（Asia/Tokyo）
- repository: `/Users/maenokota/share/work/github_kewton/CommandAgent-develop`
- branch / HEAD: `develop` / `d9550326b5e0c28e3fbe210ec13f5aa1a46cb0ee`
  （`Document intent CLI resolution`）
- 契約authority: `docs/fix-intent-contract.md` v0 fixed
- measurement workspace:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0717_fix_001`
- report: `workspace/management/runs/uat-test0717-fix-001/`
- planner: Ollama `qwen3.6:27b-coding-nvfp4`
- executor: Ollama `qwen3.6:35b-a3b-coding-nvfp4` / `gemma4:31b`
- profile / preset: `nextjs` / `none`
- context budget: `65536`
- 外側の`commandagent` invocation: 6回。各run最大1回、再試行0回、中断0回
- 同一run内のbounded repair / corrective planningは製品所定経路であり、外側runの
  再試行には数えない
- `time_profile.total_ms`合計: 1,239,468 ms（20分39.468秒）

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
`workspace/management/runs/uat-test0715-ff1-001/`は、内容を変更せず対象パスだけを
一時stashしてcleanを確認し、その後同じ資料を復元した。本コミットには含めない。

| 項目 | 結果 |
|---|---|
| `git status --porcelain` | 一時隔離後に空。tracked差分なし |
| HEAD / origin | `d955032`そのもの、`origin/develop`と一致。`d955032..HEAD`は空 |
| workspace / report新規性 | いずれも開始前に非存在 |
| disk | 335 GiB available |
| Ollama models | planner 1種、executor 2種が全て存在 |
| 権限付き`cargo test` | exit 0。lib 1382 passed / 13 ignored、byte fixture 6/6、conformance 18 passed / 1 ignored、corpus 1/1、data conformance 10/10、fix conformance 9/9、guardrail 7/7を含め全green |
| `cargo build --release` | exit 0 |
| install | `target/release/commandagent`を`/Users/maenokota/.local/bin/commandagent`へinstall |
| target / install SHA-256 | 両方`f81bb2f931a31b20212d114d2e6a0d61a4435dbe4b0d2f72b0fb65eb76ccd912` |
| `commandagent --version` | `commandagent 0.1.0 d955032 2026-07-16T16:28:26Z`、`+dirty`なし |
| `--setup-interaction-probe` | `probe ready: playwright 1.61.1 (managed_interaction_probe)` |
| 3011 listener | 各runの開始前・終了後とも残留なし |

原値は[`preflight.json`](artifacts/analysis/preflight.json)に保存した。

## 壊れた出発点のprovenance

ソースは全て歴代live UATの実成果物であり、合成していない。各セットを独立した
runディレクトリへcopyし、`.git`、過去の`.anvil`、`node_modules`、`.next`、
過去UAT metadata / logsは持ち込まなかった。全6ディレクトリで
`npm install --no-audit --no-fund`がexit 0。copy前後の対象tree SHA-256も一致した。

| Set | 採取元run / event run UUID | 割当 | tree SHA-256 | 事前R確認 |
|---|---|---|---|---|
| compile-A | `gate_breakout_combo2_qwen27_plan_gemma31_exec_preset_profile_001`<br>`019f563b-7381-7860-abd1-34fed72300ac` | Run 1 | `b1c4f06527c651019004bcf5009be8dd421f3e4283a6e92478cc02b7c2d0215b` | `npm run build` exit 1。`src/app/page.tsx:250:5`、`initGame`未定義 |
| compile-B | `space_combo1_qwen27_plan_qwen35_exec_explicit_none_001`<br>`019f5008-f559-7f03-8652-e77e025e220a` | Run 2, 3 | `332c32151692e4c4a13f721a9a697708cf845c9c7f9b206f24e43b0071a7c000` | `npm run build` exit 1。`SpaceInvaders.tsx:305:22`、`Bullet.dy`欠落 |
| hook-A | `nopreset_space_combo1_qwen27_plan_qwen35_exec_001`<br>`019f4c99-902e-7072-a7d6-c35974ab8823` | Run 4, 6 | `d6a52b7f58d479f3cc1b5ab309024a71aaf0df6e8e2b512899e33c49b5e7b406` | restart hook check exit 1。補助buildはexit 0 |
| hook-B | `cell2_space_qwen27_plan_gemma31_exec_preset_profile_001`<br>`019f56a7-634a-71e0-bfb9-4e3a34ad848e` | Run 5 | `fb4ceb97240fb3a83c167de12879289a9ba08d556340b610f135c996a1ea9bda` | restart hook check exit 1。補助buildは別の既存欠陥でexit 1 |

hook-Bの補助buildで見つかった別欠陥は、
`src/app/SpaceInvadersGame.tsx:4:10`がexportされていない
`useSpaceInvadersGame`をimportするものだった。契約フック欠落は事前checkで独立に
確認済みだが、このセットは単一障害ではないことを分布解釈上の注意として残す。

採取元絶対path、全採取ファイル名、各ファイルSHA-256、copy SHA、事前check結果は
[`source-provenance.json`](artifacts/analysis/source-provenance.json)に保存した。

## Run行列

`terminal / final`は`tui_command_stop.status / final_acceptance_status`、時間は同eventの
`time_profile.total_ms`。全runで`ultra_final_acceptance`が1件あり、その
`verdict / assurance_level`を転記した。

| # | run / event run UUID | 族 / set | executor | exit | terminal / final | verdict / assurance | 主要終端 | 時間 |
|---:|---|---|---|---:|---|---|---|---:|
| 1 | `fix1_compile_qwen35_001`<br>`019f6bca-1667-7530-8d36-f3eae5b113e4` | compile / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 read-only stagnation。`initGame` compile errorに対するwriteへ進めず | 57.244 s |
| 2 | `fix1_compile_gemma31_001`<br>`019f6bcb-78df-7231-803e-5d9710247e34` | compile / B | gemma31 | 0 | completed / **full_success** | **full / full** | F1〜F3成立、completed | 389.930 s |
| 3 | `fix1_compile_qwen35_002`<br>`019f6bd2-8dba-76b3-bcbc-cb3677dfb039` | compile / B | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 read-only stagnation。`Bullet.dy` compile errorに対するwriteへ進めず | 76.311 s |
| 4 | `fix2_hook_qwen35_001`<br>`019f6bd4-2b5b-76f3-aed2-8210b8c07cf8` | hook / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 verifyでローカルdev依存3件不足 | 217.009 s |
| 5 | `fix2_hook_gemma31_001`<br>`019f6bd7-dd6d-7890-aa95-19ee93539eca` | hook / B | gemma31 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 read-only stagnation、`package.json` write_required枯渇 | 281.884 s |
| 6 | `fix2_hook_qwen35_002`<br>`019f6bdc-de81-7c12-aae1-76d40d90466b` | hook / A | qwen35 | 1 | failed / failed | failed / failed (`after_not_executed`) | Phase 2 verifyでローカルdev依存3件不足 | 217.090 s |

各runの完全な`stop_reason`、fix run UUID、計画、repair prompt、recovery UltraPlan、
event streamは対応する[`artifacts/`](artifacts/)に保存した。

## F系evidence監査

### 全run要約

| Run | F1 before_fails | F2 after_passes | F3 no_regression | 裁定 |
|---:|---|---|---|---|
| 1 | PASS。`npm run build`、`before/failure`、lineage `reproducer:a33c603932fd7056`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 2 | PASS。`npm run build`、`before/failure`、lineage `reproducer:a33c603932fd7056`、epoch 1、実行failure | PASS。同じ`npm run build` / lineage、`after/success`、epoch 2、実行success | PASS。固定2件をepoch 3/4で全実行success | **full** |
| 3 | PASS。`npm run build`、`before/failure`、lineage `reproducer:a33c603932fd7056`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 4 | PASS。restart hook `grep`、`before/failure`、lineage `reproducer:39f63d6b307e4a98`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 5 | PASS。`page.tsx` semantic restart-hook check、`before/failure`、lineage `reproducer:215ef92b74567a98`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |
| 6 | PASS。restart hook `grep`、`before/failure`、lineage `reproducer:990de9aab424c3d3`、epoch 1、実行failure | not_executed | not_executed | failed (`after_not_executed`) |

全6件で、F1 eventより前の`Edit` / `Write` / `ApplyPatch`成功eventは0件だった。
生成された4段計画も全6件で次の順序に固定されている。

1. `reproduce-before`
2. `isolate-cause`
3. `repair`
4. `verify-regressions`

Run 1 / 3〜6はPhase 1だけを完了してPhase 2で正直に停止した。Run 2だけが4 phaseを
完了した。

### 初full（Run 2）の完全evidence

裁定ファイル:
[`fix-019f6bcb-791c-7ab2-b365-ce933c92c8ac-adjudication.json`](artifacts/fix1_compile_gemma31_001/evidence/fix-019f6bcb-791c-7ab2-b365-ce933c92c8ac-adjudication.json)

| 項目 | 値 |
|---|---|
| schema / intent / contract | `1` / `fix` / `v0` |
| contract ref | `docs/fix-intent-contract.md` |
| fix run id | `019f6bcb-791c-7ab2-b365-ce933c92c8ac` |
| assurance / reason | `full` / 空 |
| requirement statuses | `before_fails=passed`, `after_passes=passed`, `no_regression=passed` |
| fix written | `true` |
| bound regression ids | `profile_contract`, `profile_verify_1` |
| bound regression lineages | `regression:5653a54980e5d888`, `regression:56446385dfddfe26` |

F1 leaf evidence
[`before.json`](artifacts/fix1_compile_gemma31_001/evidence/fix-019f6bcb-791c-7ab2-b365-ce933c92c8ac-before.json):

| Field | Value |
|---|---|
| `schema_version` | `1` |
| `intent` / `contract_version` / `contract_ref` | `fix` / `v0` / `docs/fix-intent-contract.md` |
| `requirement_id` | `before_fails` |
| `binding_id` | `npm run build` |
| `stage` / `expected` | `before` / `failure` |
| `lineage` / `epoch` | `reproducer:a33c603932fd7056` / `1` |
| `run_id` | `019f6bcb-791c-7ab2-b365-ce933c92c8ac` |
| `executed` / `outcome` | `true` / `failure` |
| `reason` | `outcome: CommandFailed status: exit status: 1 elapsed_ms: 2527 summary: Failed to compile. Type error: Argument of type '{ x: number; y: number; }' is not assignable to parameter of type 'Bullet'. stdout: > anvilminimal-nextjs-app@1.0.0 build > next build ▲ Next.js 14.2.35 Creating an optimized production build ... ✓ Compiled successfully Linting and checking validity of types ... stderr: Failed to compile. ./src/app/components/SpaceInvaders.tsx:305:22 Type error: Argument of type '{ x: number; ` |

上の`reason`は製品がevidenceへ保存した上限までの文字列をそのまま転記した。

F2 leaf evidence
[`after.json`](artifacts/fix1_compile_gemma31_001/evidence/fix-019f6bcb-791c-7ab2-b365-ce933c92c8ac-after.json):

| Field | Value |
|---|---|
| `schema_version` | `1` |
| `intent` / `contract_version` / `contract_ref` | `fix` / `v0` / `docs/fix-intent-contract.md` |
| `requirement_id` | `after_passes` |
| `binding_id` | `npm run build` |
| `stage` / `expected` | `after` / `success` |
| `lineage` / `epoch` | `reproducer:a33c603932fd7056` / `2` |
| `run_id` | `019f6bcb-791c-7ab2-b365-ce933c92c8ac` |
| `executed` / `outcome` / `reason` | `true` / `success` / `command_succeeded` |

F3 leaf evidence 1
[`regression-profile_contract.json`](artifacts/fix1_compile_gemma31_001/evidence/fix-019f6bcb-791c-7ab2-b365-ce933c92c8ac-regression-profile_contract.json):

| Field | Value |
|---|---|
| `schema_version` | `1` |
| `intent` / `contract_version` / `contract_ref` | `fix` / `v0` / `docs/fix-intent-contract.md` |
| `requirement_id` / `binding_id` | `no_regression` / `profile_contract` |
| `stage` / `expected` | `after` / `success` |
| `lineage` / `epoch` | `regression:5653a54980e5d888` / `3` |
| `run_id` | `019f6bcb-791c-7ab2-b365-ce933c92c8ac` |
| `executed` / `outcome` / `reason` | `true` / `success` / 空 |

F3 leaf evidence 2
[`regression-profile_verify_1.json`](artifacts/fix1_compile_gemma31_001/evidence/fix-019f6bcb-791c-7ab2-b365-ce933c92c8ac-regression-profile_verify_1.json):

| Field | Value |
|---|---|
| `schema_version` | `1` |
| `intent` / `contract_version` / `contract_ref` | `fix` / `v0` / `docs/fix-intent-contract.md` |
| `requirement_id` / `binding_id` | `no_regression` / `profile_verify_1` |
| `stage` / `expected` | `after` / `success` |
| `lineage` / `epoch` | `regression:56446385dfddfe26` / `4` |
| `run_id` | `019f6bcb-791c-7ab2-b365-ce933c92c8ac` |
| `executed` / `outcome` / `reason` | `true` / `success` / 空 |

F1とF2のbindingとlineageは完全一致し、`2 > 1`。F3の実測件数2はrun冒頭で
束縛した集合2件と一致し、縮小0である。leaf 4件は全て`executed=true`で、
期待polarityどおりだった。初fullのevidence 5ファイルを省略せずartifactへ保存した。

### 初fullのsource変化

Run 2で変化したアプリsourceは
`src/app/components/SpaceInvaders.tsx`だけだった。SHA-256は
`8747dc66eb1bb117a5b4967056b8ebad42638d2440b2c58c611bd437e18107b1`から
`4631b1d83dbbf71c42de3b7cd2178e97e817e36169880ee693b723305618528b`へ変化した。
実パッチは[`first-full.patch`](artifacts/analysis/first-full.patch)に保存した。

変更は2つのbullet生成値に`as any`を加えるものだった。契約§2のとおり、修正の
設計品質はfix intentの恒久的スコープ外であり、本レポートはこの変更の筋の良さ、
最小性、美しさを評価・主張しない。主張するのはF1〜F3の実測成立だけである。

## intent_resolved監査

全6 eventが次の同一値だった。欠落、重複、default origin、create解決は0件。

```json
{"event":"intent_resolved","origin":"cli","schema_version":"1","source":"fix","value":"fix"}
```

`run_start.model`、`planner_model`、`profile`、`plan_preset`も指定行列と6/6一致した。

## 偽装耐性の実戦観測

| 拒否対象 | 発生 | 観測 |
|---|---:|---|
| before / after lineage不一致 | 0 | **未行使**。唯一F2へ達したRun 2は同一lineage |
| 回帰集合の縮小・不一致 | 0 | **未行使**。Run 2は束縛2件を2件とも実行 |
| after epochがbefore以前 | 0 | **未行使**。Run 2はbefore 1、after 2 |
| 未実行probeからのfull | 0 | fullのleaf 4件は全て`executed=true` |
| `baseline_not_reproduced` | 0 | 6/6でF1が実行failure |

ネガティブ拒否コードはこの自然run集合では発火しなかったため、3種の拒否そのものは
「未行使」と記録する。これは拒否能力の未確認をPASSへ読み替えるものではない。
本ゲートで確認したのは、正常fullがlineage・集合・epoch・provenance条件を満たし、
条件未達5本がfullへ昇格しなかったことである。

## baseline_not_reproduced突合

`baseline_not_reproduced`は0件。出発点の事前確認とrun内F1は次のとおり整合した。

- compile-A: 事前buildとRun 1 F1が同じ`initGame`未定義でfailure。
- compile-B: 事前buildとRun 2 / 3 F1が同じ`Bullet.dy`欠落でfailure。
- hook-A: 事前hook checkとRun 4 / 6 F1がrestart属性不在でfailure。
- hook-B: 事前hook checkとRun 5のsemantic checkがrestart属性不在でfailure。

事前に失敗したRがrun内で成功へ変化した例はなく、環境差分調査条件は発火しなかった。

## 実戦観測と解釈上の制限

### host環境とhook 2セル

全6本で次のeventが1件ずつ発行された。

```json
{"contamination":["NODE_ENV=production"],"event":"host_env_contamination","lifecycle_stage":"process","schema_version":"1"}
```

この環境での`npm install`はdev依存をローカルへ配置しなかった。Run 4 / 6では
Next.jsのstrict local dependency checkが`node_modules/tailwindcss`、
`node_modules/postcss`、`node_modules/autoprefixer`の不在を検出し、
`dependency_setup_missing / dependency_setup_authority_required`として停止した。
F1のrestart hook check自体はこれより前に実行failureしているためP1-bは成立するが、
この2本のF2未到達をhook修正能力の純粋な失敗とは解釈しない。

Run 5も同じhost contaminationを記録したが、dependency gateへ到達する前に
read-only stagnationで停止した。また出発点hook-Bには前述の別compile defectがある。
このためhook 0/3は数値としてのみ記録する。

### 原因限定phaseの挙動

5/6はPhase 2 `isolate-cause`で停止した。qwen35のcompile 2本とgemma31のhook
1本は、read-only phase内でwrite_requiredが枯渇した。qwen35のhook 2本は
dependency setup境界で停止した。

唯一のfull Run 2では、`isolate-cause` phase中に`Edit` raw call 3件、成功実行2件が
記録され、source変更がPhase 3ではなくPhase 2で生じた。生成計画のPhase 2 promptは
workspace非変更を要求しており、これは原因限定phaseの運用上の観測事項である。
ただしF1は全editより前に確定し、変更後にF2と固定F3を独立再実行したため、
fix契約v0のfull条件および今回のP0/P1判定とは矛盾しない。実装・契約変更は本タスクの
範囲外なので行っていない。

### 分布（記録のみ）

| 軸 | full | failed | full率 |
|---|---:|---:|---:|
| 全体 | 1 | 5 | 1/6（16.7%） |
| compile | 1 | 2 | 1/3（33.3%） |
| hook | 0 | 3 | 0/3 |
| gemma31 | 1 | 1 | 1/2（50.0%） |
| qwen35 | 0 | 4 | 0/4 |

## 実行規律とartifact

- 外側runは指定順に6本、各1回だけ実行。再試行0。
- panic 0、OS signal / user interrupt 0、理由なき中断0。
- 各runに`run_start`、`intent_resolved`、`ultra_final_acceptance`、
  `tui_command_stop`、`run_stop`が各1件。
- 各runにF1 leafとadjudicationを保存。Run 2にはさらにF2とF3 2件を保存。
- `fix-*.json`数はRun 1 / 3〜6が各2、Run 2が5。
- `.anvil`のevent stream、計画、snapshot、repair / recovery資料を退避。
- repository guardrailに従い、`node_modules`、`.next`、raw `*.log`はcommit対象から除外。
- `src/`、`tests/`、`docs/`、台帳、バンドは変更していない。

artifact root: [`artifacts/`](artifacts/)

分析ファイル:

- [`preflight.json`](artifacts/analysis/preflight.json)
- [`source-provenance.json`](artifacts/analysis/source-provenance.json)
- [`run-matrix.json`](artifacts/analysis/run-matrix.json)
- [`gate-summary.json`](artifacts/analysis/gate-summary.json)
- [`event-audit.json`](artifacts/analysis/event-audit.json)
- [`first-full.patch`](artifacts/analysis/first-full.patch)

以上を`uat-test0717-fix-001`の初回測定記録とする。
