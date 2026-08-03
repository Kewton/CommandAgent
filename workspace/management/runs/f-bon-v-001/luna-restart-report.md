# F-BoN-V 検証1再開 — Luna BoN確率反復

## 0. 新事前宣言（実行前固定）

記載時刻: 2026-08-03 18:08:43 JST (`+0900`)

実行revision、suite、計器binaryを次へ固定する。

- revision: `49002050dc00ddab15e6709ebbba7f1beb5a3c7f`
- suite: `workspace/management/bench/suites/cli-filter-bon0.toml`
- suite SHA-256: `f66b1c65095c86ed814e448d39ba984350fb94c0dd018b0e7d3d0467e34a1761`
- binary SHA-256: `3fa2978aed3fc09aadc84ae873133bc477117bb03bf4116cc5092cee91c68988`
- series id: `f-bon-v-cli-luna`

計器SHAはコミットAのclean checkoutを別target directoryで2回release buildし、
version文字列とbinary bytesが両方一致した値である。各campaignはv1事前宣言を
`--bon-predeclaration`で渡し、revision・suite・binaryのいずれかが不一致なら
product起動前にfail closedする。

旧`bon0-002/003`は系列ピン未装備かつcross-window binary不一致のため除外する。
旧`bon0-001`のbinary SHAも新ピンと不一致だったため除外し、条件分岐どおり4窓を
すべて新規調達する。旧窓のfull実測は改変せず、再開統計分母へだけ入れない。

新規campaign labelと独立workspace rootを次へ固定する。

- `bon0-001r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_001r`
- `bon0-002r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002r`
- `bon0-003r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_003r`
- `bon0-004r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_004r`

各窓は`Luna × N=6`、同じfilter goal bytes、suite bytes、binary bytes、planner、
executor、endpoint、tool protocol、context budget、preset、pack absenceを使う。
追加・打切り・product terminal再投入・途中選別は行わない。

単発full基準率は`p̂=0.17`、1窓の期待full本数は`6 × 0.17 = 1.02`。
4窓24runの期待full総数は`4.08`、事前期待帯は`3.5〜4.5`とする。

```text
P(>=1 full in one N=6 window) = 1 - 0.83^6
                                  = 0.672019118431...
expected windows with >=1 full   = 4 × 0.672019118431...
                                  = 2.688076473724... ≈ 2.69/4
```

依頼者の旧値`3.4/4`は実行前訂正済みで、台帳の裁定者誤り系譜へ記録した。

独立性は窓別full本数`X_i`を使い、p値は計算しない。

```text
expected binomial variance = 6 × 0.17 × 0.83 = 0.8466
dispersion ratio           = sample_variance(X_i) / 0.8466
ratio < 0.5                = underdispersed
0.5 <= ratio <= 1.5        = binomial_consistent
ratio > 1.5                = overdispersed
```

samplingは明示seed・temperatureを窓内/窓間で固定共有せず、各runの独立Responses
requestでprovider管理/defaultを使う。selectorの全run response-id集合が記録済み・
集合内unique・run間disjointであることを`trial_specific=true`の条件とする。

旧有効2窓の平均費用`$0.26406475`から、新規4窓の点見積りを`$1.056259`、
費用見込みを約`$1.2`とする。停止条件は、preflight pin不一致、measurement invalid、
構成同一性不一致、sampling分離不成立、scrub不成立、または単一窓で`full >= 4`を
観測した場合。停止条件に達したら後続窓を開始せず、乖離を結果として持ち帰る。

## 1. 実測

### 1.1 v1 preflight（支出前停止）

`bon0-001r`のworkspace rootを新規作成し、v1事前宣言を指定してpreflightを
開始した。`cargo test`はgreen、release buildも完了したが、固定clean worktreeの
`target/release/commandagent` SHA-256は
`1eb01906f23524da10463b35bf4ea5d58cf40078f4672431f2f6f25a69f18de1`であり、
別target directoryで採取した宣言値`3fa2978a...`と一致しなかった。
preflightは設計どおりinstall・product起動前にfail closedした。

- campaign directory: `cli-filter-bon0-20260803-091056`
- `uat-meta.json`: なし
- run artifact: 0件
- product/API trial: 0本
- 統計分母: 0本
- 費用: `$0`

差分検算では、2 binaryの実行コードを含む通常領域は同一で、差はMach-Oの
`LC_UUID` 16 bytesと、それを覆うlinker生成ad-hoc署名32 bytesだけだった。
version文字列はどちらも
`commandagent 0.1.0 49002050 2026-08-03T18:05:55+09:00`で一致した。
Apple linkerの`-no_uuid`は現行macOSで実行不能なbinaryを作るため採用せず、
異なるtarget directoryのraw SHAを系列ピンへ転記した手順誤りとして扱う。

