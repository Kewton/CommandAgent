# 結果サマリ

- **構成同一性**: binary SHA-256 `03159d12811aa3385d877b1d81ad7f2fdd9942e1b9af1c24314b4ece63ccdbfa`、
  sealed warikan suite SHA-256 `215abae70c63d72be8bec4ad683b92d68b68349cd7edbeea5f741ee636cd9e0c`。
  全4本でplanner=`qwen3.6:27b-coding-nvfp4`/Ollama、think未指定、
  executor=`gpt-5.6-luna`/Responses/nativeが一致した。
- **隔離**: workspace 4 unique、state/events 4 unique、run_id 4 unique、
  summary owner path 4/4一致、foreign path参照0。**cross contamination 0**。
- **成果物**: 4 workspaceを個別tree hash化し、4値はすべて異なる。run単位scrub 4/4 pass。
- **所要**: individual p50/p95=507.80/633.91秒、parallel makespan=650.62秒。
  historical single p50 170秒に対しindividual p50 2.987倍、makespan 3.827倍。
  観測4本を逐次実行した場合との実効speedupは3.160倍。
- **費用**: events usageと`pricing.toml`からの機械算出は4本合計
  `$0.00901714`。headless summaryのnullを推測で置換せず、別正本`cost.json`に
  turn/token内訳を保持した。
- **品質**: 4/4はfailed。3件=`community_schema_version_invalid`、
  1件=`community_core_manifest_path_malformed`。隔離違反ではなくmodel成果物failure。
- **未実施・逸脱・新規床**: distinct goalは3種で4本目はmain再標本（封緘suite先頭4runを
  byte変更せず使用）。単発p50はgolden-008引用で計器世代交絡あり。新規model署名2件を
  classesへ登録し、製品補修は行っていない。

# CM-4 four-process isolation probe

## 1. 実行構成

2026-08-18、同一binaryを4 processから同時起動した。suite自体は変更せず、
`warikan_001..004`（main/late/mixed/main）を使用した。全commandは`--summary-json`を
指定し、workspace、state directory、events pathをrunごとに別rootへ固定した。

| run | exit / verdict | duration | run_id | events SHA-256 | artifact tree SHA-256 |
|---|---|---:|---|---|---|
| warikan_001 | 1 / partial | 390.05秒 | `warikan_001` | `d1d81edb…69dff` | `80d5f370…accb` |
| warikan_002 | 1 / partial | 650.59秒 | `warikan_002` | `c57d8d5d…5e6b` | `4c474850…40a1` |
| warikan_003 | 1 / partial | 476.24秒 | `warikan_003` | `e2f273c0…adf1` | `1c582878…0bed` |
| warikan_004 | 1 / partial | 539.36秒 | `warikan_004` | `c9b1f032…65f4` | `a2e18696…e7f8` |

## 2. 同一性

headless summaryの`model_metadata`は4本すべて次の値だった。

```json
{"executor_provider":"openai","executor_model":"gpt-5.6-luna","planner_provider":"ollama","planner_model":"qwen3.6:27b-coding-nvfp4","ollama_think":null,"ollama_think_request_field_present":false}
```

provider turnはOllama 8件すべてrequested modelがqwen3.6の厳密ID、OpenAI 23件すべて
requested/returned modelが`gpt-5.6-luna`で一致した。model driftは0。

## 3. 交差汚染検査

| 検査 | 結果 |
|---|---|
| workspace path unique | 4/4、pass |
| state/events path unique | 4/4、pass |
| run_id unique | 4/4、pass |
| summary artifacts/events pathがowner rootへ一致 | 4/4、pass |
| events内の他run workspace/state絶対path参照 | 0、pass |
| artifact tree hash個別導出 | 4/4、pass |
| run単位scrub | 4/4、pass |

隔離衝突は観測されなかったため、E-5e隔離装備への是正は不要だった。

## 4. 並行オーバーヘッド

| 指標 | 実測 |
|---|---:|
| single p50（golden-008 warikan引用） | 170.00秒 |
| parallel individual p50 / p95 | 507.80 / 633.91秒 |
| parallel makespan | 650.62秒 |
| individual p50 / single p50 | 2.987倍（+198.7%） |
| makespan / single p50 | 3.827倍（+282.7%） |
| sum(individual duration) / makespan | 3.160倍 |

同一GPU上のplanner 4並行により個別所要は増えたが、4本の観測所要合計に対する
makespan短縮は3.160倍だった。single p50は旧計器引用なので、絶対的な性能回帰とは
断定しない。

OpenAI executor費用はrun順に`$0.00175344 / $0.00352618 / $0.00281906 /
$0.00091846`、合計`$0.00901714`。これは保存eventsのusageと封緘pricingから
機械算出した。ローカルplannerの電力費は計測していない。

## 5. 品質failure

- warikan_001/002/003: `community_schema_version_invalid`。
- warikan_004: `path does not exist: core.sha256sums...invalid quote path`を
  `community_core_manifest_path_malformed`として登録。

4件ともprovider到達性、binary/suite pin、model metadata、隔離検査はpassしている。
従ってBuilder Planeの**process隔離は成立**したが、この4並行標本の成果品質は0/4 fullであり、
品質成立を主張しない。
