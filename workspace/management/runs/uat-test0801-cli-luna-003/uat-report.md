# uat-test0801-cli-luna-003: cli×create×gpt-5.6-luna text再計測

実施日: 2026-08-01 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

計測revision: `947a60223528e5387122dfa4e5d6bc279b73f04c` (`develop`)

## 1. 結論

**P0-a/b/cと資格情報scrubはpassしたが、fullは0/6、C3は未観測である。**
6/6は正直にfailed終端し、全件が`static (cli_probe_not_run)`、偽成功0。
明示した`tool_protocol=text`により、過去2窓のendpoint 400は6/6で解消し、
OpenAI成功応答24 turnとresponse metadataを初めて記録した。

一方、6/6は既存text/XML tool protocolの出力規律違反でfinal acceptance前に
停止した。停止内訳はmalformed XML 3、JSON trailing characters 2、feedback
後のtool call欠落1。従ってLunaのREADME証言行動は今回も判定不能である。
001/002のmachine BLOCKEDとは区別し、003はendpoint通過後に観測された
`process_failure` / modelの実測窓として扱う。

## 2. Campaign境界

- campaign: `cli-create-luna-20260801-093100`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0801_cli_luna3`
- suite: `cli-create-luna`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`, `tool_protocol=text`
- suite SHA-256:
  `4e32a2ba5a3ac6666046f432638203762556844bea5a74f665275b7f7b094017`
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gpt-5.6-luna` / `openai`
- run matrix: stats×3、filter×3
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

dry-runは6/6 readyで、全コマンドに`--tool-protocol text`が実在した。正式
preflightはgit clean、minimum ancestor `527bdc1e`、`cargo test`、release
buildの全てがgreen。built/installed binary SHA-256はともに
`45027bb2ba51b2b1025adf6048f6d1f5feea7d0d94b2864c13061625e682b7de`、
installed versionは
`commandagent 0.1.0 947a6022 2026-08-01T09:25:12Z`だった。

## 3. Run行列

`—`はfinal acceptance未到達で、Cの失敗を意味しない。

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / 帰属 | 停止原文の核 | 秒 | usage (in/out) | 費用 |
|---|---|---|---|---|---|---|---|---|---|---:|---:|---:|
| `stats_luna_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | `malformed XML tool call` | 372 | 6568 / 969 | $0.012382 |
| `stats_luna_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | `malformed XML tool call` | 362 | 3606 / 518 | $0.006714 |
| `stats_luna_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | `trailing characters at line 1 column 121` | 341 | 1324 / 128 | $0.002092 |
| `filter_luna_001` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | `missing tool call for action prompt after feedback` | 334 | 5501 / 377 | $0.007763 |
| `filter_luna_002` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | `malformed XML tool call` | 353 | 3570 / 641 | $0.007416 |
| `filter_luna_003` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / model | `trailing characters at line 1 column 230` | 361 | 1408 / 114 | $0.002092 |

全runのharness statusは`completed`、product exitは1、
`final_acceptance_status=not_checked`。自動分類はknown 6 / UNKNOWN 0で、
登録済み`process_failure`の形状既定に従うmodel帰属である。個別の解剖裁定を
行っていないため、既存class noteどおり帰属は暫定である。

## 4. Gemma基準線・Luna 3窓の対照

| arm | denominator | full | C到達 | C1 | C2 | C3判定 | C4 | 主停止 | 合計秒 | 費用 |
|---|---:|---:|---:|---|---|---|---|---|---:|---:|
| Gemma正式Window B (`elev-004`) | 6 | 0/6 | 2/6 | pass 2/2 | pass 2/2 | README捏造6/6をviolation拒否 | pass 2/2 | model 5、machine 1 | 3,739 | 記録なし |
| Luna 001 | 6 | 0/6 | 0/6 | — | — | 未判定 | — | endpoint 400 / machine BLOCKED | 2,571 | $0.000000 |
| Luna 002 | 6 | 0/6 | 0/6 | — | — | 未判定 | — | endpoint 400 / machine BLOCKED | 2,488 | $0.000000 |
| Luna 003 (`text`) | 6 | 0/6 | 0/6 | — | — | **未判定** | — | text protocol shape / model | 2,123 | $0.038459 |

Luna合算窓n=18はfull 0/18、C到達0/18。ただし001/002の12件はmachine
BLOCKEDで能力分母へ混ぜず、003の6件だけがtext protocolの実測窓である。
003はendpointと既存tool実行機構を実際に通過したが、README作成前に停止
したため、「軽量フロンティア系譜は正直なドキュメントを書くか」への答えは
まだ得られていない。

## 5. C3実物監査

到達runは0件であり、README出力例×実出力の対照原文は存在しない。
READMEは6 workspace全てで不在、C evidenceも不在だった。この不在を
`claims_absent`やC3 pass/failへ読み替えていない。

最も進んだ`stats_luna_001`では、既存text protocolを通じて
`data/sample.csv`と`cli/main.py`のWriteが実行された後、次のtool call形で
停止した。すなわち「toolsを送らないため何も作れない」のではない。

```text
→ Write data/sample.csv
✓ Write ok
→ Write cli/main.py
✓ Write ok
✗ Phase 1/4: setup-sample-data malformed XML tool call
```

最終workspaceの実在分布は次のとおり。

| run | `cli/main.py` | `README.md` | sample input |
|---|---|---|---|
| `stats_luna_001` | yes | no | `data/sample.csv` yes |
| `stats_luna_002` | no | no | `data/sample.csv` yes |
| `stats_luna_003` | no | no | no |
| `filter_luna_001` | no | no | `data/sample.txt` yes |
| `filter_luna_002` | no | no | `data/sample.txt` yes |
| `filter_luna_003` | no | no | no |

