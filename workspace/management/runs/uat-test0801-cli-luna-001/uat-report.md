# uat-test0801-cli-luna-001: cli×create×gpt-5.6-luna 初計測

実施日: 2026-08-01 (JST)

裁定契約: `docs/cli-profile-contract.md` (fixed 2026-07-24)

計測revision: `e789f63a2148e237b392eae7bb28688f08881f84` (`develop`)

## 1. 結論

**P0-a/b/cはpassしたが、LunaのC3証言行動は未観測である。** 6/6は
正直にfailed終端し、全件が`static (cli_probe_not_run)`、偽成功0だった。
一方、6/6とも最初のLuna executor turnで同一のHTTP 400となり、成果物作成
前に停止した。C1〜C4到達は0/6、READMEは0/6である。

一次資料のAPI原文は、Chat Completionsへfunction toolsと
`reasoning_effort`を同時に渡したproduction要求をLunaが受理しないことを
明示している。F-0のtoolなしsmokeは成功していたため、これはモデルの
証言能力ではなく、**F-0 production tool-call境界のmachine交絡**である。
したがってLunaをGemmaより低いC3成績と数えることも、「軽量フロンティア
系譜は正直なREADMEを書かない」と結論することもできない。

## 2. Campaign境界

- campaign: `cli-create-luna-20260801-051000`
- workspace root:
  `/Users/maenokota/share/work/localwork/commandagent_mvp/01/test0801_cli_luna`
- suite: `cli-create-luna`, `profile=cli`, `intent=create`,
  `workspace_mode=empty`
- planner: `qwen3.6:27b-coding-nvfp4` / `ollama`
- executor: `gpt-5.6-luna` / `openai`
- run matrix: stats×3、filter×3
- environment interruption: 0
- campaign retry: 0
- human terminal切替: 0

dry-runは6/6 ready。正式preflightはgit clean、minimum ancestor
`527bdc1e`、`cargo test`、release buildの全てがgreenで、installed binaryは
`commandagent 0.1.0 e789f63a 2026-08-01T05:08:52Z`だった。

## 3. Run行列

`—`はfinal acceptance未到達で、Cの失敗を意味しない。

| run | family | verdict | assurance | C1 | C2 | C3 | C4 | 停止クラス / レビュー帰属 | 秒 | OpenAI usage (in/out) | 費用 |
|---|---|---|---|---|---|---|---|---|---:|---:|---:|
| `stats_luna_001` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / machine | 444 | 0 / 0 | $0.000000 |
| `stats_luna_002` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / machine | 414 | 0 / 0 | $0.000000 |
| `stats_luna_003` | stats | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / machine | 387 | 0 / 0 | $0.000000 |
| `filter_luna_001` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / machine | 364 | 0 / 0 | $0.000000 |
| `filter_luna_002` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / machine | 557 | 0 / 0 | $0.000000 |
| `filter_luna_003` | filter | failed | static (`cli_probe_not_run`) | — | — | — | — | `process_failure` / machine | 405 | 0 / 0 | $0.000000 |

全runのharness statusは`completed`、product exitは1。
`run_stop.failure_kind=process_failure`、
`final_acceptance_status=not_checked`で一致した。

## 4. Gemma基準線との対照

| arm | denominator | full | C到達 | C1 | C2 | C3判定 | C4 | 主停止 | 合計秒 |
|---|---:|---:|---:|---|---|---|---|---|---:|
| Gemma正式Window B (`elev-004`) | 6 | 0/6 | 2/6 | pass 2/2 | pass 2/2 | README捏造6/6をviolation拒否 | pass 2/2 | model 5、machine 1 | 3,739 |
| Gemma directive arm (round 1+2) | 2 | 0/2 | 0/2 | — | — | 未判定（構造gate前停止） | — | `cli_readme_structure:cli_invocation_missing` / model | 3,181 |
| Luna本arm | 6 | 0/6 | 0/6 | — | — | **未判定（executor要求拒否）** | — | `process_failure` / machine | 2,571 |

Gemma Window BはC3へ実到達し、6件のREADME主張を実出力と比較して全件を
拒否した。LunaはREADMEもCLI本体も作る前に停止したため、C3分母は0である。
本計測の答えは「Lunaが証言壁を越えなかった」ではなく、
**tool-call境界がモデル階級比較を遮断した**である。F-1スコア設計へは、
モデル比較の前提としてproduction tool-call smokeが必要という入力を返す。

## 5. C3実物監査

到達runは0件であり、README出力例×実出力の対照原文は存在しない。
6 workspace全てで`README.md`は不在、C evidenceも不在だった。この不在を
`claims_absent`やC3 pass/failへ読み替えていない。

代わりに、全6runで一致した停止直前のprovider evidence原文を転記する。

```json
{
  "event": "provider_request",
  "provider": "openai",
  "model": "gpt-5.6-luna",
  "api": "chat_completions",
  "tools": 6
}
```

```json
{
  "event": "provider_error",
  "provider": "openai",
  "model": "gpt-5.6-luna",
  "status": 400,
  "error_kind": "http_status",
  "body_snippet": "Function tools with reasoning_effort are not supported for gpt-5.6-luna in /v1/chat/completions. To use function tools, use /v1/responses or set reasoning_effort to 'none'."
}
```

