# F-BoN-V 検証1 — Luna BoN確率反復

## 0. 事前宣言（実行前固定）

記載時刻: 2026-08-03 12:14:09 JST (`+0900`)

実行revisionは`baaefb6307616270c865599340d372aae19fb238`（コミット1）とする。
既存`bon0-001`と同じ`cli-filter-bon0.toml`をそのまま使い、
`bon0-002`、`bon0-003`、`bon0-004`の3キャンペーンを、それぞれ別の
workspace rootで6runずつ逐次実行する。goal bytes、suite bytes、binary、planner、
executor、endpoint、tool protocol、context budget、preset、pack absenceは同一とし、
追加・打切り・product terminal再投入・途中選別は行わない。

単発full基準率は事前値`p̂=0.17`、1バッチ6runの期待full本数は
`6 × 0.17 = 1.02`。既存1バッチを含む4バッチ24runの期待full総数は
`24 × 0.17 = 4.08`であり、事前期待帯を`3.5〜4.5`とする。

```text
P(>=1 full in one N=6 batch) = 1 - (1 - 0.17)^6
                              = 1 - 0.83^6
                              = 0.672019...
expected batches with >=1 full among 4
                              = 4 × 0.672019...
                              = 2.688076... ≈ 2.69/4
```

依頼文の`≥1成立 3.4/4`は、同じ依頼文で固定された`p̂=17%`、`N=6`とは
算術的に両立しないため、上記の**2.69/4**へ事前訂正する。full総数期待4.08と
3.5〜4.5帯は訂正不要である。

独立性の事前判定はキャンペーン別full本数`X_i`だけを使う。p値は計算しない。

```text
expected binomial variance = 6 × 0.17 × 0.83 = 0.8466
dispersion ratio           = sample_variance(X_i) / 0.8466
ratio < 0.5                = underdispersed（過少分散）
0.5 <= ratio <= 1.5        = binomial_consistent（二項整合）
ratio > 1.5                = overdispersed（過分散）
```

samplingは同一性項目と分離して「分散は資源」と扱う。明示seedとtemperatureを
固定共有せず、各runの独立Responses requestでprovider管理/defaultのsamplingを使う。
selectorはrunごとのexecutor response-id集合を記録し、6run間で集合が分離している
ことを`trial_specific=true`の必要条件とする。

workspace rootは次の3本に固定する。

- `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002`
- `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_003`
- `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_004`

費用見込みは3バッチ合計約`$0.9`。停止条件は、measurement invalid、構成同一性不一致、
sampling分離不成立、scrub不成立、または追加1バッチで`full >= 4`を観測した場合とする。
停止条件に達したら残りキャンペーンを開始せず、その乖離を結果として報告する。

### 0.1 環境停止後の再開裁定（再実行前固定）

記載時刻: 2026-08-03 12:29:28 JST (`+0900`)

初回`bon0-002`はprovider turn 0の環境欠落であり、統計試行を消費していない。
ユーザーが`.env`の`OPENAI_API_KEY`を使用するよう指示した後、値を表示せず子プロセス
だけへexportしてLuna/Responses doctorを実行し、key set（redacted）・Responses endpoint
reachable・Ollama reachable・state writableを確認した。

失敗terminalは再投入しない。有効な`bon0-002`は新しいcampaignとworkspace root
`/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002_valid`
を使う。suite SHA、revision、N、goal bytes、停止条件、費用見込み、選別規則、
分散比閾は0節から変更しない。失敗attemptは分母外証拠として永久保持する。

### 0.2 bon0-003 preflight停止後の再開裁定（再実行前固定）

記載時刻: 2026-08-03 15:04:12 JST (`+0900`)

最初の`bon0-003`開始要求はproduct実行前のgit-clean preflightで停止した。
`bon0-002`のC3 violation 1件が専用detached worktreeの既存calibration台帳へ
追記されていたためで、provider turn、API費用、統計試行は0、作成されたrootは
空である。原C3実物は`bon0-002`の外部artifactに保存済みであり、追記はdiff確認後に
専用worktreeだけから除去した。`git status --short`が空であることを再確認した。

空rootは再利用せず、有効な`bon0-003`には新しいroot
`/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_003_valid`
を使う。suite SHA、revision、N、goal bytes、停止条件、費用見込み、選別規則、
分散比閾は0節から変更しない。

## 1. 実測

事前宣言後、clean detached worktreeから`bon0-002`を開始した。preflightはgit
clean、minimum ancestor、`cargo test`、release build、installed binary SHA一致、
version確認までgreenだった。しかしproduct起動時に6runすべてが0秒、exit 1となった。

```text
error: OPENAI_API_KEY is not set.
```

provider turnは0で、API費用は発生していない。これはfull 0/6というモデル実測ではなく、
環境欠落による**invalid measurement**である。事前停止条件に従い、`bon0-003`と
`bon0-004`は開始していない。失敗した6 terminalを再投入していない。

