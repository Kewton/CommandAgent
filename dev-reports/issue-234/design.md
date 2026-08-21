# Issues 234 and 235 Design

## Scope

Implement Epic 260 Lane C as one coherent change. The executor and repair
roles retain the existing `--think` behavior. Step and ultra planners default
to Ollama `think=false`, while an explicit global `--think` continues to win.
Gate 1 classification uses its own preset-resolved provider/model, defaults to
the planner provider/model, and always runs with `think=false` and a generation
budget no greater than 64 tokens.

The implementation stays out of the planner and minimal-loop growth
tripwires. `src/config.rs` owns resolution, `src/provider_call.rs` owns
scope-specific call settings and telemetry, and
`src/tui/boundary_shell/ambiguity.rs` supplies the bounded classifier call
arguments. Provider construction needs only the minimal role-aware wiring
required to give the existing planner client its resolved planner setting.

## Configuration

- Add optional preset keys `planner_think`, `classifier_model`, and
  `classifier_provider`.
- Resolve `planner_think` as explicit global `--think`, then the selected
  preset, then built-in `false`.
- Resolve classifier provider/model from the selected preset, falling back to
  the resolved planner provider/model. A classifier provider that differs from
  the planner provider requires an explicit classifier model.
- Validate OpenAI classifier model IDs with the same strict model validation
  used for executor and planner roles.
- Keep `ollama_think` unchanged as the executor/repair setting so omission
  remains omission for those roles.

## Provider calls and evidence

- Normal planner calls use the resolved planner think value; executor and
  repair calls use the existing executor value.
- Add a narrowly scoped call override for Gate 1. It selects the classifier
  provider and model, fixes think to `false`, and caps `num_predict` at 64
  without changing the shared executor default.
- Add a `think` field to every `provider_turn_duration` event. Encode an
  explicit Ollama value using the existing vocabulary and encode no setting as
  `omitted`; this is additive and leaves existing event keys unchanged.
- Preserve the closed-candidate parser, response byte limit, cancellation,
  timeout, and honest typed-unknown behavior.

## Tests and verification

- Add config tests for default/explicit planner think and independent
  classifier provider/model resolution.
- Add provider-boundary tests that inspect an Ollama request for planner
  `think=false`, Gate 1 `think=false`, and `num_predict <= 64`, and assert
  additive event evidence while executor omission remains unchanged.
- Update the ambiguity tests to assert the configured classifier provenance
  and bounded turn evidence without changing its closed-list contract.
- Run focused Rust tests first, then formatting, Clippy, and the full Rust test
  suite because configuration and provider-call behavior are shared.
- Run the required four-profile create UAT parity check when the local provider
  and corpus harness are available, recording the exact result honestly.

## Scope authorization follow-up

The user subsequently authorized mechanical default-field additions at every
exhaustive `Config` initializer, the minimal role-aware constructor wiring in
`src/providers/mod.rs`, focused tests, and the bilingual configuration rows
required by the repository's documentation contract. This cleared the earlier
scope blocker without authorizing unrelated downstream behavior.

Before final verification, the committed Issue 208 branch was merged. Its only
overlap with this design was the `Config` initializer in
`src/planner/profiles/python_cli.rs` (plus the shared conformance initializer).
The merge was clean, and the role defaults were reapplied as mechanical fields
afterward; no Issue 208 behavior was rewritten.
