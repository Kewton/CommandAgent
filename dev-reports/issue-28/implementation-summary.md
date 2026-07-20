# Issue 28 implementation summary

## Outcome

Added a documented `justfile` and a reproducible Dev Container for the
repository's current development workflow. The task recipes preserve the
commands and warning flags from `.github/workflows/ci.yml`, while the
container supplies the language runtimes and command-line tools needed to run
them.

## Changes

- Added root recipes for debug and release builds, the complete CI-equivalent
  test sequence, corpus/guardrail/conformance/eval groups, the opt-in PTY
  command, benchmarks, and a configurable local run command. Every recipe has
  a description visible in `just --list`.
- Kept the CI commands literal in the `justfile`. The aggregate `ci` recipe
  runs the Codex harness validation, Ruff, pytest, ShellCheck, all Rust
  targets with warnings denied, corpus regression, generality guardrails,
  conformance, and the three Python evaluation modules in workflow order.
- Added `.devcontainer/devcontainer.json` based on the official Python 3.12
  Bookworm image. Reusable features install stable Rust 1.94.1 with Clippy and
  rustfmt, Node.js LTS, `just` 1.40.0, and ShellCheck 0.10.0. The Python
  harness dependencies use the same pinned versions as CI. The generated
  Dev Container lock file pins each feature implementation by digest.
- Added `.devcontainer/README.md` with the container workflow and host Ollama
  connection instructions. Ollama is not installed in the image.
- Documented optional `just` installation in `CONTRIBUTING.md` and retained
  `.github/workflows/ci.yml` as the authoritative raw-command source.
- Added an Unreleased changelog entry for the developer tooling.

## Scope notes

No production Rust code, event schema, corpus fixture, historical evidence, or
runtime namespace changed. The existing release builder and benchmark script
remain the single implementations; the new recipes are thin wrappers.

The Dev Container pins a stable toolchain above `Cargo.toml`'s Rust 1.88
minimum so the environment is reproducible. Its final image size is
1,132,262,419 bytes (about 1.13 GB). Container creation and dependency setup
require network access, but the final `just ci` validation completed with
`CARGO_NET_OFFLINE=true` and made no provider calls.
