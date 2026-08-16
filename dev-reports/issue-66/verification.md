- Status: `passed`

## Checks

- `git diff --check`: `passed`
- `node --check scripts/smoke.mjs`: `passed`
- `npm run lint`: `passed`
- `npm run typecheck`: `passed`
- `npm run build`: `passed`
- `cargo test --test gui_read_only_guard trial_ui_keeps_gate_one_confirmation_and_has_no_intervention_surface`: `passed`
- `cargo build --release --bin commandagent`: `passed`
- `npm run smoke -- --output /tmp/commandagent-issue-66-smoke.d4Uk51/evidence --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test --quiet`: `passed`

## Notes

- The `node` and `npm` checks ran with `gui/` as their working directory.
- The environment's npm configuration omits development dependencies by
  default, so `npm ci --include=dev` restored the lockfile-pinned TypeScript and
  React type packages before the passing GUI checks.
- The smoke report recorded overall `ok: true` for `/` and
  `/proxy/commandagent/`. In both cases all six launch-identity controls were
  disabled at Gate 2 and terminal, programmatic token focus was rejected while
  the chip remained `GATE 2`, CLOSED returned to an editable `DRAFT`, old run
  UI was cleared, and a distinct second session reached terminal.
- Smoke evidence was written to the temporary path above. Its isolated runtime
  was removed automatically after success; no historical repository evidence
  was rewritten.
