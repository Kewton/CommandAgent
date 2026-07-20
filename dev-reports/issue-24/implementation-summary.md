# Issue 24 Implementation Summary

## Outcome

Added an executable, Bash 3.2-compatible `scripts/setup.sh` that turns a source
checkout into a guided CommandAgent installation while keeping optional tools
and credentials optional. It supports interactive setup, non-interactive safe
defaults with `--yes`, and a read-only `--check-only` prerequisite report.

## Changes

- Read `rust-version` and package `version` from `Cargo.toml`, then validate
  Cargo, Rust, and Git with URL-bearing remediation for required failures.
- Report Node/npm, Ollama CLI or localhost reachability, Python 3, and remote
  provider key presence as optional checks without making them fatal.
- Install with `cargo install --path . --locked`, skip a matching installed
  build by package/Git provenance, or fall back to `cargo build --release` with
  a PATH command when interactive installation is declined.
- Verify the resulting executable with `commandagent --version`.
- Prompt silently for absent Gemini/OpenAI keys, append only absent entries,
  preserve existing `.env` content, reject symlink/non-file targets, and create
  a new `.env` with mode `0600`. `--yes` never prompts for or writes secrets.
- Display `ollama list`; when it is empty, accept only a user-entered model name
  for an optional pull. No default model is imposed.
- Offer the existing `--setup-interaction-probe` command when Node/npm are
  present and offer the time-consuming `--model-probe` smoke when Ollama is
  reachable.
- Print an `ok`/`skipped`/`warn` summary and README Quickstart pointer. Expected
  and unexpected failures identify both the failed step and manual recovery.
- Add six hermetic process-level tests covering check-only immutability, Rust
  version rejection, non-interactive installation and repeat idempotence,
  release-build fallback, explicit Ollama model selection, and secret-safe
  `.env` creation/reuse.
- Add a `shellcheck scripts/*.sh` CI step. Resolve the three pre-existing
  findings in `scripts/bench.sh` and `scripts/snapshot-uat-corpus.sh` so that
  the complete new CI scope is warning-free.
- Replace the planned-script note in both README translation partners with the
  new command and its two modes.

## Predecessor integration

Before implementation, the completed Issue 22 aggregate tip was fast-forwarded
into this branch. That tip contains all required Issue 19–23 and Issue 25
commits, so this change targets their final README, doctor, CLI, and doc-drift
contracts without duplicating or rewriting predecessor work.
