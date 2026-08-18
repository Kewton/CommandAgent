# Community planner-generation candidate band

Full meaning: L2 `full`はspec検証済み（schema/拘束/材料）であり、runtime
smokeはプラットフォーム統合の被覆である。L3/L4 `full`は有効なpromotionと
S+Z+Bを全件passした状態である。generic assuranceを読み替えない。

| arm | planner / think | executor | n | 一発full [Wilson 95% CI] | 修復込みfull [Wilson 95% CI] | failed | duration五数要約 (秒) | p95 | cost total |
|---|---|---|---:|---:|---:|---:|---|---:|---:|
| A (引用) | qwen3.6:27b / omitted | Luna/Responses | 12 | 9/12, 75.0% [46.77, 91.11] | 10/12, 83.3% [55.20, 95.30] | 2 | 139 / 167.5 / 181.5 / 191.75 / 235 | 218.50 | $0.01611224 |
| E | qwen3.8:27b-mlx / medium | Luna/Responses | 12 | 7/12, 58.3% [31.95, 80.67] | 8/12, 66.7% [39.06, 86.19] | 4 | 44 / 53 / 59 / 65.5 / 77 | 74.80 | $0.02128278 |
| F | qwen3.8:27b-mlx / high | Luna/Responses | 12 | 7/12, 58.3% [31.95, 80.67] | 8/12, 66.7% [39.06, 86.19] | 4 | 116 / 137.75 / 148.5 / 183.25 / 673 | 540.45 | $0.03342056 |

Aはgolden-008/matrix-001の引用で計器世代が異なる。E/Fは同一binary・同一model・
同一goalでthinkだけを変えた直接比較。n=12/armのため、この分母を超えて一般化しない。
採用判断はowner裁定待ちであり、自動採用していない。
