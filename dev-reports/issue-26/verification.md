- Status: `blocked`

## Checks

- `cargo test --test release_distribution`: `passed`
- `shellcheck scripts/install.sh`: `passed`
- `cargo fmt --all -- --check`: `passed`
- `cargo clippy --all-targets -- -D warnings`: `passed`
- `cargo test`: `blocked` (existing environment-dependent doctor, probe, and process-group tests failed)
- `cargo publish --dry-run --allow-dirty`: `blocked` (crates.io index DNS unavailable)
- `cargo build --release --locked && ./target/release/commandagent --version`: `passed` (`commandagent 0.1.0 b91579d+dirty 2026-07-20T09:48:53Z`)
- Remote GitHub prerelease UAT (`v0.1.0-rc.20260720`): `blocked` (tag push succeeded and workflow run `29732897843` started, but the required test job remained `in_progress`; no release assets/checksums were available to verify before this run ended)