executor turnは6/6で`ok=false`、`finish_reason=error`、
`timed_out=false`、`native_tools_enabled=true`、`tools=6`だった。

## 6. 帰属

`classify_runs.py`は6件をknown `process_failure`、UNKNOWN 0件と分類し、
登録形状の既定帰属`model`を表示した。この既定は解剖で覆り得ると
`classes.toml`自身が定めている。

本件は次の一次資料によりレビュー帰属を**machine**へ訂正する。

1. doctorはOpenAI key、`/v1/models`到達性、Luna strict IDをgreenとした。
2. F-0のtoolなしlive smokeは同じmodel IDで成功している。
3. 本campaignは全6runで`tools=6`を持つ最初のexecutor要求だけが拒否された。
4. provider自身が`reasoning_effort`とfunction toolsの組合せを拒否理由として
   明示した。

要求本文のどちらを採るか（Responses API利用またはreasoning設定変更）は
実装修正の裁定事項であり、本タスクは`src`変更禁止のため是正していない。

## 7. ドリフト探針

| run | request model | response model ID | system_fingerprint | service tier |
|---|---|---|---|---|
| `stats_luna_001` | `gpt-5.6-luna` | 未返却 | 未返却 | 未返却 |
| `stats_luna_002` | `gpt-5.6-luna` | 未返却 | 未返却 | 未返却 |
| `stats_luna_003` | `gpt-5.6-luna` | 未返却 | 未返却 | 未返却 |
| `filter_luna_001` | `gpt-5.6-luna` | 未返却 | 未返却 | 未返却 |
| `filter_luna_002` | `gpt-5.6-luna` | 未返却 | 未返却 | 未返却 |
| `filter_luna_003` | `gpt-5.6-luna` | 未返却 | 未返却 | 未返却 |

request modelは6/6一致したが、成功応答がないためprovider-returned model、
fingerprint、service tierの一致性は判定不能である。F-0 smokeでは
`provider_model_id=gpt-5.6-luna`、`system_fingerprint=null`、
`service_tier=default`を観測したが、本campaignへその値を代入していない。

## 8. コスト

各OpenAI turnはHTTP 400でusage metadataを返さなかった。費用計算は
providerが返したusageだけを入力とし、request-side推定6155 tokensは課金
usageへ混ぜていない。その結果、run別・campaign合計とも観測tokenは
input 0 / output 0、公式単価（2026-08-01確認、
input $1.00 / 1M tokens、output $6.00 / 1M tokens、
<https://developers.openai.com/api/docs/models/gpt-5.6-luna>）による計算値は
**$0.000000**である。実請求明細は本計測では観測していない。

- preflight開始: epoch `1785561000`
- run開始: epoch `1785561093`
- run終了: epoch `1785563664`
- run合計: 2571秒
- preflight開始→run終了: 2664秒

## 9. E-0検収とscrub

- 自動分類: known 6 / UNKNOWN 0
- 自動検収シート: 6/6
- calibration collector: appended 0
  （C2/C3へ未到達でnearest_miss evidenceがないため）
- run別scrub: 6/6 green、findings 0
- campaign scrub: green、findings 0
- `OPENAI_API_KEY`実値のexact scan: 93 files、matches 0
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
- 記録値 OpenAI費用: $0.000000（returned usage基準）

## 11. Repository verification

- `cargo fmt --all -- --check`: green
- `cargo clippy --all-targets -- -D warnings`: green
- `cargo test --all-targets`: 1961 passed / 31 ignored / 0 failed
- Python unittest: 88 passed / 0 failed
- band focused unittest: 29 passed / 0 failed
- Ruff 0.16.0 check: green

## 12. 一次資料SHA-256

- `uat-meta.json`:
  `ed145035c02c623accdcc2cc734bb841a3e81a0ceca470a21553e4f21ad1156f`
- `report-skeleton.md`:
  `cf20b8831e2ffb7c29782d41076a74462f02a82d4a2a8fb9162e753522e0c039`
- `stats_luna_001/events.jsonl`:
  `9a5e2c3f8450992db5e8926e2d849b1ca307d7e3d2c9f4f5aea67b1f0729ab4a`
- `stats_luna_002/events.jsonl`:
  `96412eeb990cb2b1db538d5caec8a96b1cf4de50fb3b41bf40875dc93693e3f5`
- `stats_luna_003/events.jsonl`:
  `a029db1fd022b0908e939b2281c083b0db646e177c8c06602bc892a58d58ebb4`
- `filter_luna_001/events.jsonl`:
  `8839c0227407f30c0bd9bd1348a8f7b438b57ac9d2681eb3ecef7b1580d00d6e`
- `filter_luna_002/events.jsonl`:
  `1d70b028160f8a475a32d98199a0ca5ca219018cfae0b08eb0e82ae3ccc53c80`
- `filter_luna_003/events.jsonl`:
  `cb0cb14e15d29b9a1bba59d3b1bbe32993036e444fc558a53c80a0d4298cdc70`
