# Issue 20 Implementation Summary

## Outcome

CommandAgent now has a complete bilingual end-user guide under `docs/guide/`.
English and Japanese readers have matching references for the CLI, TUI slash
commands, configuration, providers, and troubleshooting, with a bilingual guide
index linking the full set and the model behavior probe.

## Implemented

- Replaced the placeholder `docs/guide/README.md` with English and Japanese
  tables of contents, README/security pointers, and a link to
  `docs/model-probe.md`.
- Added paired `en/` and `ja/` CLI references covering all 37 public flags,
  arguments, effective defaults, action selection, combinations, and the
  `--footer` / `--no-footer` conflict.
- Added paired slash-command references covering the 14 primary registry
  entries plus `/quit`, inline `--profile`, `--style`, and `--prompt-layout`,
  goal quoting, `$(cat <path>)`, profile inference, and TUI input/interrupt
  behavior.
- Added paired configuration references covering per-field precedence, all
  search paths, all 12 currently accepted preset keys, the 11-field early-stop
  condition and omitted-`prompt_layout` trap, all five top-level keys,
  extensionless legacy config, and current plus `ANVIL_*` environment names.
- Added paired provider setup references for Ollama, OpenAI, and Gemini,
  including official key/setup URLs, executor/planner roles, environment and
  `.env` loading, Ollama host behavior, do-not-display guidance, and recommended
  `.env` mode `600`.
- Added paired troubleshooting references for the required exact diagnostics,
  busy-port choices, degraded interaction-probe gates, footer recovery, invalid
  model IDs, and an unreachable Ollama server.
- Added translation links at the start of every localized file and maintained
  matching H2/H3 level sequences in every pair.

## Contract accuracy

- Derived the public CLI set and Clap-facing values from `src/cli.rs`; the
  hidden completion-contract option is identified but excluded from the public
  count.
- Derived effective model, context, timeout, plan-preset, footer, stream, config
  file, preset, environment, and state-directory behavior from `src/config.rs`
  and `src/env_compat.rs`.
- Derived command names, help usages, inline parsing, expansion, and inference
  from `src/tui/slash.rs` and `src/planner/profile.rs`.
- Preserved the exact `.anvil/` runtime paths and legacy `ANVIL_*` spellings
  supported by the current compatibility layer.
- Used the exact preflight and provider error prefixes implemented in
  `src/preflight.rs`, `src/minimal_loop/interaction_probe.rs`, and
  `src/providers/`.

## Tests

No production behavior, schema, corpus contract, or runtime state changed, so
no Rust or corpus fixtures were modified. Verification compares documented
flags and commands directly with their source registries, checks bilingual
heading and link structure, and runs the focused existing CLI, configuration,
slash-command, and preflight test groups.
