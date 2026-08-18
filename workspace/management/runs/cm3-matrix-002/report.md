# 結果サマリ

- **preflight**: 親 `0a96702cda17b9a6d47c479cedf793b0d6553fde` はCI
  `32110390113`、acceptance `32110390172` ともに`completed/success`。
  親green規律を満たしてから着手した。
- **コミット**: `260fa853`（B/D失敗8件の原文解剖）、`6352bbdb`
  （Community L3 build material計画ゲート）、本清算コミット（B′/D′
  24run・band/map/台帳）。
- **受理条件**: ローカルfull suiteは2,091 pass / 34 ignored / 0 fail。
  CI/acceptanceの対象HEAD最終run IDとconclusionはpush後のhandoffで確定する。
  golden/schema/adversarialの外側manifest SHA-256は順に
  `4ea74f2f…dad86` / `6242f354…72c1` / `792c9696…2b0b`、全entry照合もpass。
  A/C相当の初期Community guidance本文は変更せず、qwen27 L2計画の検査前後
  JSON bytes一致、非Community UltraPlan非発火、既存adjudication 6 fixtureと
  `--summary-json`未指定stdout fixtureのbyte一致を確認した。
- **解剖・較正**: D旧失敗5件はpackageが計画に無い3件=伝達、specを計画に
  書いたがexecutorが作らない2件=model。伝達3件だけをL3 UltraPlan/StepPlan
  必須ゲートへ昇格した。B旧失敗3件は完全語彙が届いた上で`validations`を省略
  したmodel帰属のため補修していない。
- **実測**: B′ 12/12完走・full 11/12=91.7% [Wilson 64.61%, 98.51%]、
  D′ 12/12完走・full 7/12=58.3% [31.95%, 80.67%]。D→D′のfull率差は
  0.0pp [Newcombe 95% CI −34.57pp, +34.57pp]、p50は32.5→31.0秒。
  `community_package_missing`は3→0件だが、別のmodel/L3署名5件が残った。
- **判定**: 事前線「修復込みfull≥90%かつp50≤30秒」に対し、D′は
  58.3%（−31.7pp）かつ31.0秒（+1.0秒）。**30秒×高full率アームは不成立**。
- **数値**: live 24/24、run所要合計2,471秒、campaign区間2,519秒、
  provider費用$0.07468386（予算$3の2.49%）。返却model ID 117件でdrift 0。
- **未実施・逸脱・新規床**: D′開始時に`.env`変数未exportで0run目停止1回
  （provider支出/run directory生成とも0、分母非消費）。full suite初回で
  `lint.rs` tripwireが1行超過し、baselineを上げずleaf配線へ移して3行縮小、
  再実行green。新規観測署名4クラスはmodel帰属で登録し、本較正では補修なし。

# CM-3b calibrated tier matrix

## 1. 事前宣言

この節はcampaign開始前の2026-08-18T16:45:25+09:00に記録した。

- 分母はB′ 12 + D′ 12 = 24 run。各アームは封緘golden 3種×4観測。
- B′はplanner=`qwen3.6:27b-coding-nvfp4`/Ollama、executor=
  `qwen3.5-9b-mlx`/LM Studio Chat Completions native。matrix-001 Bと比較する。
- D′はplanner=`gpt-5.6-luna`、executor=`gpt-5.6-luna`、ともにOpenAI
  Responses native。matrix-001 Dと比較する。
- 比較計測なので一般Go/No-Go閾値も点予測も置かない。full率はWilson 95%
  CI、較正前後の率差はNewcombe hybrid-score 95% CIを併記する。
- 「30秒×高full率」D′は、修復込みfullの点推定が90%以上、かつrun所要
  p50が30秒以下の両方を満たす場合にのみ成立と判定する。
- 合算予算上限は$3。超過見込みなら次のprovider支出前に停止する。
- 計器ピンはrevision `6352bbdbc46577d905d4c4a88c40b3fb00587615`、
  campaign binary SHA-256
  `d11b216d09d7b483becad1317c200b45477708d357838fe21fb2ad48299eeded`。
- provider到達性、宣言modelの厳密ID、suite pin、binary pinをrun作成・支出前に
  照合する。環境中断はrun非消費とする。
- golden/schema/adversarialの既存封緘manifestは改訂しない。B′/D′ goal値は
  matrix-001 B/Dからbyte値一致で導出し、新arm suite自体を別SHAで封緘する。

n=12/armのため、以下の読みはすべて「この分母では」に限定する。

## 2. Fullの意味

CommunityのL2 `full`はspec検証済み（schema/拘束/材料）を意味する。
runtime smokeはプラットフォーム統合の被覆であり、L2で実測済みとは主張しない。
L3/L4 `full`は有効なpromotionとS+Z+B全件passを意味する。

## 3. B/B′・D/D′対比

