# Issue 158 verification report

- Status: `passed`

## Required checks

- `cargo build --release --bin commandagent`: `passed`
- `target/release/commandagent --version`: `passed`
- `cd gui && npm run smoke -- --gate-one-only --output /private/tmp/commandagent-issue158-resume-gate-one --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cd gui && npm run smoke:session-index -- --output /private/tmp/commandagent-issue158-resume-session-index`: `passed`
- `cd gui && npm run smoke -- --output /private/tmp/commandagent-issue158-resume-full --commandagent-bin ../target/release/commandagent --model qwen3:8b`: `passed`
- `cd gui && npm run lint`: `passed`
- `cd gui && npm run typecheck`: `passed`
- `cd gui && node --check scripts/smoke.mjs && node --check scripts/session-index-smoke.mjs && node --check scripts/storage-smoke.mjs && node --check scripts/error-smoke.mjs`: `passed`
- `cd gui && npm run build`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo clippy --all-targets --features gui -- -D warnings`: `passed`
- `cargo test`: `passed`
- `cargo test --features gui`: `passed`
- `git diff --check`: `passed`

## Verdict

The rebuilt `ceaa6d36` candidate satisfies Issue 158 at both required widths
and base paths. The additive focused smoke and unchanged full smoke agree on
the hash-layout measurements. The Issue 162 auth-retry follow-up prevents
automatic session-index revalidation from racing compose/reconnect, and the
wrong-token retry smoke proves recovery without weakening GET-only behavior.
The full smoke retains and passes the exact seven-control absence contract.
