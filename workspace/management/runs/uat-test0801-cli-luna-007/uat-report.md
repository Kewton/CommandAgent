# uat-test0801-cli-luna-007: CLI投影是正後のLuna再計測

実施日: 2026-08-02 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

計測revision: `c2df3e098573292ae0096f22488f51887db60e2b` (`develop`)

## 1. 結論

**CLI史上初のfullが`filter_luna_001`で成立した。Luna-007はfull 1/6、
C到達2/6、到達runのC3はpass 2/2（5 claims matched、violation 0）だった。**

投影是正後、C1〜C4全passはfull、C1違反はfailed、未到達はstaticとなり、
契約§4との不整合は0/6。Luna-006で初めて単独観測したモデル因子
（C3 pass 2 / violation 1）は、007でC3 pass 2/2とfull 1件へ進んだ。
Gemma正式Window Bのfull 0/6・C3捏造拒否6件に対し、証言壁がモデル階級と
Responses正門で動くことを再現し、初のend-to-end fullまで観測した。

## 2. Campaign境界

- campaign: `cli-create-luna-20260802-045807`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0801_cli_luna7`
- suite: `cli-create-luna`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`, `api=responses`, `tool_protocol=native`
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gpt-5.6-luna` / `openai`
- run matrix: stats×3、filter×3
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

既知の未追跡`history.txt` 2件と、既存の未追跡`.agents/` / `.claude/`は移動・
削除・stashせず、同一HEADのdetached clean worktreeからbenchを実行した。
最初のgeneric `python3`起動はmacOS Python 3.9の`tomllib`不足でcampaign生成前に
終了したため、確立済みPython 3.12で同一コマンドを起動した。run消費・retryは0。
preflightはgit clean、minimum ancestor `527bdc1e`、`cargo test`、release buildが
green。built/installed binary SHA-256はともに
`41a49fb66b58c63926077ecd657066bd449e9096ef2228c05e45775aea79301d`、
versionは`commandagent 0.1.0 c2df3e09 2026-08-02T05:00:31Z`だった。

## 3. Run行列

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止主因 | 秒 | reasoning | 費用 |
|---|---|---|---|---|---|---|---|---|---:|---:|---:|
| `stats_luna_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | C1 case extraction (`README.md:8`) | 1,433 | 777 | $0.032310 |
| `stats_luna_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | C1 case extraction (`README.md:8`) | 1,260 | 1,339 | $0.048304 |
| `stats_luna_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | C1 case extraction (`README.md:8`) | 1,598 | 1,396 | $0.059063 |
| `filter_luna_001` | filter | **complete** | **full** | pass | pass | **pass** | pass | completed | 1,526 | 1,780 | $0.069364 |
| `filter_luna_002` | filter | failed | failed (`cli_assurance_failed`) | fail | pass | **pass** | pass | C1 bare placeholder polarity | 1,186 | 1,191 | $0.052345 |
| `filter_luna_003` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | verify command false negative | 1,678 | 1,020 | $0.048668 |

全runのharness statusは`completed`。product exitは0が1件、1が5件。C3分布は
pass 2 / violation 0 / claims_absent 0 / not reached 4。投影分布はfull 1 / failed 1 /
static 4で、対象形だけが是正されたことをliveでも確認した。

## 4. C3実物監査

### `filter_luna_001` — full成立

README claim 1 (`README.md:27->33`)と実出力:

```text
python3 cli/main.py data/sample.txt --pattern ERROR
2026-08-01 09:24 ERROR Failed to connect to the database
2026-08-01 10:15 ERROR Unable to send notification email
```

README claim 2 (`README.md:42->48`)と実出力:

```text
python3 cli/main.py data/sample.txt --pattern WARNING --count
3
```

README claim 3 (`README.md:54->60`)と実出力:

```text
python3 cli/main.py --pattern INFO --count
6
```

3/3 claimsは字義一致。C1の凍結正常case
`python3 cli/main.py --pattern PATTERN`はdefault sampleを使ってexit 0、
invalid `--anvil-invalid-probe`はexit 2、rerunはexit/stdout/stderr一致だった。
C2はhelp→implementationの5 optionsとimplementation→helpの未知option拒否を全pass。

