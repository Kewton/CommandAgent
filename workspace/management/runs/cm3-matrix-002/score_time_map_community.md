# Community calibrated model-tier score/time map

| arm | configuration | full meaning | score | duration p50 | duration p95 | cost total |
|---|---|---|---:|---:|---:|---:|
| B | qwen27 → qwen3.5-9b-mlx | L2 spec verified | 9/12 (75.0%) | 180.5秒 | 237.8秒 | $0 |
| B′ | qwen27 → qwen3.5-9b-mlx | L2 spec verified | 11/12 (91.7%) | 157.5秒 | 210.70秒 | $0 |
| D | Luna → Luna | L2/L3 contract verified | 7/12 (58.3%) | 32.5秒 | 136.45秒 | $0.10150708 |
| D′ | Luna → Luna | L2/L3 contract verified | 7/12 (58.3%) | 31.0秒 | 101.35秒 | $0.07468386 |

D′のlevel内訳はL2 11/L3 1。事前線のhigh-full 90%とp50 30秒をともに
満たさず、「30秒×高full率」点としては不成立。CI幅を超えた一般化はしない。
