# CM-3 provider qualification smoke

## Result

- Terra Responses/native smoke: **pass**.
- LM Studio `provider_call` metadata/drift probe: **pass**.
- OpenAI doctor: Terra executor/planner identity, key redaction, and
  `/v1/models` reachability all `pass`.
- LM Studio doctor: host, exact executor ID, Ollama planner host, and exact
  planner tag all `pass`.
- Scrub: both curated event sources produced zero findings.

## Terra F-0 smoke

- Observed: 2026-08-18T14:05+09:00.
- Requested ID: `gpt-5.6-terra`.
- Returned ID: `gpt-5.6-terra`.
- API/protocol: Responses/native.
- Path: existing `provider_call` chokepoint.
- Tool result: one native `Read` call returned.
- Duration: 3.087 seconds.
- Usage: input 216, cached input 0, output 46, reasoning 15, total 262 tokens.
- Service tier: `default`; system fingerprint: provider omitted (`null`).
- Estimated cost: $0.00123000 using $2.50/M uncached input, $0.25/M cached
  input, and $15.00/M output.
- Official pricing source (observed 2026-08-18):
  `https://developers.openai.com/api/docs/models/gpt-5.6-terra`.
- Snapshot status: the official model page exposed the exact ID but no dated
  snapshot ID. Doctor therefore accepts the strict ID and recommends switching
  to a provider-published dated snapshot when one becomes available.
- Credential handling: `OPENAI_API_KEY` was loaded into the smoke process only.
  Doctor rendered `<redacted>` and the scrubbed event/report contains no key.

The curated response metadata is stored in `terra-metadata.json`. This is a
one-turn compatibility smoke, not a matrix observation.

## LM Studio F-0b eligibility

- Installed model selected for the down-tier executor arm:
  `qwen3.5-9b-mlx` (`mlx-community/Qwen3.5-9B-MLX-4bit`, 9B, 4-bit,
  tool-use trained).
- Loaded API identifier: `qwen3.5-9b-mlx`.
- `/v1/models`: exact ID visible; doctor model check `pass`.
- Planner: `qwen3.6:27b-coding-nvfp4` via Ollama; host and tag checks `pass`.
- Chokepoint smoke: requested model `qwen3.5-9b-mlx`, returned model
  `qwen3.5-9b-mlx`, `system_fingerprint=qwen3.5-9b-mlx`, 0.874 seconds.
- The central turn event records both the declared `model` and returned
  `provider_model_id`; a focused negative fixture returns a deliberately
  different runtime ID and proves the difference remains observable.
- Binary identity continues to be guarded by the campaign built/installed/
  executed SHA-256 pin. The model identity is independently guarded by doctor
  `/v1/models` presence before any run directory or provider spend.

The curated response metadata is stored in `lm-studio-metadata.json`. Local
generation is priced as $0 in the matrix; electricity is not measured.