### `filter_luna_002` — C3は正直、C1だけ不成立

README claim 1 (`README.md:26->32`)と実出力:

```text
python cli/main.py data/sample.txt --pattern error
error: connection failed
error: request failed with status 500
```

README claim 2 (`README.md:39->45`)と実出力:

```text
python cli/main.py data/sample.txt --pattern warning --count
2
```

2/2 claimsは一致。一方、先頭usageの裸metavarを凍結したC1正常caseは
`python cli/main.py FILE --pattern PATTERN`で、存在しない`FILE`によりexit 1。
invalidはexit 2、rerunは同一、C2は両方向passであるため、C1違反だけを理由に
failedへ投影した。これはLuna-006解剖で確定した「近因model・設計根因machine
（角括弧正準形の字義配布欠落）」の再発形であり、偽成功にはしていない。

## 5. C未到達4件

- stats 3件はすべて`profile_behavior_probe_error:
  case_extraction_failed: README.md:8`で正直終端した。001/003の先頭usageは
  `<CSVファイル>` / `<列名>`という日本語placeholderで、現resolverがsampleへ
  束縛できない。002の最終artifactは具体値へ修復済みだが、初回凍結失敗を
  差し替えず終端した。
- `filter_luna_003`は機械正準verifyが、生成CLIの必須`--file`を欠く
  `python cli/main.py --pattern 'error' data/sample.txt`を実行し、既知class
  `cli_verify:canonical_command_dropped_positional_input`でacceptance前に停止した。

自動分類はknown 5 / UNKNOWN 1。UNKNOWNは唯一の成功run
`filter_luna_001`であり、失敗classを持たない成功をfailure classifierへ通した結果である。
失敗5件はprocess_failure/model 4、既知machine class 1と表示された。上記C1再発は
一次資料に基づき設計根因machineと分離記録する。

## 6. Gemma基準線・Luna窓推移

| arm | denominator | full | C到達 | C3判定 | 合計秒 | 費用 |
|---|---:|---:|---:|---|---:|---:|
| Gemma正式Window B (`elev-004`) | 6 | 0/6 | 2/6 | README捏造6件をviolation拒否 | 3,739 | 記録なし |
| Luna 001 | 6 | 0/6 | 0/6 | 未判定・machine BLOCKED | 2,571 | $0.000000 |
| Luna 002 | 6 | 0/6 | 0/6 | 未判定・machine BLOCKED | 2,488 | $0.000000 |
| Luna 003 (`text`) | 6 | 0/6 | 0/6 | 未判定 | 2,123 | $0.038459 |
| Luna 004 (`text`+自己記録) | 6 | 0/6 | 0/6 | 未判定 | 2,598 | $0.118284 |
| Luna 005 (`text`+dialect repair) | 6 | 0/6 | 0/6 | 未判定 | 3,069 | $0.202160 |
| Luna 006 (`responses`+native) | 6 | 0/6 | 5/6 | pass 2 / fail 1 / absent 2 | 8,655 | $0.312987 |
| **Luna 007（投影是正後）** | **6** | **1/6** | **2/6** | **pass 2 / fail 0 / absent 0** | **8,681** | **$0.310053** |

Luna合算n=42はfull 1/42、C到達7/42、費用$0.981943。001/002はmachine
BLOCKED、003〜005はtext bridge、006/007はResponses native窓として区分する。
Responses 2窓のC3合計はpass 4 / violation 1 / claims_absent 2である。

## 7. Responses/nativeとドリフト探針

| run | provider turns | native tool calls | input | cached input | output | reasoning |
|---|---:|---:|---:|---:|---:|---:|
| `stats_luna_001` | 16 | 16 | 80,903 | 70,946 | 2,543 | 777 |
| `stats_luna_002` | 18 | 18 | 106,081 | 93,410 | 4,382 | 1,339 |
| `stats_luna_003` | 20 | 21 | 144,861 | 130,658 | 5,299 | 1,396 |
| `filter_luna_001` | 19 | 18 | 141,583 | 124,063 | 6,573 | 1,780 |
| `filter_luna_002` | 23 | 26 | 162,123 | 147,489 | 3,827 | 1,191 |
| `filter_luna_003` | 16 | 18 | 93,155 | 80,457 | 4,654 | 1,020 |
| 合計 | **112** | **117** | **728,706** | **647,023** | **27,278** | **7,503** |

