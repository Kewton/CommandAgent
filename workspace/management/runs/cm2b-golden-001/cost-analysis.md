# CM-2c cost wiring analysis

The prior warikan_001 events contain `provider_total_tokens`, `provider_cached_input_tokens`, `provider_model_id=gpt-5.6-luna`, and `provider= openai` on `provider_turn_duration` events. The summary projected `provider_cost_usd: null`; no cost field was emitted. The pricing file also parsed the dotted model as nested TOML keys, so exact model lookup failed. CM-2c quotes the model table key and derives cost from the recorded event shape.

Observed result: 224237 total tokens, 199864 cached input tokens, `cost_usd=0.01628088` using `pricing.toml` (`$0.20/M` input, `$0.02/M` cached input, `$1.20/M` output). No token or price estimate was invented.
