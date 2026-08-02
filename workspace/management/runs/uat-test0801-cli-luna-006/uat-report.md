# uat-test0801-cli-luna-006: OpenAI Responses/native再計測

実施日: 2026-08-02 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

計測revision: `8b2c97498ffdf0fd86137e20f5a473ed979862f6` (`develop`)

## 1. 結論

**Responses API/native function toolsは112/112 turnで成立し、text系列の
endpoint/parser壁を越えてC系へ5/6到達した。C3はpass 2、violation 1、
claims_absent 2で、Lunaが正直な出力例を書ける実物を2runで得た。fullは0/6。**

native tool callは115件、endpoint rejection 0、text parse failure 0。
reasoning stateを含む後続turnが継続し、provider eventはresponse ID、usage、
reasoning tokensを全112 turnで記録した。C3 passはstats 1、filter 1で、
Gemma正式窓のC3捏造拒否6件に対し「階級と正規endpointで動く壁」を初めて実測した。

一方、C1の字義placeholder束縛2件、Python実行互換性1件、C3 violation 1件、
claims absent 2件が残った。さらに`stats_luna_002`はC1/C2/C4 pass・C3
`claims_absent`にもかかわらず`failed`へ投影された。契約§4ならpartialであるため、
P0-bは**fail**とする。偽成功ではなく過小評価だが、Responses経路とは別の既存CLI
assurance投影gapとして正直に記録し、本バッチでは修正範囲を広げていない。

## 2. Campaign境界

- campaign: `cli-create-luna-20260802-005454`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0801_cli_luna6`
- suite: `cli-create-luna`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`, `api=responses`, `tool_protocol=native`
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gpt-5.6-luna` / `openai`
- run matrix: stats×3、filter×3
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

既知の未追跡`history.txt` 2件は移動・削除・stashせず、同一HEADのdetached clean
worktreeからbenchを実行した。preflightはgit clean、minimum ancestor
`527bdc1e`、`cargo test`、release buildの全てがgreen。built/installed binary
SHA-256はともに
`13ce4a2018662346df0d47fd19d77eaf138a5f997db8b2128f65b6c1fcc5e635`、
versionは`commandagent 0.1.0 8b2c9749 2026-08-02T00:57:24Z`だった。

## 3. Run行列

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止主因 | 秒 | reasoning | 費用 |
|---|---|---|---|---|---|---|---|---|---:|---:|---:|
| `stats_luna_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | phase README verify bounded repair exhaustion | 1,157 | 931 | $0.036806 |
| `stats_luna_002` | stats | failed | failed (`cli_assurance_failed`) | pass | pass | claims_absent | pass | profile assurance incomplete | 1,365 | 1,231 | $0.053263 |
| `stats_luna_003` | stats | failed | failed (`cli_assurance_failed`) | fail | pass | **pass** | pass | C1 placeholder polarity | 1,757 | 1,009 | $0.055712 |
| `filter_luna_001` | filter | failed | failed (`cli_assurance_failed`) | pass | pass | **fail** | pass | C3 output mismatch | 1,697 | 2,282 | $0.068289 |
| `filter_luna_002` | filter | failed | failed (`cli_assurance_failed`) | fail | fail | claims_absent | pass | generated Python runtime incompatibility | 1,472 | 1,966 | $0.059332 |
| `filter_luna_003` | filter | failed | failed (`cli_assurance_failed`) | fail | pass | **pass** | pass | C1 placeholder polarity | 1,207 | 980 | $0.039585 |

全runのharness statusは`completed`、product exitは1。fullは0/6、C到達は5/6。
C3のrun分布はpass 2 / violation 1 / claims_absent 2 / not reached 1。

## 4. Responses/nativeとreasoning状態

| run | provider turns | native tool calls | input | cached input | output | reasoning |
|---|---:|---:|---:|---:|---:|---:|
| `stats_luna_001` | 14 | 14 | 77,305 | 65,899 | 3,135 | 931 |
| `stats_luna_002` | 20 | 20 | 134,195 | 119,058 | 4,370 | 1,231 |
| `stats_luna_003` | 22 | 25 | 164,461 | 149,819 | 4,348 | 1,009 |
| `filter_luna_001` | 20 | 20 | 151,701 | 135,733 | 6,458 | 2,282 |
| `filter_luna_002` | 18 | 18 | 127,749 | 112,405 | 5,458 | 1,966 |
| `filter_luna_003` | 18 | 18 | 101,869 | 91,198 | 3,299 | 980 |
| 合計 | **112** | **115** | **757,280** | **674,112** | **27,068** | **8,399** |

112 responseのうち111 responseが1件以上のfunction callを返し、合計115 call。
残る1 responseはmessage item後のfeedback turnで、後続turnがfunction callへ復帰した。
全turnが`api=responses`かつ`native_tools_enabled=true`で、HTTP 400は0、
text tool parse failureは0だった。各turnの`provider_response_id`とreasoning usageが
存在し、出力itemを履歴へ戻した後続turnでもnative callが継続したことをliveで確認した。

## 5. C3実物監査

### 正直な転記: `stats_luna_003`

README claim (`README.md:18->24`):

