# Issue #249 verification

- Status: `passed`

## Checks

- `cargo test --test issue249_draft_local_pack`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations -- --exact`: `passed`
- `cargo test planner::pack:: --lib`: `passed`
- `cargo test planner::profile_descriptor::tests --lib`: `passed`
- `cargo test --test pack_actions`: `passed`
- `cargo test --test issue117_extension_profiles`: `passed`
- `cargo test --test doc_drift`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test --features gui --test gui_server gui_lists_and_proposes_an_external_draft_profile_with_a_local_pack -- --exact`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Notes

The full GUI target initially found the expected generated-export prerequisite
absent (`gui/out/index.html`). After installing the lockfile's development
dependencies and producing the documented root export, all 31 GUI integration
tests passed. Generated `gui/node_modules` and `gui/out` remain ignored and are
not part of this change.