固定clean worktreeの同じ`target/release`で`cargo build --release`を再実行すると
buildはfreshとなり、SHA-256は再び`1eb01906...`で一致した。以後の系列ピンは
全campaignが実際に使うこの固定build pathのraw binary SHAとする。

### 1.2 v2新事前宣言（実行前固定）

記載時刻: 2026-08-03 18:20:29 JST (`+0900`)

v1の統計期待、sampling規律、停止条件は変更しない。計器ピンだけを固定build
pathの実測へ訂正し、支出0の`bon0-001r` preflight窓を除外して、代替窓を
`bon0-001r2`として宣言する。

- revision: `49002050dc00ddab15e6709ebbba7f1beb5a3c7f`
- suite SHA-256: `f66b1c65095c86ed814e448d39ba984350fb94c0dd018b0e7d3d0467e34a1761`
- binary SHA-256: `1eb01906f23524da10463b35bf4ea5d58cf40078f4672431f2f6f25a69f18de1`
- `bon0-001r2`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_001r2`
- `bon0-002r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002r`
- `bon0-003r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_003r`
- `bon0-004r`: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_004r`

この節を含む実行前report SHAと全宣言値は
`evidence/luna-restart-predeclaration-v2.json`へ固定する。

### 1.3 `bon0-001r2`実測→検算

- campaign: `cli-filter-bon0-20260803-092213`
- workspace root: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_001r2`
- metadata SHA-256: `c61fc967d6d7f8d0c787c962d1656141125c9476fbafdefaa5a7e5cded7d8d41`
- report skeleton SHA-256: `8813c9792376cb59098479993c1ca232099b840f382f9f223c4d294df02a3ba4`
- selection: `evidence/bon0-001r2-selection.json`
- selection SHA-256: `007ff4f8ddbae852df0b82dd1fe57e19800b7e002032f6430a9d58e0097195f3`
- valid measurement: `true`（invalid reason 0件）
- full: `1/6`、採用`filter_bon0_004`
- reached: `6/6`
- score five-number: `25.0 / 34.4 / 62.5 / 71.9 / 100.0`
- sampling: `trial_specific=true`、run間response-id集合disjoint
- identity: 全run一致、binary SHA `1eb01906...`、v2 pin照合一致
- 費用: `$0.3193204`
- run所要合計: `8880`秒
- scrub: green（finding 0件）

事前期待full `1.02`に対する実測は`1`で差`-0.02`。単窓`full >= 4`停止条件、
invalid、構成不一致、sampling不一致、scrub不成立はいずれもない。測定中に追記された
calibration 4件はcampaign artifactに保存され、selectorで件数を記録した後、専用
worktreeの既存台帳bytesだけをHEADへ戻してgit cleanを再確認した。したがって次窓へ進む。

### 1.4 `bon0-002r`実測→検算

- campaign: `cli-filter-bon0-20260803-115414`
- workspace root: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002r`
- metadata SHA-256: `bae56a22982a4296c0be01494ab85388b26ec8477068862aee9852d81b95e060`
- report skeleton SHA-256: `d96802df8690a97208ab7bcc42e46639024d082af4803101a1d2ca755c7d0973`
- selection: `evidence/bon0-002r-selection.json`
- selection SHA-256: `36906fa06336de79cbb724ca3918b2721b8e8157cc60b5be57f4bea756064d92`
- valid measurement: `true`（invalid reason 0件）
- full: `0/6`（報告用most-promising loserは`filter_bon0_006`）
- reached: `3/6`
- score five-number: `25.0 / 25.0 / 25.0 / 31.2 / 37.5`
- sampling: `trial_specific=true`、run間response-id集合disjoint
- identity: 全run一致、binary SHA `1eb01906...`、v2 pin照合一致
- 費用: `$0.2199217`
- run所要合計: `6193`秒
- scrub: green（finding 0件）

2窓暫定full分布は`[1, 0]`、合計`1`（期待`2.04`）、`>=1`成立窓は`1/2`
（期待`1.3461`）。sample varianceは`0.5`、分散比は`0.5905976849`で事前閾内の
`binomial_consistent`。単窓停止条件を含む全停止条件に該当しない。暫定検算は
`evidence/luna-restart-independence-partial-2.json`（SHA-256
`ceae43fdc90b86ba8bae5888c043f7170e5d3e4f19835260e92461e1b19c6552`）へ保存した。
calibration追記2件はartifactへ保存後、専用worktreeの既存台帳bytesを復元した。

### 1.5 `bon0-003r`実測→検算

