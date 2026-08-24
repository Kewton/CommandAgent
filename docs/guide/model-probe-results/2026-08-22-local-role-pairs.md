# Local Role-Pair Probe — 2026-08-22

[Model probe guide](../model-probe.md) | [日本語の判定](#日本語の判定)

This record is the measured basis for the local role-split preset shown in the
provider and configuration guides. It reports `model-probe-v3` micro-task
evidence, not scenario-UAT capability or a universal model-size rule.

## Measurement identity

- CommandAgent source parent: `494d49b4f4a2bec72be0a94fbbdfb6180241af4f`
- Candidate binary: `commandagent 0.1.0 494d49b4+dirty 2026-08-22T16:11:29+09:00`
  (`dirty` is the Issue #240 v3 probe implementation measured before commit)
- Host: macOS 26.6.2 (`25G83`), arm64
- Provider: local Ollama; `context_budget = 65536`; `planner_think = "false"`
- Executor: `qwen3.8:27b-mlx`, Ollama digest `5642e97495e1`
- Planner candidates: `qwen3.8:27b-mlx` (`5642e97495e1`),
  `qwen3.5:9b` (`6488c96fa5fa`), and `qwen3.5:4b` (`2a654d98e6fb`)
- Classifier candidates: the planner candidate, plus the final independent
  `qwen3.5:4b` arm
- Runs: two sequential observations per configuration. The first split-model
  observation can include model load; the second was run with both models
  confirmed resident by `ollama ps`.

The command shape for every row was:

```text
target/release/commandagent --cwd <arm-workspace> --preset issue240 --model-probe
```

Each workspace preset pinned all three role providers/models. Reproduce on the
target host with the same fixed battery and exact model digests; do not compare
cloud aliases or changed Ollama digests as though they were the same model.

## Observations

Durations are the sum of `provider_turn_duration.duration_ms` owned by that
role. `complete`, `partial`, and `failed` are fixed-probe completion bands, not
production capability tiers.

| UTC stamp | Executor | Planner | Classifier | Executor band / ms | Planner band / ms | Classifier band / ms | Profile SHA-256 |
| --- | --- | --- | --- | ---: | ---: | ---: | --- |
| `20260822-085135` | 27B | 27B | 27B | complete / 59,532 | complete / 1,671 | failed / 981 | `09ee2ff8a0bb2e60b760695c89ebd30a0393f338b61dc32d92f5d7b8b98ddcce` |
| `20260822-085510` | 27B | 27B | 27B | complete / 49,313 | complete / 1,557 | complete / 662 | `b421d57a7737e7e3624e01c7a7c0b588c8848b9c19606f6cc2eac6bf9c41d41e` |
| `20260822-085253` | 27B | 9B | 9B | complete / 48,367 | complete / 17,629 | complete / 685 | `08f5c9aaa8ab2da10d8f3efbb3b78dee1522eccad760f96464e1deb0012ef9d6` |
| `20260822-085413` | 27B | 9B | 9B | complete / 56,317 | complete / 3,194 | complete / 455 | `6d06258345ffc96476715ce7c8d702d785eca2cda4d6968b74c654e0f0818561` |
| `20260822-085657` | 27B | 4B | 4B | complete / 70,585 | complete / 12,867 | complete / 264 | `89b80d7823cead01999094243af22527daf5ff2464e07e5d69d199042701bc55` |
| `20260822-085756` | 27B | 4B | 4B | complete / 49,545 | failed / 978 | complete / 178 | `179c6ac57bf1f97e69924ccdbd5882c57837f184b693e3ee7341a7914e72912f` |
| `20260822-085926` | 27B | 27B | 4B | partial / 61,234 | complete / 1,468 | complete / 176 | `0d0f5cc9f9620bf1ccc8ab49246062e7539a82cc7a3689b598d8add3e6af3906` |
| `20260822-090033` | 27B | 27B | 4B | complete / 56,617 | complete / 1,691 | complete / 304 | `684db22d7583f5bc38e6749d5d16abff4a313da1b6b06acd6688e8ba4831852b` |

The raw runtime profiles and generated cards were removed after summarization
to keep the live `~/.anvil` namespace unchanged. The table above preserves each
JSON profile hash; the generated Markdown card hashes are:

| UTC stamp | Card SHA-256 |
| --- | --- |
| `20260822-085135` | `ade5ffcf1c534b0401d5b836f1e57d382f7405074ddc9d4fc75c877aee722f3c` |
| `20260822-085253` | `3a7c7724b4ec5c0324eccb7a7d9eef74bbb587ac4205faee6cdc76b06c67c9d9` |
| `20260822-085413` | `a2906804f73d271bbc61146dbd58fc809c9b2f40f29f65dbbe5540acf266f169` |
| `20260822-085510` | `697812a7191da66d95ee0093dc1a174b07a9f55e0912dd13ceceeffc398b5f07` |
| `20260822-085657` | `efc76dda2b767e459114598e5d9b1ecdf8aa9720a12c5249c9706b91bf1026c1` |
| `20260822-085756` | `2730209f317ca6a1e6016826704a416748c702a6e96a5f7c379b3ad7e78b6a20` |
| `20260822-085926` | `47c32f1446ff63a1add47088e8eb27ffe2d06e145e8d3f193bf9557353c0b9b0` |
| `20260822-090033` | `89c1bffc334e747f70aa506e448cd764cc2d3fb00b4522512e5978f3f27ebb9c` |

The cleanup targeted only the `.json` and `.md` forms of these exact generated
basenames:

```text
executor-qwen3.8-27b-mlx--planner-qwen3.8-27b-mlx--classifier-qwen3.8-27b-mlx--20260822-085135
executor-qwen3.8-27b-mlx--planner-qwen3.5-9b--classifier-qwen3.5-9b--20260822-085253
executor-qwen3.8-27b-mlx--planner-qwen3.5-9b--classifier-qwen3.5-9b--20260822-085413
executor-qwen3.8-27b-mlx--planner-qwen3.8-27b-mlx--classifier-qwen3.8-27b-mlx--20260822-085510
executor-qwen3.8-27b-mlx--planner-qwen3.5-4b--classifier-qwen3.5-4b--20260822-085657
executor-qwen3.8-27b-mlx--planner-qwen3.5-4b--classifier-qwen3.5-4b--20260822-085756
executor-qwen3.8-27b-mlx--planner-qwen3.8-27b-mlx--classifier-qwen3.5-4b--20260822-085926
executor-qwen3.8-27b-mlx--planner-qwen3.8-27b-mlx--classifier-qwen3.5-4b--20260822-090033
```

No wildcard or directory-wide cleanup was used. The final audit checked that
all 16 exact paths were absent and that pre-existing profiles, including the
most recent pre-run pair `m-20260822-023929.{json,md}`, were still present.

## Decision

The measured local starting recommendation is:

- executor `qwen3.8:27b-mlx`;
- planner `qwen3.8:27b-mlx` with `planner_think = "false"`;
- classifier `qwen3.5:4b`.

The independent 4B classifier completed 4/4 observed classifier tasks across
the 4B and final hybrid arms. In the final hybrid it took 176–304 ms, while the
all-27B classifier took 662–981 ms and completed 1/2. This supports the
classifier split for this exact local setup. The one partial executor band in
the first final-hybrid run came from the unchanged 27B executor battery; it is
reported rather than attributed to the classifier.

No smaller planner is recommended from this evidence. The warm 9B planner took
3,194 ms versus 1,557 ms for the warm 27B planner. The 4B planner was faster
when warm but satisfied the JSON contract in only 1/2 observations. Model size
alone therefore did not predict planner speed or probe reliability.

Treat the preset as a probe/smoke starting point. Before production adoption,
run the two smoke tasks and the full scenario band described in the model-probe
guide. Re-measure after any model digest, CommandAgent build, provider, context,
or hardware change.

## 日本語の判定

この実測で推奨できるローカル開始構成は、executor と planner に
`qwen3.8:27b-mlx`、classifier に `qwen3.5:4b` を使う組です。4B classifier は
対象 4 run で 4/4 完了し、最終 hybrid では 176〜304 ms でした。一方、9B planner
は warm run でも 3,194 ms で、27B planner の 1,557 ms より遅く、4B planner は
JSON 契約が 1/2 でした。このため「小さいほど planner が速い」という推奨は行いません。

この結果は固定 micro-task の probe/smoke 開始点であり、本番能力 tier ではありません。
model digest、build、provider、context、hardware のいずれかが変わった場合は再計測し、
本番採用前には 2 本の smoke と full scenario band を実行してください。