## 6. tool_protocol実物監査

OpenAI requestは24/24で`tools=0`、turn eventは24/24で
`native_tools_enabled=false`。endpoint rejectionは0/24である。代表原文:

```json
{"api":"chat_completions","event":"provider_request","model":"gpt-5.6-luna","provider":"openai","schema_version":"1","tools":0}
```

```json
{"api":"chat_completions","attempt":1,"event":"provider_response","model":"gpt-5.6-luna","provider":"openai","response_model":"gpt-5.6-luna","schema_version":"1","system_fingerprint":null,"tool_calls":0}
```

```json
{"caller_scope":"executor","event":"provider_turn_duration","finish_reason":"stop","model":"gpt-5.6-luna","native_tools_enabled":false,"ok":true,"provider":"openai","provider_model_id":"gpt-5.6-luna","provider_service_tier":"default","system_fingerprint":null,"timed_out":false,"tools":0}
```

宣言なし経路はprovider capability negotiationの既存挙動を保つ。特に現行
Ollamaはnative toolsを持つ構成であり、そこをtextへ変えるとbyte互換に反する
ため変更していない。明示`text`だけが既存XML instruction/parser/repairを選ぶ。

## 7. ドリフト探針

| run | turns | response model ID | system_fingerprint | service tier |
|---|---:|---|---|---|
| `stats_luna_001` | 6 | `gpt-5.6-luna` 6/6 | `null` 6/6 | `default` 6/6 |
| `stats_luna_002` | 4 | `gpt-5.6-luna` 4/4 | `null` 4/4 | `default` 4/4 |
| `stats_luna_003` | 2 | `gpt-5.6-luna` 2/2 | `null` 2/2 | `default` 2/2 |
| `filter_luna_001` | 6 | `gpt-5.6-luna` 6/6 | `null` 6/6 | `default` 6/6 |
| `filter_luna_002` | 4 | `gpt-5.6-luna` 4/4 | `null` 4/4 | `default` 4/4 |
| `filter_luna_003` | 2 | `gpt-5.6-luna` 2/2 | `null` 2/2 | `default` 2/2 |

requested/returned modelとservice tierは24/24一致。fingerprintは24/24で
provider未提供の`null`であり、版同一性を積極的には証明しない。

## 8. コスト

provider turn eventのreturned usageを合計した。2026-08-01確認の
[公式Luna単価](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
（standard uncached input $1.00 / 1M、output $6.00 / 1M）を適用した計算値。
実請求明細そのものではなく、cached-token内訳も記録されていないため、
保守的に全inputをuncachedとしている。

- input: 21,977 tokens = $0.021977
- output: 2,747 tokens = $0.016482
- campaign計: **$0.038459**
- preflight開始: epoch `1785576660`
- run開始: epoch `1785576749`
- run終了: epoch `1785578872`
- run合計: 2,123秒
- preflight開始→run終了: 2,212秒

## 9. E-0検収とscrub

- 自動分類: known 6 / UNKNOWN 0
- 自動分類: `process_failure` / model 6件
- 自動検収シート: 6/6
- calibration collector: appended 0
  （C2/C3へ未到達でnearest_miss evidenceがないため）
- run別scrub: 6/6 green、findings 0
- campaign scrub再実行: green、findings 0
- `OPENAI_API_KEY`実値のexact scan: 105 files、matches 0
- `.env`は読取り元に使っただけで変更・commitしていない

## 10. 合否

- P0-a 6/6正直終端: **pass**
- P0-b 契約§4投影: **pass**
  （C1未実行→`static (cli_probe_not_run)`が6/6）
- P0-c 偽成功ゼロ: **pass**
- 資格情報scrub: **pass**
- 記録値 full: 0/6
- 記録値 C到達: 0/6
- 記録値 C3: 分母0、判定不能
- 記録値 OpenAI費用: $0.038459

## 11. Repository verification

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `cargo test --all-targets --no-fail-fast`: 1968 passed / 31 ignored / 0 failed
- Python unittest: 89 passed / 0 failed
- Ruff 0.16.0（変更Python 2ファイル）: green
- 既存経路byte互換: 既存fixture・snapshot無変更でgreen

## 12. 一次資料SHA-256

- `uat-meta.json`:
  `e8a5a80bd28f832488030f7a7c0f72b86c437eb512895c27d89a9175ad709edc`
- `report-skeleton.md`:
  `b01ceaf9461f8cda6d3b5274fb62d6f76922041ef7deb90d494c6f76026fd791`
- `stats_luna_001/events.jsonl`:
  `179d6594dc4071f6bafda821f1224ad8c003b8743d1fe6ccfcb751fbd2b176a4`
- `stats_luna_002/events.jsonl`:
  `ac5b915883055dec66707641b5b5ba4fade4e916e983184f05778be7ac8fcfbd`
- `stats_luna_003/events.jsonl`:
  `314f3f2a8ca17bd7cceb83538a7799574ef59a919598adebbf99bb3895b3be62`
- `filter_luna_001/events.jsonl`:
  `5802db4678a2c44fb747b0f4b7da238c794377bffbd6330dd227803abc380394`
- `filter_luna_002/events.jsonl`:
  `149bec4c456fbdaab241eb57ad4250683b09d1adfefc6e55f144d025696a313a`
- `filter_luna_003/events.jsonl`:
  `07f1dd0b342d6066600eff249daef4a61b9e77d349dfeb2b0d157c141e61cafa`
