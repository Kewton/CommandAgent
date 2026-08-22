# Issue 241 verification

- Status: `passed`

## Checks

- `cargo test --lib provider_call::tests:: -- --test-threads=1`: `passed`
- `cargo test --lib providers::ollama::tests:: -- --test-threads=1`: `passed`
- `COMMANDAGENT_PTY_TESTS=1 cargo test --test tui_pty tui_pty_planner_interrupt_closes_http_and_reaches_gate_four_within_one_second -- --ignored --exact`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `uv run --offline --with PyYAML==6.0.3 cargo test -q`: `passed`
- `git diff --check`: `passed`

## Evidence

- The full dependency-complete suite reported 2,008 passed library tests and
  16 intentionally ignored library tests; every default integration and
  documentation target also passed.
- The focused PTY regression observed planner streaming with `--stream off`,
  an interrupted Gate 4 in less than one second, and HTTP disconnect in less
  than one second while keeping the raw planner payload off the terminal.
- The provider-call cancellation test confirmed both existing interruption
  event forms: `provider_turn_duration` with `aborted_by_user: true` and
  `provider_turn_aborted_by_user`.
- A diagnostic run of all eight opt-in PTY tests passed six, including both
  planner interruption cases. Two unrelated legacy cases still submit commands
  without the current Gate 1 confirmation and fail before reaching their
  asserted behavior; they are outside Issue 241's required verification.
- No corpus fixture, event schema, historical run evidence, or `.anvil/`
  runtime namespace changed.