```text
count: 5
sum: 75
average: 15
```

実行argvと観測:

```text
python3 cli/main.py --input data/sample.csv --column amount
exit 0
count: 5
sum: 75
average: 15
```

判定: `matched=true`。

### 正直な転記: `filter_luna_003`

README claim 1 (`README.md:21->22`)と実出力:

```text
error: unable to connect to the database.
error: request returned an invalid response.
```

README claim 2 (`README.md:29->30`)と実出力:

```text
2
```

README help claim (`README.md:36->37`)は実`--help`の先頭行
`usage: main.py [-h] --pattern PATTERN [--count] input_file`と一致した。
3/3 claimsが`matched=true`。

### 捏造拒否: `filter_luna_001`

README claim:

```text
Banana smoothies are popular for breakfast.
Banana and cherry work well in a fruit salad.
```

実行`--pattern banana data/sample.txt`はexit 0だがstdoutは空。READMEのcount
claim `2`に対し、`--pattern banana --count data/sample.txt`の実出力は`0`。
help全文claimも実argparse出力の空行・`positional arguments`節・折返しと不一致だった。
`lemon --count`のclaim `1`のみ一致し、全体は3 violations / 1 matchedとして拒否した。

`stats_luna_002`と`filter_luna_002`は出力claim blockがなく
`claims_absent`。捏造と同一視していない。

## 6. C1/C2/C4監査と残存gap

- C4: 到達5/5でpass。
- C2: 4/5 pass。`filter_luna_002`だけ生成CLIが起動前TypeErrorとなりfail。
- C1: 2/5 pass。`stats_luna_003`と`filter_luna_003`はREADME先頭の
  `PATH`/`COLUMN`または`INPUT_FILE`/`PATTERN`を字義argvへ束縛し、実在sampleを
  使う後続例より先に選んだため正常caseがexit 1/2。モデルは後続の具体例とC3出力を
  正しく書いており、C1 extractorのplaceholder語彙被覆gapである。
- `filter_luna_002`: `str | None`を使った生成CLIが計測環境Pythonで
  `TypeError: unsupported operand type(s) for |: 'type' and 'NoneType'`となり、C1/C2 fail。

自動分類はknown 6 / UNKNOWN 0（registry表示はmachine 1 / model 5）。ただし
`cli_output_claims:observed_stdout_mismatch`を過去の機械gap classへ一律対応するため、
`filter_luna_001`のlive不一致もmachine表示になる。上記実物監査ではモデルのREADME
不一致として分離し、既定class形状を最終帰属とはみなさない。

## 7. Gemma基準線・Luna窓推移

| arm | denominator | full | C到達 | C3判定 | 主停止 | 合計秒 | 費用 |
|---|---:|---:|---:|---|---|---:|---:|
| Gemma正式Window B (`elev-004`) | 6 | 0/6 | 2/6 | README捏造6/6をviolation拒否 | model 5、machine 1 | 3,739 | 記録なし |
| Luna 001 | 6 | 0/6 | 0/6 | 未判定 | endpoint 400 / machine BLOCKED | 2,571 | $0.000000 |
| Luna 002 | 6 | 0/6 | 0/6 | 未判定 | endpoint 400 / machine BLOCKED | 2,488 | $0.000000 |
| Luna 003 (`text`) | 6 | 0/6 | 0/6 | 未判定 | text tool-call shape | 2,123 | $0.038459 |
| Luna 004 (`text`+自己記録) | 6 | 0/6 | 0/6 | 未判定 | a=4、b=2 | 2,598 | $0.118284 |
| Luna 005 (`text`+dialect repair) | 6 | 0/6 | 0/6 | 未判定 | repair後b/empty | 3,069 | $0.202160 |
| Luna 006 (`responses`+native) | 6 | 0/6 | **5/6** | **pass 2 / fail 1 / absent 2** | C1 2、C2 1、C3 1、投影1 | 8,655 | **$0.312987** |

Luna合算n=36はfull 0/36、C到達5/36、費用$0.671890。001/002はmachine
BLOCKED、003〜005はtext bridge、006はResponses nativeの正式比較窓として区分する。
006により「reasoning系フロンティアでtext橋は不適、Responsesが正門」というendpoint
裁定と、「C3証言壁はモデル階級で動くことがある」の両方を実測した。

## 8. ドリフト探針

6run、112/112 turnsでrequested/returned modelは`gpt-5.6-luna`、service tierは
`default`。system fingerprintはprovider未提供の`null` 112/112で、版同一性を
積極的には証明しない。response IDは112/112で相異なる実値を記録した。

## 9. コスト

provider turn eventのreturned usageを合計し、2026-08-02確認の公式Luna単価
（uncached input $1.00/M、cached input $0.10/M、output $6.00/M）を適用した。

- uncached input: 83,168 tokens = $0.083168
- cached input: 674,112 tokens = $0.067411
- output: 27,068 tokens = $0.162408
- campaign計: **$0.312987**
- reasoning: 8,399 tokens（output内数）
- preflight開始: epoch `1785632094`
- run開始: epoch `1785632270`
- run終了: epoch `1785640926`
- run合計: 8,655秒
- preflight開始→run終了: 8,832秒

