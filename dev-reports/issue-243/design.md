# Issue 243 Design

## Goal

Make a single run summary show provider time and prompt, generation, and
thinking-token usage by caller role, together with the role's prefill ratio.
Preserve every existing `summary.md`, time-profile event, and `--summary-json`
field.

## Design

- Extend the existing `time_profile` event aggregation rather than adding new
  provider instrumentation. `provider_turn_duration` already carries
  `caller_scope`, wall duration, provider token counts, reasoning-token counts,
  and provider timing components when the selected provider reports them.
- Normalize caller scopes to the summary's existing role vocabulary:
  `planner_ultra` and `planner_step` become `planner`; ordinary execution and
  repair remain `executor` and `repair`; repair calls inside final-acceptance
  repair remain `acceptance-repair`.
- Accumulate an additive `provider_usage_by_role` map. Each role exposes
  `duration_ms`, `prompt_tokens`, `generation_tokens`, `thinking_tokens`, and
  `prefill_ratio`. Token values and the ratio are `null` when their source
  telemetry was not observed. `prefill_ratio` is provider-reported prefill
  duration divided by the provider-reported total/component duration used by
  the existing aggregate time profile.
- Render the same aggregation as a compact `Provider usage by role` Markdown
  table in `summary.md`, using `n/a` for unavailable provider measurements.
- Add `provider_usage_by_role` to `--summary-json` without changing the schema
  identifier or any existing scalar key. An event stream without provider
  turns produces an empty object.

## Verification

- Unit-test role normalization, accumulation, missing telemetry, Markdown, and
  JSON projection in `time_profile` and `headless_summary`.
- Update the focused headless-summary corpus fixture because its machine output
  contract gains the additive role-usage object.
- Run focused tests first, then formatting, Clippy, and the full Rust suite
  because shared summary and CLI output contracts are touched.
