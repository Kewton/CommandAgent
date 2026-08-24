# Issue #356 verification

- Status: `passed`
- `node --check src/minimal_loop/assets/interaction_probe.js`: `passed`
- `cargo test --lib canvas_non_redraw -- --nocapture`: `passed`
- `cargo test --lib embedded_interaction_probe_asset_bytes_are_frozen -- --nocapture`: `passed`
- `cargo test --lib held_key_input_observes_player_x_and_makes_restart_judgeable -- --nocapture`: `passed`
- `cargo test --lib derived_state_attribute_alias_resolves_to_reactive_snapshot -- --nocapture`: `passed`
- `cargo test --lib input_behavior_failure_is_primary_over_consequential_restart_gap -- --nocapture`: `passed`
- `cargo test --lib space_and_breakout_contracts_keep_canvas_game_guidance -- --nocapture`: `passed`
- `cargo test --lib quiz_contract_uses_generic_interaction_guidance_only -- --nocapture`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations -- --nocapture`: `passed`
- `cargo test --test generality_guardrails -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Required Next.js scenario matrix

| Scenario | Regression measurement | Result |
|---|---|---|
| Space Invaders | game contract retains canvas/render/input guidance; corpus replay includes Space evidence rows | `passed` |
| Breakout | game contract retains canvas/render/input guidance; corpus replay includes `test0710_bs_006_breakout_combo1` | `passed` |
| Quiz | non-game contract retains generic interaction guidance only; corpus replay includes Quiz evidence rows | `passed` |

The Space and Breakout rows were asserted by
`space_and_breakout_contracts_keep_canvas_game_guidance`; the Quiz row was
asserted by `quiz_contract_uses_generic_interaction_guidance_only`. The full
generated-app corpus replay passed independently.

The held-key focused test and the full suite were run outside the filesystem
and network sandbox so loopback probe coverage executed instead of taking its
sandbox skip path. The full suite completed with zero failures; expected
ignored tests remained ignored.
