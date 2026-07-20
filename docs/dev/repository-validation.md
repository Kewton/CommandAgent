# Repository Validation

These procedures are for CommandAgent maintainers. End-user installation and
usage stay in the root [English](../../README.md) and
[Japanese](../../README.ja.md) READMEs.

## Build and test

Run the narrowest relevant check first, then broaden in proportion to risk:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run -- --help
```

## UAT provenance

Before a UAT run, verify the binary provenance:

```bash
commandagent --version
command -v commandagent
```

`commandagent --version` should show the intended commit, dirty marker, and
build timestamp. `command -v commandagent` should resolve to the expected
`target/` binary or installation path for the run.

The offline presentation walkthrough is available for manual TTY review:

```bash
COMMANDAGENT_PTY_TESTS=1 cargo test tui_pty_smoke -- --ignored
commandagent --ux-demo
commandagent --help
```

`commandagent --ux-demo` exercises the banner, plan card, phase header, activity
narration, live footer interrupt hint, and terminal summary card without
contacting a provider. If a terminal shows cursor-region artifacts, rerun with
`--footer off`; scrollback breadcrumbs remain enabled.

## Live provider tests

`OPENAI_API_KEY` and `GEMINI_API_KEY` are read from the process environment
first, then from `.env` in the active workspace. Values are redacted from logs.

Live provider tests are gated:

```bash
COMMANDAGENT_LIVE_PROVIDER_TESTS=1 cargo test live_ -- --ignored
```

Override smoke model IDs with `COMMANDAGENT_OPENAI_SMOKE_MODEL` and
`COMMANDAGENT_GEMINI_SMOKE_MODEL` when needed.

## Clean release build

```bash
./scripts/build-release.sh
target/release/commandagent --version
```

The release script builds with Cargo's optimized release profile in an isolated
temporary target directory, verifies the staged executable's package version
and Git commit provenance, and publishes it at `target/release/commandagent`.
On success, that executable is the only entry left under `target/release`. On a
build or provenance failure, the previously published executable is preserved.
Temporary release artifacts are removed on both paths; ordinary `cargo build`
and `cargo test` commands continue to use Cargo's normal cache.

## Local symlink

A maintainer may point a local launcher at the clean release binary:

```bash
ln -sfn "$(pwd)/target/release/commandagent" "$HOME/.local/bin/commandagent"
commandagent --help
```

An existing `commandagentdev` symlink to `target/release/commandagent` continues
to use the newly published executable and can be checked with
`commandagentdev --version`. These symlinks are local conveniences and are not
part of the copy artifact.

## Copy validation

Validate that the tracked repository works without local ignored state:

```bash
tmp=$(mktemp -d)
git archive --format=tar HEAD | tar -x -C "$tmp"
cd "$tmp"
cargo test
cargo run -- --help
```

## Codex harness

Repository-local Codex skills live under `.agents/skills/` and are invoked as
`$skill-name`. See [docs/codex-harness.md](../codex-harness.md) for the migrated
command map, orchestration entry point, safety boundaries, and validation
commands.
