# Issue #50 Verification

- Status: `passed`

## Checks

- `cargo test --lib tui::markdown::capture::tests::capture_ignores_output_from_other_test_threads -- --exact`: `passed`
- `cargo test --lib tests::direct_non_execution_actions_skip_generic_terminal_gate_card -- --exact`: `passed`
- `cargo test tui::`: `passed`
- `cargo test --test doc_drift`: `passed`
- `LC_ALL=C COMMANDAGENT_UX_DEMO_FAST=1 cargo run --quiet -- --cwd /tmp --ux-demo`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --check`: `passed`

## Notes

The locale demo exited successfully and its captured output contained no
non-ASCII characters. It exercised the ASCII activity marks, interruption
ellipsis, footer separators, and compact `2s/10m00s` live timing.

Two pre-fix parallel full-suite runs exposed unrelated presentation output in a
global Markdown capture assertion. Capture ownership was then scoped to the
creating thread without weakening the assertion. The focused capture regression,
the formerly affected test, and the final complete suite all passed.

No SVG/GIF recapture was performed; as documented in `docs/assets/ux-demo.md`,
that recorded-demo work remains delegated to Issue #43 item D.
