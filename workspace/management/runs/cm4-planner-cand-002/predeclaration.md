# CM-4x E extension predeclaration

Recorded at `2026-08-18T21:31:57+09:00`, before provider execution.

- Additional denominator: 24 runs. Combined with the existing same-instrument E 12 runs, the final denominator is 36.
- Tasks: warikan, mochimono, and vote; 8 additional runs per task. Combined observations contain four repetitions of each of the three sealed goal variants per task.
- Planner: `qwen3.8:27b-mlx`, Ollama, `think=medium`.
- Executor: `gpt-5.6-luna`, OpenAI Responses/native.
- Instrument: execution revision `f2072b570b5eddde06215e8025cce859335c7916`, binary SHA-256 `b9f9818602d34c1b383a1910bcaf0c8737d596bcf0d792f5b3e0399d330c13fa`.
- Extension suite SHA-256: `41b180bcc41c2a278c9ed774553c27ae8a2eafbc251d8f31820e7003342a4b2b`.
- Existing E baseline: full 8/12=66.7%, Wilson 95% CI [39.06%, 86.19%]. No point prediction and no automatic adoption threshold.
- The local planner has USD API cost `$0`. The Luna executor remains necessary for same-arm aggregation, so its provider usage cost is measured separately and is not represented as zero.
- Adoption remains an owner decision after the n=36 result; this campaign does not alter defaults.
