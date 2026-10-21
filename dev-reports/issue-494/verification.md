# Issue #494 verification

- Status: `passed`

## Checks

- `cargo test issue494 --lib`: `passed`
- `cargo test --test corpus_regression generated_app_corpus_matches_detector_and_probe_expectations`: `passed`
- `cargo test issue479 --lib`: `passed`
- `cargo test issue484 --lib`: `passed`
- `cargo test issue488 --lib`: `passed`
- `cargo test --test generality_guardrails`: `passed`
- `cargo test l2_repair_shape_control_once --lib -- --nocapture`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`

## Coverage notes

- Focused Issue tests: 8 passed. They cover the exact original grammar and
  negative grammar matrix, loader-specific guidance and correspondence,
  preservation failures, legitimate owner movement, registration persistence,
  test-only trace correlation, the saved A/B controls, and pinned Node runtime
  behavior.
- The corpus regression consumed the hash-bound Issue fixture and preserved the
  configured/Recovery command and contract-byte controls.
- Neighbor suites for dynamic-import formation (#479), package-script
  formation (#484), and later recovery behavior (#488) remained green.
- The guardrail suite passed without raising chokepoint baselines.
- Full Rust unit, integration, and documentation tests passed; tests requiring
  external live providers, frozen browser dependencies, or PTY fixtures stayed
  ignored under their existing contracts.
- The fresh `-03` replay left the `-02` control untouched and bounded every
  case to the recorded two repair responses plus the third-call guard.