| arm | planner / executor | 一発full [Wilson 95% CI] | 修復込みfull [Wilson 95% CI] | p50 / p95 | cost min / p50 / p95 / max / total |
|---|---|---:|---:|---:|---:|
| B | qwen27 / qwen3.5-9b-mlx | 7/12=58.3% [31.95%, 80.67%] | 9/12=75.0% [46.77%, 91.11%] | 180.5 / 237.8秒 | $0 / $0 / $0 / $0 / $0 |
| B′ | qwen27 / qwen3.5-9b-mlx | 10/12=83.3% [55.20%, 95.30%] | 11/12=91.7% [64.61%, 98.51%] | 157.5 / 210.70秒 | $0 / $0 / $0 / $0 / $0 |
| D | Luna / Luna | 7/12=58.3% [31.95%, 80.67%] | 7/12=58.3% [31.95%, 80.67%] | 32.5 / 136.45秒 | $0.00267000 / $0.00470730 / $0.02334046 / $0.02902692 / $0.10150708 |
| D′ | Luna / Luna | 7/12=58.3% [31.95%, 80.67%] | 7/12=58.3% [31.95%, 80.67%] | 31.0 / 101.35秒 | $0.00309880 / $0.00453013 / $0.01528275 / $0.02377208 / $0.07468386 |

| comparison | 修復込みfull率差 [Newcombe 95% CI] | 一発full率差 [Newcombe 95% CI] | p50差 | p95差 |
|---|---:|---:|---:|---:|
| B→B′ | +16.67pp [−14.82, +45.72] | +25.00pp [−10.93, +53.97] | −23.0秒 | −27.1秒 |
| D→D′ | 0.00pp [−34.57, +34.57] | 0.00pp [−34.57, +34.57] | −1.5秒 | −35.1秒 |

B側は解剖でmodel帰属となり製品較正を行っていないため、B→B′差は再標本差で
あり較正効果とは呼ばない。D側はpackage計画欠落だけが介入対象で、対象署名は
3→0件になった一方、総fullは7→7である。CIは広く、母集団効果を断定しない。

## 4. タスク別

| arm | warikan full / p50 / p95 | mochimono full / p50 / p95 | vote full / p50 / p95 |
|---|---:|---:|---:|
| B′ | 3/4 / 153.5 / 169.75秒 | 4/4 / 171.0 / 187.9秒 | 4/4 / 154.0 / 224.3秒 |
| D′ | 1/4 / 37.0 / 140.0秒 | 3/4 / 32.0 / 51.55秒 | 3/4 / 27.5 / 29.7秒 |

## 5. 失敗署名と較正境界

| arm | stop class | 件数 | 帰属・扱い |
|---|---|---:|---|
| B′ | `community_schema_pin_path_invented` | 1 | model。正準`.sha256`でなく未宣言`.sha256sums`をread。補修なし。 |
| D′ | `community_verify_instruction_not_executable` | 2 | model。verifyへ`not applicable`/自然言語を記載。補修なし。 |
| D′ | `community_esbuild_script_missing` | 1 | model。packageは計画・生成したが必須scriptなし。 |
| D′ | `community_spec_artifact_missing` | 1 | model。発明path `app.spec.yaml^^^^invalid?`をread。 |
| D′ | `community_package_artifact_missing` | 1 | model。計画にpackageを明記したが作成前read。 |

`d_prime_mochimono_003`のUltraPlan原文は「`package.json と
package-lock.json も作成または更新`」、StepPlan expected_pathsも両pathを列挙した。
それでもexecutorは`path does not exist: package.json`で停止したため、旧Dの
「計画に無い」伝達床ではなく、計画に書いた後のmodel不実行と判定した。

## 6. 計器・費用・証跡

- B′の0run目ゲートはOllama `/api/tags` 25 model、LM Studio `/v1/models`
  18 model、両宣言ID存在を確認。D′はOpenAI keyを`<redacted>`表示し、
  `/v1/models` status 200を確認した。
- B′ 33、D′ 84の返却model ID観測でrequested/returned driftは0。
- B′ costはlocal provider $0。D′ totalは$0.07468386、予算使用率2.49%。
- 全24 artifactのrun単位scrubはpass。campaign全体scrubは同梱release binaryを
  oversize/secret文字列として検知するため、生campaignとbinaryはrepositoryへ
  取り込まず、scrub済み集計・exact hash・原文引用だけを封緘した。
- B′ `uat-meta.json` SHA-256は`535d1245…0c38`、D′は`2a4e0482…4252`、
  curated `summary.json`は`73344c62…34a`。

機械可読な24行とeffect/CIは`summary.json`、pinは`arm-suite.sha256sums`、
`predeclaration.sha256sums`、`sealed-manifest-outer.sha256sums`、raw evidenceの
exact hashは`evidence-hashes.json`を正本とする。

## 7. 検証

- Rust: `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings`、
  `cargo test` 2,091 pass / 34 ignored / 0 fail。
- bytes/non-regression: `adjudication_compat` 6/6、`headless_summary` 2/2、
  qwen27 L2 plan JSON byte一致、nextjs UltraPlan非発火、tripwire green。
- Python: `test_cm3b_matrix.py` 3/3、Ruff green。
- 封緘: golden 3/3、schema 7/7、adversarial 22/22のentry照合pass。

## 8. 結論

この分母では、Dのpackage計画伝達床はゲート化により消えた。しかしD′全体は
7/12 fullのままで、p50も31秒だった。従って階級地図の残り2枚は確定したが、
「30秒×高full率」のLuna planner/Luna executorアームは成立しない。
