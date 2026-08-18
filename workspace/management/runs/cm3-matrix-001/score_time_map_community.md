# Community model-tier score/time map

| arm | configuration | full meaning | score | duration p50 | duration p95 | cost total |
|---|---|---|---:|---:|---:|---:|
| A (quoted) | qwen27 → Luna | L2 spec verified | 10/12 (83.3%) | 181.5秒 | 218.5秒 | $0.01611224 |
| B | qwen27 → qwen3.5-9b-mlx | L2 spec verified | 9/12 (75.0%) | 180.5秒 | 237.8秒 | $0 |
| C | qwen27 → Terra | L2 spec verified | 12/12 (100%) | 179.0秒 | 206.55秒 | $0.20217050 |
| D | Luna → Luna | L2/L3 contract verified | 7/12 (58.3%) | 32.5秒 | 136.45秒 | $0.10150708 |

Dのlevel内訳はL2 9/L3 3、他はL2 12。点の比較はn=12/armかつAのみ既存計器
引用という範囲に限定する。

