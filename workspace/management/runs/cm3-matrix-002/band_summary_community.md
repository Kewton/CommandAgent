# Community calibrated model-tier band summary

Full meaning: L2 `full`はspec検証済み（schema/拘束/材料）であり、runtime
smokeはプラットフォーム統合の被覆である。L3/L4 `full`は有効なpromotionと
S+Z+Bを全件passした状態である。generic assuranceを読み替えない。

| arm | planner | executor | n | 一発full [Wilson 95% CI] | 修復込みfull [Wilson 95% CI] | failed | duration五数要約 (秒) | p95 | cost total |
|---|---|---|---:|---:|---:|---:|---|---:|---:|
| B | qwen27/Ollama | qwen3.5-9b-mlx/LM Studio | 12 | 7/12, 58.3% [31.95, 80.67] | 9/12, 75.0% [46.77, 91.11] | 3 | 130 / 162.75 / 180.5 / 195.75 / 262 | 237.8 | $0 |
| B′ | qwen27/Ollama | qwen3.5-9b-mlx/LM Studio | 12 | 10/12, 83.3% [55.20, 95.30] | 11/12, 91.7% [64.61, 98.51] | 1 | 118 / 150 / 157.5 / 173 / 236 | 210.70 | $0 |
| D | Luna/Responses | Luna/Responses | 12 | 7/12, 58.3% [31.95, 80.67] | 7/12, 58.3% [31.95, 80.67] | 5 | 18 / 24.75 / 32.5 / 53.5 / 170 | 136.45 | $0.10150708 |
| D′ | Luna/Responses | Luna/Responses | 12 | 7/12, 58.3% [31.95, 80.67] | 7/12, 58.3% [31.95, 80.67] | 5 | 22 / 27 / 31 / 36.5 / 158 | 101.35 | $0.07468386 |

B′は製品較正なしのrepeatで、差を介入効果としない。D′はpackage計画欠落3件を
0件へしたが総fullは7/12のまま。このn=12/armを超えて一般化しない。
