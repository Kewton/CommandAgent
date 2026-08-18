# Community model-tier band summary

Full meaning: L2 `full`はspec検証済み（schema/拘束/材料）であり、runtime
smokeはプラットフォーム統合の被覆である。L3/L4 `full`は有効なpromotionと
S+Z+Bを全件passした状態である。generic assuranceを読み替えない。

| arm | planner | executor | n | 一発full [Wilson 95% CI] | 修復込みfull [Wilson 95% CI] | failed | duration五数要約 (秒) | p95 | cost total |
|---|---|---|---:|---:|---:|---:|---|---:|---:|
| A (quoted) | qwen27/Ollama | Luna/Responses | 12 | 9/12, 75.0% [46.77, 91.11] | 10/12, 83.3% [55.20, 95.30] | 2 | 139 / 167.5 / 181.5 / 191.75 / 235 | 218.5 | $0.01611224 |
| B | qwen27/Ollama | qwen3.5-9b-mlx/LM Studio | 12 | 7/12, 58.3% [31.95, 80.67] | 9/12, 75.0% [46.77, 91.11] | 3 | 130 / 162.75 / 180.5 / 195.75 / 262 | 237.8 | $0 |
| C | qwen27/Ollama | Terra/Responses | 12 | 11/12, 91.7% [64.61, 98.51] | 12/12, 100% [75.75, 100] | 0 | 127 / 163.75 / 179 / 188.75 / 217 | 206.55 | $0.20217050 |
| D | Luna/Responses | Luna/Responses | 12 | 7/12, 58.3% [31.95, 80.67] | 7/12, 58.3% [31.95, 80.67] | 5 | 18 / 24.75 / 32.5 / 53.5 / 170 | 136.45 | $0.10150708 |

Aは旧計器からの引用であり、B/C/Dの36件だけが新規live計測。この分母では
Cが12/12、Bは閉語彙3件、Dはpackage/spec artifact欠落5件という異なる署名を
示した。n=12/armのCI幅を超えた一般化はしない。

