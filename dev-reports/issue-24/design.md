# Issue 24 Design: One-command source setup

## Context

Source installation currently requires readers to assemble prerequisites,
`cargo install`, provider keys, and optional probe setup from several places.
Issue 24 needs one idempotent entry point while preserving optional-provider and
optional-tool boundaries. The completed predecessor chain also establishes the
bilingual README Install/Quickstart structure and the built-in interaction and
model probe commands that this script should call rather than reimplement.

## Scope

- Add `scripts/setup.sh` with interactive, `--yes`, and read-only
  `--check-only` modes.
- Validate required Cargo, Rust, and Git prerequisites; read the minimum Rust
  version from `Cargo.toml`; report Node/npm, Ollama reachability, and Python as
  optional capabilities.
- Install with `cargo install --path . --locked`, or build a release binary
  when an interactive user declines installation.
- Optionally append absent API-key entries without exposing values, inspect or
  pull Ollama models, and invoke the existing interaction/model probe commands.
- Add focused process-level tests, CI ShellCheck coverage, and matching English
  and Japanese README setup guidance.

## Design

The script resolves the repository root from its own location and keeps a
single indexed-array summary of step name, `ok`/`skipped`/`warn` state, and a
secret-free detail. Required prerequisite failures print install URLs and stop
before all mutation. Optional failures remain warnings and identify a manual
recovery command. Unexpected failures are covered by an `ERR` trap whose
message names the active step and a manual rerun path.

Mode behavior is explicit:

- No arguments prompts before install, each provider-key entry, an Ollama pull,
  interaction-probe setup, and the long-running model probe.
- `--yes` performs safe automatable work without prompts. It never requests or
  creates API-key entries and cannot choose an Ollama model when none exists;
  those steps are reported as skipped with manual instructions.
- `--check-only` runs prerequisite detection and the final summary, then exits
  without build, install, probe, or file writes.

Idempotence is provided by matching the installed binary's package version and
Git build commit before reinstalling, preserving existing `.env` contents and
key names, and relying on the existing idempotent
`--setup-interaction-probe` implementation. A locally built fallback is used
directly for smoke checks and accompanied by PATH guidance.

The focused Rust integration test copies the script and manifest into a
temporary repository and supplies fake prerequisite/program executables. It
will cover read-only check mode, outdated Rust rejection with remediation, the
noninteractive safe defaults, repeat execution, and secret-safe `.env`
appending. This exercises actual Bash control flow without network, Cargo
installation, Ollama pulls, or probe provisioning.

## Verification

Run the focused setup-script test first, then ShellCheck and Bash syntax checks.
Because CI and shared repository documentation are touched, also run formatting,
Clippy, the complete Rust test suite, and the doc-drift test explicitly.

## Risks and mitigations

- Shell portability: avoid associative arrays, `mapfile`, and GNU-only flags;
  verify with the host's Bash 3.2 and ShellCheck.
- Secret disclosure: never source `.env`, never interpolate key values into
  status output, and use silent input for interactive values.
- Accidental mutation in check mode: return immediately after prerequisite
  reporting and assert no fake mutating command was invoked.
- Stale installs: compare both package version and embedded source commit,
  falling back to reinstall when provenance cannot be confirmed.