112/112 turnsでrequested/returned modelは`gpt-5.6-luna`、service tierは
`default`、`api=responses`、native tools enabled。system fingerprintはprovider
未提供の`null` 112/112で、版同一性を積極的には証明しない。response IDは112/112で
相異なる。111 responsesが1件以上のnative function callを返し、合計117 calls。
endpoint rejectionとtext parse failureは0だった。

## 8. コスト

provider turn eventのreturned usageを合計し、2026-08-02確認済みの公式Luna単価
（uncached input $1.00/M、cached input $0.10/M、output $6.00/M）を適用した。

- uncached input: 81,683 tokens = $0.081683
- cached input: 647,023 tokens = $0.064702
- output: 27,278 tokens = $0.163668
- campaign計: **$0.310053**
- reasoning: 7,503 tokens（output内数）
- preflight開始: epoch `1785646687`
- run開始: epoch `1785646854`
- run終了: epoch `1785655536`
- run合計: 8,681秒
- preflight開始→run終了: 8,849秒

## 9. E-0検収とscrub

- 自動分類: known 5 / UNKNOWN 1（UNKNOWNはfull成功runで停止classなし）
- 自動検収シート: 6/6
- calibration collector: appended 0（C2/C3 nearest_missなし）
- campaign scrub再実行: green、findings 0
- report scrub: green、findings 0
- `OPENAI_API_KEY`実値のexact scan: campaign 232 files、
  campaign+report合計234 files、matches 0
- `.env`は読取り元に使っただけで変更・commitしていない

## 10. 合否

- P0-a 6/6正直終端: **pass**
- P0-b 契約§4投影: **pass**
- P0-c 偽成功ゼロ: **pass**
- 資格情報scrub: **pass**
- Responses/native transport: **pass**（112/112 turns、117 tool calls）
- 記録値 full: **1/6**
- 記録値 C到達: **2/6**
- 記録値 C3: **pass 2 / fail 0 / claims_absent 0**
- 記録値 OpenAI費用: **$0.310053**

## 11. 一次資料SHA-256

- `filter_luna_001/cli-case-binding.json`:
  `0098fd7c5b072214f127b31f5768b0b9d0ab8cec9363a0bfc26c6430d6b0a053`
- `filter_luna_001/cli-probe.json`:
  `c4e3ff9ed2158214708525e07ef9296982d9cc489cbf4911800d6489b774040b`
- `filter_luna_001/help-binding.json`:
  `de39d50dc0f218ee515c6805b5f43eb186382f10ffe8e99b784c56a3d2253a44`
- `filter_luna_001/cli-assurance.json`:
  `5d583cc6f973b4d7138a5ca148af56e098743eff54fac06ec2f59c5ad513be4e`
- `filter_luna_002/cli-case-binding.json`:
  `faa7a36c090228de5bedb92accf16abf66e955107ada893902d2c790064a4ce2`
- `filter_luna_002/cli-probe.json`:
  `873d40d777126e1d82105089169d341b0357a620328ebc8dee8c70a65e6617d2`
- `filter_luna_002/help-binding.json`:
  `c58bfbf31712dbdd10e08ef8f4b14f33f1f14bea6dbd26c4a219e3d0dba1ac2d`
- `filter_luna_002/cli-assurance.json`:
  `cef53eae0ae8feaef6ddc879bf2783309ced63423cac08694e6b70e964468324`

## 12. Repository verification

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `cargo test --all-targets -- --format terse`: **1,986 passed / 32 ignored /
  0 failed**
- Python `unittest discover`: **92 passed / 0 failed**
- Ruff 0.16.0: green
- CLI投影fixture、corpus regression、classes双方向guard、既存非CLI byte fixtureを
  含めてgreen。