- failed attempt campaign:
  `cli-filter-bon0-20260803-031558`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002`
- metadata SHA-256:
  `ea11bbab4bd77c2330373373fac24c3d4584e3cd26b5e79c70f7e6ab6f327003`
- report skeleton SHA-256:
  `7f162d4f3612f629fe4be0c59a1e8391ae0bf44ec7bf4be6e6c78e4234380e30`
- campaign scrub: green、findings 0

ユーザー指示後の0.1節どおり、新しいrootから有効な`bon0-002`を実行した。
6runはすべて自然終端し、campaign scrubはgreen（findings 0）、selectorの
`valid_measurement=true`、構成`all_equal=true`、sampling
`trial_specific=true`だった。

- valid campaign: `cli-filter-bon0-20260803-033022`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_002_valid`
- suite SHA-256: `209843c8bab4fcfdc028b3220383694c50b0b5e46360c284e4a83a8586cbf500`
- metadata SHA-256: `44101e9605113f46210e54a1a7cfa7dd92b09f2d7b3bfeca9947e8a712887501`
- report skeleton SHA-256: `dda5b908e7bff90cb7b041ca99dddd4502ec87fa3510aeceb77486ac0c8880d9`
- result evidence: `evidence/bon0-002-selection.json`
- result evidence SHA-256:
  `9e272fa853ee726917ca5402ac95f3b5efc2b1dbfea5f9740bf9d0e0ecb967c8`
- full: `1/6`（採用`filter_bon0_002`）
- reached: `5/6`
- score five-number: `25.0 / 62.5 / 62.5 / 62.5 / 100.0`
- API費用: `$0.270849`
- 総所要: `8964秒`

## 2. 合算検算

初回の環境失敗6件は統計分母へ混入しない。有効な`bon0-002`は期待full
`1.02`に対して実測`1`で、差は`-0.02`。追加バッチ停止閾`full >= 4`には
達していない。

途中検算として、既存`bon0-001`と有効`bon0-002`のfull本数は`[1, 1]`、
合計`2/12`である。4バッチ横断の分散比は全4バッチ終了後にだけ確定する。
`bon0-003`、`bon0-004`は事前宣言を変更せず逐次実行する。

### 2.1 bon0-003実測

有効な`bon0-003`は6runすべて自然終端し、campaign scrubはgreen（findings 0）、
selectorの`valid_measurement=true`、campaign内の構成`all_equal=true`、sampling
`trial_specific=true`だった。

- valid campaign: `cli-filter-bon0-20260803-060453`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0803_bon0_003_valid`
- suite SHA-256: `209843c8bab4fcfdc028b3220383694c50b0b5e46360c284e4a83a8586cbf500`
- metadata SHA-256: `c9a923405d33a46cb9124183a3c3ad305d8b02e5680457daf03cc389cddedd15`
- report skeleton SHA-256: `5670ac674d99dbbf0d13fb96916c0ca534f3109b1b098a040399d2bd3def475e`
- result evidence: `evidence/bon0-003-selection.json`
- result evidence SHA-256:
  `747b5402f2f5e880f22c76cab915098ad64ce2778d0f4e539972dc9719ab40ce`
- full: `0/6`（報告専用most-promising loserは`filter_bon0_002`）
- reached: `5/6`
- score five-number: `25.0 / 25.0 / 62.5 / 62.5 / 75.0`
- API費用: `$0.2572805`
- 総所要: `8187秒`

既存`bon0-001`を含む途中3バッチのfull本数は`[1, 1, 0]`、合計`2/18`、
fullありバッチは`2/3`である。3バッチ時点の事前期待はfull合計`3.06`、
fullありバッチ`2.019.../3`なので、後者はほぼ期待どおりだが、full合計は
`-1.06`少ない。機械計算した標本分散は`0.333333...`、二項期待分散
`0.8466`、分散比`0.3937317899`で、事前閾`0.5`未満の
`underdispersed`となった。これは4バッチ未完かつ後述の構成不一致を含む途中値であり、
独立性の最終統計主張には使わない。

### 2.2 停止判定 — キャンペーン横断binary bytes不一致

停止判定時刻: 2026-08-03 17:26:48 JST (`+0900`)

キャンペーン横断同一性を検算すると、同じrevision・suiteであるにもかかわらず、
実行binary SHA-256が一致しなかった。

| campaign | revision | binary SHA-256 |
|---|---|---|
| bon0-001 | earlier validation revision | `5b77243ec1cdcec36e513cefaf8cd9f2253967a413e8a7c0ea55b4a2a432fb3a` |
| bon0-002 | `baaefb63` | `6eaae73c63088657be366b41a8587fd8f7daa370bb030938daa4018e4f8040c5` |
| bon0-003 | `baaefb63` | `33af36ab2b4807c295f82819c22edff787a57df87d6baba2873dee7bb3bf722d` |

`bon0-002`と`bon0-003`は同一commitだが、各campaignのfull preflightが
`cargo build --release`を再実行し、`build.rs`が現在UTC時刻をbinaryへ埋め込む。
そのため再buildごとにbytesが変化した。campaign内6runは同じbinaryだが、0節で固定した
「binary同一」と「構成同一性不一致なら停止」をキャンペーン横断では満たさない。

この不一致は本来`bon0-002`後に既存`bon0-001`との比較で検出すべきだったが、
その時点ではsuite SHAとcampaign内同一性だけを検算し、cross-campaign binary SHAを
比較しなかった。検出が1キャンペーン遅れたことも隠さず逸脱として記録する。

停止条件に従い`bon0-004`は開始しない。負の対照、ローカル時分割BoN、品質監査、
settlementも開始せず、固定binaryをcampaign間で再利用する仕組みを先に裁定する。
機械可読な停止根拠は`evidence/luna-stop-decision.json`、途中分散検算は
`evidence/luna-partial-independence.json`に保存した。既存calibration台帳へ測定中に
追記されたC2/C3実物は各外部artifactに保存されており、専用worktreeの既存台帳bytesは
HEADへ戻してcleanを確認した。