- campaign: `cli-filter-bon0-20260803-134215`
- workspace root: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_003r`
- metadata SHA-256: `8188dac381fa29c0d4a2be629db1db7fb88d42637717102072db7a10b8149a2f`
- report skeleton SHA-256: `bffa284dc0f0b224df7944e5475c3223872429c466987f7af0e3c27ea8695b94`
- selection: `evidence/bon0-003r-selection.json`
- selection SHA-256: `54aac00003bc7bb8cd1c0eb4482a6ee2d150d34063cdbfc1be5972180f274c8b`
- valid measurement: `true`（invalid reason 0件）
- full: `0/6`（報告用most-promising loserは`filter_bon0_006`）
- reached: `4/6`
- score five-number: `25.0 / 53.1 / 62.5 / 62.5 / 62.5`
- sampling: `trial_specific=true`、run間response-id集合disjoint
- identity: 全run一致、binary SHA `1eb01906...`、v2 pin照合一致
- 費用: `$0.2499581`
- run所要合計: `8366`秒
- scrub: green（finding 0件）

3窓暫定full分布は`[1, 0, 0]`、合計`1`（期待`3.06`）、`>=1`成立窓は`1/3`
（期待`2.0192`）。sample varianceは`0.3333333333`、分散比は`0.3937317899`で
事前閾上の`underdispersed`となった。一方、18試行の二項分布でfull合計が1以下となる
確率は約16.4%であり、単窓停止条件を含む停止条件には該当しない。第4窓で事前宣言済みの
4窓検定を確定する。暫定検算は
`evidence/luna-restart-independence-partial-3.json`（SHA-256
`a65fa1799e2449cae9d37a290efbc66dc11183d5f03101f5afae7a460e19c927`）へ保存した。
calibration追記1件はartifactへ保存後、専用worktreeの既存台帳bytesを復元した。

### 1.6 `bon0-004r`実測→検算

- campaign: `cli-filter-bon0-20260803-160629`
- workspace root: `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_004r`
- metadata SHA-256: `6f7acc4dd50996b66912c1736c092175a52a425a4092fd233cfb3ab04387d8ca`
- report skeleton SHA-256: `2b40045b586972f2db6a33bee18c32540044757251db7503db579f45d7cb54a5`
- selection: `evidence/bon0-004r-selection.json`
- selection SHA-256: `90ffc20b51a725ca2f1035f755471c1f0a9bad4c9ffa1e8d963e754a4b80103a`
- valid measurement: `true`（invalid reason 0件）
- full: `0/6`（報告用most-promising loserは`filter_bon0_004`）
- reached: `5/6`
- score five-number: `-12.5 / 25.0 / 25.0 / 62.5 / 62.5`
- sampling: `trial_specific=true`、run間response-id集合disjoint
- identity: 全run一致、binary SHA `1eb01906...`、v2 pin照合一致
- 費用: `$0.2375415`
- run所要合計: `8705`秒
- scrub: green（finding 0件）

事前期待full `1.02`に対する実測は`0`で差`-1.02`。measurement、構成、sampling、
scrubの各妥当性条件は成立した。calibration追記5件はartifactへ保存後、selectorで
件数を記録し、専用worktreeの既存台帳bytesを復元した。

## 2. 合算検算

### 2.1 事前宣言→実測

| 指標 | 事前宣言 | 実測 |
|---|---:|---:|
| 1窓あたりfull | `1.02` | `[1, 0, 0, 0]` |
| 4窓full合計 | `4.08`（期待帯`3.5〜4.5`） | `1/24` |
| `>=1`成立窓 | `2.6922 ≈ 2.69/4` | `1/4` |
| full率 | `17%` | `4.17%` |
| 分散比 | 閾`0.5 / 1.5` | `0.2952988424` |
| 独立性判定 | ratioによる事前分類 | `underdispersed` |

全4窓の実測費用は`$1.0267417`、run所要合計は`32144`秒だった。旧除外窓の既支出
`$0.5281295`は統計分母・この4窓費用から除外する。4窓検算は
`evidence/luna-restart-independence.json`（SHA-256
`82ce8db2a38e765bd56e31e25ee34779b784fce1f5ca8c13a299d975b396414a`）へ保存した。

### 2.2 検算→停止裁定

二項`n=24, p=0.17`の単純参照では`P(full合計 <= 1) = 0.067589...`である。
p値による独立性判定は事前宣言していないため、これは乖離の大きさを読む補助値に留める。
事前宣言した主判定では、full合計`1`は期待帯`3.5〜4.5`を明確に下回り、分散比も
`0.5`未満の`underdispersed`である。一方、全24runのprovider response-id集合は
run間disjointであり、sampling試行別の機構条件は成立した。したがって観測はinvalidや
同一リクエスト再利用では説明できず、「17%基準率の再現不成立」と「窓間full数の過少分散」
をそのまま主要結果とする。

依頼者の「期待と大きく乖離した時点で停止」に従い、ここで有償・ローカルを含む新規計測を
停止する。検証2（gemma負対照）、検証3（ローカル時分割）、品質監査、settlementは
未実行であり、結果を推測・補完しない。再開には、この乖離を採択した上での新しい指示と
新しい事前宣言を要する。

### 2.3 事後算術訂正（predeclaration原本は不変）

最終読み合わせで、v1/v2事前宣言の`>=1`期待に使った中間値だけに裁定者算術誤りを
発見した。原本の`1 - 0.83^6 = 0.672019118431...`と4窓`2.688076473724...`は誤りで、
正しくはそれぞれ`0.673059626631...`、`2.692238506524...`である。丸めた事前宣言
`2.69/4`は変わらない。また、full総数期待`4.08`、期待帯`3.5〜4.5`、観測`1/24`、
分散比`0.2952988424`には影響せず、大幅乖離による停止裁定も変わらない。

実行済み事前宣言を書き換えると全campaignが保存したpredeclaration SHAとの照合を
破壊するため、v1 SHA `f6e47b76...`、v2 SHA `4ecb7544...`と本文の事前宣言節は
不変のまま保持する。訂正値と影響範囲は
`evidence/luna-restart-arithmetic-correction.json`へ別記し、台帳の裁定者誤り系譜へ
追記した。independence計器は入力`p=0.17, N=6`から正しい値`2.692238506524`を
再計算しており、最終検算JSONに誤った中間値は伝播していない。

## 3. Luna確率検証の正直な清算（CLOSE）

### 3.1 基準率の出所、プール値、CI

「17%」の出所は、投影是正後のLuna-007と床返済後のLuna-008がそれぞれfull
`1/6`を得た合計`2/12 = 16.67%`である。これは母比率ではなく小標本の点推定で、
Wilson 95% CIは`4.70%〜44.80%`だった。元の宣言は分母を本文に持っていたが、CIを
併記せず、この点推定を既知の`p`として二項期待へ代入した。

同じResponses/native後期系列で採用可能な窓を、後知恵による窓追加なしで次のように
プールする。

| source | full / denominator |
|---|---:|
| Luna-007 | `1/6` |
| Luna-008 | `1/6` |
| bon0-001 | `1/6` |
| pinned restart 4窓 | `1/24` |
| **pool** | **`4/42 = 9.5238%`** |

プール`p̂=4/42`のWilson 95% CIは`3.7662%〜22.0651%`である。Luna-006以前は
投影・床条件が異なるため、この後期系列の分母へ混ぜない。restart前のbon0-002/003も
計器不一致で除外したままとし、結果を都合よく足し引きしない。機械可読な分母、CI、
source集合、close裁定は`evidence/luna-close-decision.json`へ固定する。

### 3.2 仮説aを主因とし、仮説bを排除しない

- 仮説a（主因）: `2/12`の95% CIが`4.70%〜44.80%`と広いことから、17%は小分母の
  上振れを含む不安定な値であり、`4/42`への回帰は標本誤差でまず説明できる。
- 仮説b（排除不能）: Luna-007/008、bon0-001、restart系列は日付、revision、binary
  が完全同一ではない。providerのsystem fingerprintも全turn `null`であり、日時版
  snapshotまたは供給側ドリフトによる基準率変化を積極的には否定できない。

restart 24run内ではbinary、suite、input pinが一致し、response-id集合もrun間disjoint
だったため、同一リクエスト再利用や計器invalidを主因とはしない。しかしこれは系列を
跨ぐ定常性を証明しない。したがって仮説aを簡潔な主説明としつつ、仮説bを残す。

### 3.3 BoNの実用結論

点推定`p̂=4/42≈9.5%`でも、6独立試行で1件以上fullを得る確率は

```text
1 - (1 - 4/42)^6 = 0.451463... ≈ 45.1%
```

である。Wilson CI端点を同じ式へ写した参考範囲は`20.6%〜77.6%`。単発9.5%から
6試行45.1%への増幅は本物だが、45%は保証ではなく誤差棒も広い。BoN-0の実用結論は
「独立な失敗個体を保持しつつ、earned fullだけを末尾選別すれば成功機会を増幅できる」
であり、17%の再現を前提にしない。費用は6倍なので、selection gapとdiversityを含む
品質・資源勘定はsettlementで別軸評価する。

このLuna確率検証は`4/42`で**CLOSE**する。数字を追う追加Luna窓は実施しない。
後続のgemma負対照、ローカルBoN、品質監査は別仮説・別事前宣言として進める。
