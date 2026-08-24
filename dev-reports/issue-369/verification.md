# Issue #369 verification

- Status: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server typed_trial_intents_are_validated_frozen_and_delegated -- --exact`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo clippy --features gui --bin gui_server -- -D warnings`: `passed`
- `RUSTFLAGS='-D warnings' CARGO_INCREMENTAL=0 cargo test --features gui --test gui_server`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cd gui && node --check scripts/smoke.mjs`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && GUI_BASE_PATH=/ npm run build`: `passed`
- `cd gui && GUI_BASE_PATH=/proxy/commandagent/ npm run build`: `passed`
- `cd gui && npm run smoke -- --overview-only --output /tmp/commandagent-issue-369-ci-fix-smoke --commandagent-bin ../target/debug/commandagent`: `passed`
- `cargo test --test gui_read_only_guard`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `passed`
- `git diff --cached --check`: `passed`

## CI failure diagnosis

- Failed job: GitHub GUI Dashboard `97545206706`, run `32762745898`, Ubuntu
  24.04, head `4b9c155062957462ff6f3d9c3f0fadba984356b3`.
- Deterministic cause: POSIX shell redirection created the delegated argument
  file before `printf` wrote it, while the test waited only for file existence.
- Completion boundary: the test now waits for `/api/trial-workspace` to report
  `idle`, which production publishes only after the delegated child exits.
- Honest coverage: the fixture deliberately exposes the empty-file window, and
  the test still requires exact `--intent`, executor provider/model, and planner
  provider/model pairs.

The browser smoke passed for `/` and `/proxy/commandagent/`, at desktop 1200 px
and mobile 390 px, with provider/model preservation, candidate discovery,
datalists, unknown-model warnings, Ollama thinking, accessible role grouping,
visual order, and Tab order all intact.
