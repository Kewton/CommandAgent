# 結果サマリ

- **preflight**: 親 `f429ecaf77900a423e04c468d7bd07d81c989c3d` は CI
  `32100263387`、acceptance `32100263372` ともに `completed/success`。
  親green規律を満たしてから着手した。
- **コミット**: `b913268f` はGPT-5.6 TerraのF-0統合、`a9e73c71` は
  4アーム定義・事前宣言・計測集計。本レポート、band、map、台帳と失敗
  class登録は清算コミットに含める。
- **受理条件**: ローカルfull suiteは2,089 pass / 34 ignored / 0 fail。
  元golden manifest SHA-256
  `4ea74f2fe2687989467a9019c4f72a160d38e77097ea441e6de4a066748dad86`
  を全entry照合し不変。既存profile経路は6 fixtureのbyte一致と
  `--summary-json`未指定fixtureで不変。対象HEADのCI/acceptance最終run IDと
  conclusionはpush後のhandoffで確定値を記録する。
- **実測**: 比較分母48（Aの既存12を再掲、B/C/Dの36を新規実測）。Bは
  full 9/12、Cは12/12、Dは7/12。n=12/armのため、以下の読みはすべて
  「この分母では」に限定する。
- **数値**: 新規live計測36/36完走、provider費用合計$0.30367758
  （予算$8の3.80%）、run所要合計4,959秒、計測開始から終了まで5,032秒。
  Terra/LM Studioとも返却model ID driftは0件。
- **未実施・逸脱・新規床**: 新規live campaignはfull suiteを重複実行せず
  `--skip-suite-tests`を使用（同一binaryのfull suiteを事前に完走し、CIを
  最終正本とする）。設計外の床・環境中断はなし。Dで観測した2つの
  model帰属失敗署名はclassesへ登録したが、比較計測内では補修していない。

# CM-3 planner × executor tier matrix

## 1. 事前宣言

この節はcampaign開始前の2026-08-18T14:20:18+09:00に記録した。

- 分母は4アーム×12観測=48。各アームは封緘golden 3種×4観測。
- Aは`cm2b-golden-008`の各種001–004を固定引用し、再計測しない。B/C/Dは
  36 live観測を追加する。
- 比較計測なのでアーム別Go/No-Go閾値も点予測も置かない。率にはWilson
  95% CIを併記する。
- 予算上限は$8。超過見込みなら追加支出前に停止する。
- 計器ピンはrevision
  `b913268f8f045bb77dc07a320b597740f0542877`、実行binary SHA-256
  `3487e1ef08fc9ff462824a999e21e46652e02eec77d8db952e9b3fd949a35a44`。
- A: planner=`qwen3.6:27b-coding-nvfp4`/Ollama、executor=
  `gpt-5.6-luna`/OpenAI Responses native。
- B: planner=`qwen3.6:27b-coding-nvfp4`/Ollama、executor=
  `qwen3.5-9b-mlx`/LM Studio Chat Completions native。
- C: planner=`qwen3.6:27b-coding-nvfp4`/Ollama、executor=
  `gpt-5.6-terra`/OpenAI Responses native。
- D: planner=`gpt-5.6-luna`、executor=`gpt-5.6-luna`、ともにOpenAI
  Responses native。
- provider到達性、宣言modelの厳密ID存在、binary SHAはrun directory生成・
  provider支出より前に照合する。

アーム定義は`arm-suite.sha256sums`で封緘した。goal文字列は元goldenから
byte同一で導出した。

## 2. Fullの意味

CommunityのL2 `full`はspec検証済み（schema/拘束/材料）を意味する。
runtime smokeはプラットフォーム統合の被覆であり、L2で実測済みとは
主張しない。L3/L4 `full`は有効なpromotionとS+Z+B全件passを意味する。
generic summaryの`assurance`表示をFullへ読み替えてはいない。

## 3. 4アーム対比

| arm | planner / executor | 一発full [Wilson 95% CI] | 修復込みfull [Wilson 95% CI] | p50 / p95 | cost min / p50 / p95 / max / total |
|---|---|---:|---:|---:|---:|
| A（既存引用） | qwen27 / Luna | 9/12 = 75.0% [46.77%, 91.11%] | 10/12 = 83.3% [55.20%, 95.30%] | 181.5 / 218.5秒 | $0.00086468 / $0.00106994 / $0.00238086 / $0.00252714 / $0.01611224 |
| B（executor down） | qwen27 / qwen3.5-9b-mlx | 7/12 = 58.3% [31.95%, 80.67%] | 9/12 = 75.0% [46.77%, 91.11%] | 180.5 / 237.8秒 | $0 / $0 / $0 / $0 / $0 |
| C（executor up） | qwen27 / Terra | 11/12 = 91.7% [64.61%, 98.51%] | 12/12 = 100% [75.75%, 100%] | 179.0 / 206.55秒 | $0.01100575 / $0.01656550 / $0.02555535 / $0.02793850 / $0.20217050 |
| D（planner up） | Luna / Luna | 7/12 = 58.3% [31.95%, 80.67%] | 7/12 = 58.3% [31.95%, 80.67%] | 32.5 / 136.45秒 | $0.00267000 / $0.00470730 / $0.02334046 / $0.02902692 / $0.10150708 |

