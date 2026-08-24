# Community planner candidate band — CM-4x

Full means the Community profile projection: L2 has passed S/Z/material checks;
L3/L4 has also passed B. It does not silently claim a platform runtime smoke for
L2. A is a prior instrument generation; E12 and its 24-run extension use the same
execution revision, exact binary, provider declarations, and `think=medium`.

| arm | planner / think | executor | n | one-shot full [Wilson 95% CI] | repair-included full [Wilson 95% CI] | duration five-number summary (s) | p95 (s) | cost total |
|---|---|---|---:|---:|---:|---|---:|---:|
| A (quoted) | qwen3.6:27b / omitted | Luna | 12 | 9/12=75.0% [46.77,91.11] | 10/12=83.3% [55.20,95.30] | 139 / 167.5 / 181.5 / 191.75 / 235 | 218.50 | $0.01611224 |
| E original | qwen3.8:27b-mlx / medium | Luna | 12 | 7/12=58.3% [31.95,80.67] | 8/12=66.7% [39.06,86.19] | 44 / 53 / 59 / 65.5 / 77 | 74.80 | $0.02128278 |
| E extension | qwen3.8:27b-mlx / medium | Luna | 24 | 11/24=45.8% [27.89,64.93] | 16/24=66.7% [46.71,82.03] | 40 / 54.5 / 61.5 / 70 / 198 | 152.30 | $0.05106268 |
| **E combined** | **qwen3.8:27b-mlx / medium** | **Luna** | **36** | **18/36=50.0% [34.47,65.53]** | **24/36=66.7% [50.33,79.79]** | **40 / 54.5 / 61.5 / 70 / 198** | **129.50** | **$0.07234546** |

A→E36 repair-included full difference is −16.67 percentage points,
Newcombe 95% CI [−36.92,+14.38]. One-shot difference is −25.00 points,
Newcombe 95% CI [−47.37,+7.22]. E36 p50 is 120 seconds shorter than A;
the interval crosses zero for both quality-rate differences, so adoption remains
`owner_adjudication_pending`.
