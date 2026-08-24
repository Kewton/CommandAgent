# uat-test0801-cli-luna-005: text修復層第1周後のLuna再計測

実施日: 2026-08-02 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

計測revision: `a245e926878e60ef304bb2b9e7d86b67e2727437` (`develop`)

## 1. 結論

**P0-a/b/cと資格情報scrubはpassした。新規repairは9回すべて自己申告され、
004で機械修復可能と裁定した2形をliveで救済したが、C系到達は0/6だった。**

発火内訳は`first_json_value` 1、`closing_tag_completed` 8。全9件に
`repair_applied` eventとenvelope準拠evidenceが対で実在し、黙った修復は0件。
しかし後続停止はb型`missing_call` 4件と`model_empty_response` 2件で、6/6が
`failed` / `static (cli_probe_not_run)`へ正直に終端した。C1〜C4は未到達、
C3のREADME出力例×実出力は今回も未判定である。

従って限定修復は狙った方言を直せたが、残存停止を支配したのはparser入力を
作らないモデル応答である。C未到達という事前条件に従い、F-0b
（Responses API/native tools正式経路）の昇格を次の裁定材料として明記する。

## 2. Campaign境界

- campaign: `cli-create-luna-20260801-153409`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0801_cli_luna5`
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

既知の未追跡`history.txt` 2件は移動・削除・stashせず、同一HEADのdetached clean
worktreeからbenchを実行した。preflightはgit clean、minimum ancestor
`527bdc1e`、`cargo test`、release buildの全てがgreen。built/installed binary
SHA-256はともに
`43861f3483b2cdfbfaf92d4b1ba38742f0a9cc324f693277af89612702be24ab`、
versionは`commandagent 0.1.0 a245e926 2026-08-01T15:36:46Z`だった。

## 3. Run行列

`—`はfinal acceptance未到達で、Cの失敗を意味しない。repair欄は
`first_json_value / closing_tag_completed`。

| run | family | verdict | assurance | repair | C1 | C2 | C3 | C4 | 停止 / 残存形 | 秒 | usage (in/out) | 費用 |
|---|---|---|---|---:|---|---|---|---|---|---:|---:|---:|
| `stats_luna_001` | stats | failed | static (`cli_probe_not_run`) | 0 / 1 | — | — | — | — | `missing_call` / b | 777 | 27,888 / 3,461 | $0.048654 |
| `stats_luna_002` | stats | failed | static (`cli_probe_not_run`) | 0 / 0 | — | — | — | — | `missing_call` / b | 433 | 18,028 / 1,726 | $0.028384 |
| `stats_luna_003` | stats | failed | static (`cli_probe_not_run`) | 0 / 2 | — | — | — | — | `model_empty_response` | 865 | 26,849 / 1,400 | $0.035249 |
| `filter_luna_001` | filter | failed | static (`cli_probe_not_run`) | 0 / 1 | — | — | — | — | `model_empty_response` | 316 | 19,520 / 1,770 | $0.030140 |
| `filter_luna_002` | filter | failed | static (`cli_probe_not_run`) | 1 / 3 | — | — | — | — | `missing_call` / b | 252 | 27,001 / 1,723 | $0.037339 |
| `filter_luna_003` | filter | failed | static (`cli_probe_not_run`) | 0 / 1 | — | — | — | — | `missing_call` / b | 426 | 15,080 / 1,219 | $0.022394 |

全runのharness statusは`completed`、product exitは1、
`final_acceptance_status=not_checked`。自動分類はknown 6 / UNKNOWN 0、
登録済み`process_failure` / modelが6件である。

## 4. repair_applied実物監査

### 集計

| repair kind | event | evidence | 変更内容 | 上限 |
|---|---:|---:|---|---:|
| `first_json_value` | 1 | 1 | 検証済み先頭JSON value後の余分な`}`を1 byte破棄 | 256 bytes |
| `closing_tag_completed` | 8 | 8 | 完結JSON bodyへ`</anvil_tool_call>`を18 bytes追記 | 256 bytes |
| 合計 | **9** | **9** | 全件`operation`と変更抜粋を記録 | 256 bytes |

`filter_luna_002`の第一値切り出しevidence原文:

````json
{"repair_kind":"first_json_value","change_excerpt":{"operation":"discarded","text":"}","max_bytes":256,"original_bytes":1,"truncated":false},"phase":"setup-sample-data"}
````

同runの閉じタグ補完evidence原文:

````json
{"repair_kind":"closing_tag_completed","change_excerpt":{"operation":"appended","text":"</anvil_tool_call>","max_bytes":256,"original_bytes":18,"truncated":false},"phase":"setup-sample-data"}
````

両形ともeventの`evidence_recorded=true`と`evidence_path`を確認した。先頭JSONは
既存のobject形・tool名・allowed tool・arguments検証を通過した後にだけ採用され、
閉じタグはbodyが完結した場合にだけ補完された。失敗形を無言で通した記録はない。

## 5. 残存b型の原文

修復後もtool callを作らなかった`missing_call`は4件。campaign scrub後の
`tool_parse_failure.raw_excerpt.text`を全件転記する。

### `stats_luna_001`

````text
Unable to proceed because the required workspace tools are unavailable in this session.
````

### `stats_luna_002`

````text
ефон
````

### `filter_luna_002`

````text
to=anvil_tool_call.name Read code:
{"path":"verify_cli.py"}
````

### `filter_luna_003`

````text
Step `verify-cli-functionality` completed successfully.

Verified with:

```bash
python cli/main.py data/sample.txt --pattern apple
```

The CLI executed without errors.
````

いずれも登録tool callとして検証できる入力ではない。残る2件はbounded recovery
4回後の空応答であり、parser修復の対象物自体がない。従って005の残存は
**b型4 + empty 2**で、追加のJSON/XML緩和では救えない。

## 6. C系・C3実物監査

final acceptance到達は0/6で、C1〜C4 evidenceは存在しない。したがって
README出力例×C1実出力のC3原文対照は分母0であり、pass/failや
`claims_absent`へ読み替えない。

一方、最終workspaceでは6/6に`cli/main.py`、`README.md`、対応する
`data/sample.csv`または`data/sample.txt`が実在した。修復規則がtool callを
実行可能にした効果は成果物分布に現れたが、acceptanceまで完走していない以上、
成果物の誠実性を推測で合格扱いしない。

## 7. Gemma基準線・Luna窓推移

| arm | denominator | full | C到達 | C3判定 | 主停止 | 合計秒 | 費用 |
|---|---:|---:|---:|---|---|---:|---:|
| Gemma正式Window B (`elev-004`) | 6 | 0/6 | 2/6 | README捏造6/6をviolation拒否 | model 5、machine 1 | 3,739 | 記録なし |
| Luna 001 | 6 | 0/6 | 0/6 | 未判定 | endpoint 400 / machine BLOCKED | 2,571 | $0.000000 |
| Luna 002 | 6 | 0/6 | 0/6 | 未判定 | endpoint 400 / machine BLOCKED | 2,488 | $0.000000 |
| Luna 003 (`text`) | 6 | 0/6 | 0/6 | 未判定 | text tool-call shape / model | 2,123 | $0.038459 |
| Luna 004 (`text`+自己記録) | 6 | 0/6 | 0/6 | 未判定 | a=4、b=2 / model | 2,598 | $0.118284 |
| Luna 005 (`text`+dialect repair) | 6 | 0/6 | 0/6 | **未判定** | repair 9後にb=4、empty=2 | 3,069 | $0.202160 |

Luna合算n=30はfull 0/30、C到達0/30、費用$0.358903。001/002の12件は
machine BLOCKED、003/004の12件は較正前text protocol窓、005の6件は方言修復後
の窓として区分する。004のa形4件に対して005では同じ2規則が計9回live発火し、
対象方言による終端は0件になった。それでもC未到達なので、
「軽量フロンティア系譜は正直なドキュメントを書くか」は未判定のままである。

## 8. ドリフト探針

OpenAI requestは81/81で`tools=0`、turn eventは81/81で
`native_tools_enabled=false`。endpoint rejectionは0/81。

| run | turns | response model ID | system_fingerprint | service tier |
|---|---:|---|---|---|
| `stats_luna_001` | 15 | `gpt-5.6-luna` 15/15 | `null` 15/15 | `default` 15/15 |
| `stats_luna_002` | 12 | `gpt-5.6-luna` 12/12 | `null` 12/12 | `default` 12/12 |
| `stats_luna_003` | 15 | `gpt-5.6-luna` 15/15 | `null` 15/15 | `default` 15/15 |
| `filter_luna_001` | 13 | `gpt-5.6-luna` 13/13 | `null` 13/13 | `default` 13/13 |
| `filter_luna_002` | 15 | `gpt-5.6-luna` 15/15 | `null` 15/15 | `default` 15/15 |
| `filter_luna_003` | 11 | `gpt-5.6-luna` 11/11 | `null` 11/11 | `default` 11/11 |

requested/returned modelとservice tierは81/81一致。fingerprintは81/81で
provider未提供の`null`であり、版同一性を積極的には証明しない。

## 9. コスト

provider turn eventのreturned usageを合計し、2026-08-01確認の
[公式Luna単価](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
（standard uncached input $1.00 / 1M、output $6.00 / 1M）を適用した。
cached-token内訳は記録されないため全inputをuncachedとして保守的に計算し、
実請求明細そのものではない。

- input: 134,366 tokens = $0.134366
- output: 11,299 tokens = $0.067794
- campaign計: **$0.202160**
- preflight開始: epoch `1785598449`
- run開始: epoch `1785598633`
- run終了: epoch `1785601702`
- run合計: 3,069秒
- preflight開始→run終了: 3,253秒

## 10. E-0検収とscrub

- 自動分類: known 6 / UNKNOWN 0
- 自動分類: `process_failure` / model 6件
- 自動検収シート: 6/6
- calibration collector: `tool_parse_repair` 9件 + `tool_parse` 4件 appended
- family追従guard: 27/27 green
- run別scrub: 6/6 green、findings 0
- campaign scrub再実行: green、findings 0
- `OPENAI_API_KEY`実値のexact scan: 178 files、matches 0
- `.env`は読取り元に使っただけで変更・commitしていない

## 11. 合否

- P0-a 6/6正直終端: **pass**
- P0-b 契約§4投影: **pass**
  （C1未実行→`static (cli_probe_not_run)`が6/6）
- P0-c 偽成功ゼロ: **pass**
- repair自己申告: **pass**（event 9/9、evidence 9/9、silent 0）
- 資格情報scrub: **pass**
- 記録値 full: 0/6
- 記録値 C到達: 0/6
- 記録値 C3: 分母0、判定不能
- 記録値 b型: 4/6
- 記録値 empty response: 2/6
- 記録値 OpenAI費用: $0.202160

## 12. Repository verification

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- 権限付き`cargo test --all-targets`: 1,981 passed / 31 ignored / 0 failed
- Python unittest: 91 passed / 0 failed
- Ruff 0.16.0: green
- corpus regression: green
- growth guard: green（baseline変更なし）
- ローカル経路byte互換: 既存fixture・snapshot無変更でgreen。
  新event/evidenceはrepair成立時だけの加法で、未修復経路のbytesは不変

## 13. 一次資料SHA-256

- `uat-meta.json`:
  `6f3f25d3c9625bcc33b1936896612a364e6c1b044bb7ffbca020cf98ad89e581`
- `report-skeleton.md`:
  `83edc49af727136127d0da0655889140e9ad0bd98aa94dc3971a2583a7992398`
- `stats_luna_001/events.jsonl`:
  `238cb074875c3a384d0f8e06191eca6dab542e1cf7f0787a18da9931ce23d252`
- `stats_luna_002/events.jsonl`:
  `afe6fcd82c9765d4db72878fe18924a02efc21fc7a3ee1549db04520ef3af609`
- `stats_luna_003/events.jsonl`:
  `a15a175c84b1e549f33327760b73f41bcfa280677a48308dea29e127226875a0`
- `filter_luna_001/events.jsonl`:
  `2ec1bcee6dad76a014cb114be073bf49d33ed1922d65bdf01589b6eff5d9a332`
- `filter_luna_002/events.jsonl`:
  `655bb4bbcdc7508de64a1524478df967c434c0ec98ccbff8094932edda4e9d14`
- `filter_luna_003/events.jsonl`:
  `9e81fa7fdfce7cfedfc2d0ca0e5c10895d1471bb75f85ecc9d45335b111fd6a6`
- repair evidence 9件・failure evidence 4件のhashは
  `evidence/campaign-summary.json`へ個別固定した。

## 14. 裁定材料

方言較正第1周は、004から導いた2形をliveで9回救済し、対象方言による停止を
消した。しかしC到達0/6で、残存はtool callを作らないb型4件と空応答2件が
支配した。追加のparser緩和は対象がないため、本系列はここで停止し、
**F-0b（Responses API/native tools）の昇格**をレビュー裁定材料とする。