Aは旧計器による固定引用で、B/C/Dとの直接差には計器世代の交絡がある。
この分母ではCがfull率最大、BはAとほぼ同じ所要でfullが1件少なく、Dは
p50が短い一方fullが7/12だった。CIは広く、母集団順位や因果は断定しない。

## 4. タスク別

| arm | warikan full / p50 / p95 | mochimono full / p50 / p95 | vote full / p50 / p95 |
|---|---:|---:|---:|
| A | 4/4 / 170.0 / 200.8秒 | 2/4 / 187.5 / 195.95秒 | 4/4 / 181.5 / 227.5秒 |
| B | 3/4 / 179.0 / 214.4秒 | 2/4 / 197.0 / 252.85秒 | 4/4 / 167.5 / 181.95秒 |
| C | 4/4 / 170.5 / 184.5秒 | 4/4 / 191.0 / 214.0秒 | 4/4 / 179.0 / 195.15秒 |
| D | 2/4 / 72.0 / 160.85秒 | 3/4 / 24.5 / 31.8秒 | 2/4 / 35.5 / 83.5秒 |

## 5. 失敗署名と成果物レベル

| arm | level | stop class |
|---|---|---|
| A | L2 12 | `community_computed_unregistered` 1、`community_l2_verify_invocation_incomplete` 1 |
| B | L2 12 | `community_spec_closed_vocabulary` 3 |
| C | L2 12 | なし |
| D | L2 9 / L3 3 | `community_package_missing` 3、`community_spec_artifact_missing` 2 |

Bはローカルexecutorが閉語彙から外れた3件でfail closedした。DはL3を3件
選んだがpackage manifestを生成せずB系で3件fail closedし、別の2件は必須
`app.spec.yaml`未生成で停止した。これは「どの階級で何が壊れたか」の
観測であり、検証を緩和する理由にはしない。

## 6. F-0/F-0bと費用

- Terraは厳密ID `gpt-5.6-terra`のみを受理し、曖昧aliasと未公表snapshot
  風IDを拒否する。公式にdated snapshotがないためdoctorはexact IDを受理し、
  公表後のpin移行を推奨する。
- Terra smokeはrequested/returnedとも`gpt-5.6-terra`、Responses/native、
  3.087秒、cost $0.00123000。公式単価はinput $2.50/M、cached input
  $0.25/M、output $15/Mをpricing正本へ出典URL付きで登録した。
- LM Studioは`qwen3.5-9b-mlx`を厳密宣言。smokeとBの47応答でreturned ID
  drift 0。CのTerra 34応答、DのLuna 106応答もdrift 0。
- doctorはkey値を`<redacted>`表示し、curated metadata/reportと36 artifact
  treeのscrubは0 finding。local generationはprovider cost $0として記録し、
  電力費は計測していない。

## 7. 証跡

| evidence | SHA-256 |
|---|---|
| arm B `uat-meta.json` | `d27f511d04bb59c148670c9c2d96ef9a3b49c0f10737b41c056efca8cbc87895` |
| arm C `uat-meta.json` | `7840344957c4a5ea9965888c165ac4673daed6844738f60101f06356847d627f` |
| arm D `uat-meta.json` | `d68453858c6dc515e6320275cde5e05419d5953aa98f471031dc2803b1b99f56` |
| arm B predeclaration | `755d6f02e4e9ccd04e96f4268c12ea57b547b4ee8e379ca5c368c725afe35abe` |
| arm C predeclaration | `89907cdb176068901525304d6877abc2179a7487735efc4eecd07251aad6af2b` |
| arm D predeclaration | `5724147c85e13d7145a8a1a74e87162514c3ef03a36bc8dbb7df3d9b4dc1e958` |

全48行の機械可読値は`summary.json`、アーム封緘値は
`arm-suite.sha256sums`、provider単発証跡は隣接する
`../cm3-terra-smoke/`を正本とする。生campaignは資格情報・raw logを
repositoryへ取り込まず、hashとscrub済み集計だけを封緘した。

## 8. 実装規模と検証

- F-0統合コミット: 611行追加、28行削除（製品、テスト、docs、証跡）。
- マトリクスコミット: 集計/費用script 469行追加・18行削除、test 124行、
  suite定義292行、機械集計2,492行。
- Rust: `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、
  full suite 2,089 pass / 34 ignored / 0 fail。
- Python: bench/cost/matrix focused test green、Ruff green。