## 10. E-0検収とscrub

- 自動分類: known 6 / UNKNOWN 0
- 自動検収シート: 6/6
- calibration collector: C3 violation 3件 appended
- campaign scrub再実行: green、findings 0
- `OPENAI_API_KEY`実値のexact scan: 251 files、matches 0
- `.env`は読取り元に使っただけで変更・commitしていない

## 11. 合否

- P0-a 6/6正直終端: **pass**
- P0-b 契約§4投影: **fail**
  （`stats_luna_002`のC1/C2/C4 pass + C3 claims_absentがpartialでなくfailed）
- P0-c 偽成功ゼロ: **pass**
- 資格情報scrub: **pass**
- Responses/native transport: **pass**（112/112 turns、115 tool calls）
- 記録値 full: 0/6
- 記録値 C到達: 5/6
- 記録値 C3: pass 2 / fail 1 / claims_absent 2
- 記録値 OpenAI費用: $0.312987

### F-2a-7解消印（2026-08-02）

`stats_luna_002`のC1/C2/C4 pass・C3 `claims_absent`をfailedへ過小投影した
machine gapは、契約§4の字義に従いF-2a-7でpartialへ是正した。上のP0-b failは
006計測時点の歴史値として不変であり、本追記は原因裁定と解消revisionを示す。

## 12. 一次資料SHA-256

- `stats_luna_002/cli-case-binding.json`:
  `4a959f9932a772874f4de6f95b548abfc7f64aca137f565fec13e8b6396be95f`
- `stats_luna_002/cli-probe.json`:
  `996e119858205330761a3366a808f0da1a1345e9d7e9a9b018a60dd7a0bee087`
- `stats_luna_002/help-binding.json`:
  `37827602edaa39421e5c982c4633d78b763f4671c20dc4443a25302967521dd5`
- `stats_luna_002/cli-assurance.json`:
  `9d761b6e51c4b8c66b081b32ededc5883f3658d78e751c52ea174e9087b3f9f6`
- `stats_luna_003/cli-case-binding.json`:
  `4aa7f2f6f0a229d8f0452424d7654a6d7f404e4f98d03dcfcfd2ce5e8e0055a3`
- `stats_luna_003/cli-probe.json`:
  `abe7f6bc64dc9eedea8dc41ad96be5b45c952b8a2a5a22adcf1f18693a320a5b`
- `stats_luna_003/help-binding.json`:
  `4d97ff45191cfb1b84f4aec2ec598832fb3ccb17aa27dce40bb4667e9a10f474`
- `stats_luna_003/cli-assurance.json`:
  `0bc271c281a2f3e8e266a7997320cf33c43cf515007169c9cb4cbfe15dc9d714`
- `filter_luna_001/cli-case-binding.json`:
  `5d4daa09d46dffcd7b2264574c79a029195688cb5224b499ed410a0ff4f8f675`
- `filter_luna_001/cli-probe.json`:
  `70bb231acc81f0f7a09979caff17f092927bae2d6404578c80a25a1014acead4`
- `filter_luna_001/help-binding.json`:
  `501a68685827bb09cad0f52b6e87d3962d6e182c607e801a726c6ac472e105ac`
- `filter_luna_001/cli-assurance.json`:
  `3d62b2d40ab7e9b2a2669c88068c715824c6d0831e302a338cccb49342761012`
- `filter_luna_002/cli-case-binding.json`:
  `703e5a6664026d48fbbbfe0989d45de402e35cc095960134623ccf8f26c7be56`
- `filter_luna_002/cli-probe.json`:
  `fd5e353cc8902fd7bc5473814d61ab825f2c7b35dd197adf8959339fe351635f`
- `filter_luna_002/help-binding.json`:
  `64f6f63c5250950cbcca0b540d77f57521ea6b13300ae61833ac294490a3d7a6`
- `filter_luna_002/cli-assurance.json`:
  `ddca1eed53d154673ba055285bcaf69b866ae40837bf3cdee7d2bc6021dc2570`
- `filter_luna_003/cli-case-binding.json`:
  `c55c33a85af00eb5fd8bf6544b9e4c2d8f6d61ff9bd2d27154e9befa8ccb8f43`
- `filter_luna_003/cli-probe.json`:
  `6a22b2a1af20bc6ac6a59dc865c036ed85fcac153e58715ee850a43130dadb2d`
- `filter_luna_003/help-binding.json`:
  `d1b74853ab51702291e4452967da7b1aebb15cc4dd8e782464b516fcd3b2cf5a`
- `filter_luna_003/cli-assurance.json`:
  `35c4bd9a483a18f53c290143eee5c2dc3f6ce254366e6208d755cebd53300602`

## 13. Repository verification

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `cargo test --all-targets -- --format terse`: **1,985 passed / 32 ignored /
  0 failed**
- Python `unittest discover`: **92 passed / 0 failed**
- Ruff 0.16.0: green
- `test_band_aggregate.py`: **29 passed / 0 failed**
- Responses固有の反射key負例、provider protection audit、既存
  chat-completions/Ollama byte fixtureを含めてgreen。
