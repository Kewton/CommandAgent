# Issue 20 Design: Bilingual User Guide

## Objective

Add a code-backed end-user reference under `docs/guide/` in English and
Japanese. The guide will cover the complete public CLI, the complete TUI slash
command surface, configuration resolution, provider setup, and the required
troubleshooting cases without changing runtime behavior.

## Source of truth

- `src/cli.rs` defines the 37 visible long flags, argument forms, Clap defaults,
  the hidden internal completion-contract flag, and the `--footer` /
  `--no-footer` conflict.
- `src/config.rs` defines effective defaults, per-field resolution, config file
  parsing, preset merging, the preset completeness check, legacy config files,
  `.env` loading, and missing-key errors.
- `src/tui/slash.rs` defines 14 primary slash commands plus the `/quit` alias,
  inline flags, goal-file expansion, help text, and profile inference.
- `src/preflight.rs` and
  `src/minimal_loop/interaction_probe.rs` define the exact preflight messages,
  choices, and interaction-probe remediation.
- `src/providers/{ollama,openai,gemini}.rs` defines provider endpoints, key
  requirements, retries, and surfaced failures. Provider setup links point to
  official provider documentation.
- `src/env_compat.rs` and the TUI modules define current `COMMANDAGENT_*`
  variables and the still-supported `ANVIL_*` compatibility names.

## Documentation shape

Create the same five files in `docs/guide/en/` and `docs/guide/ja/`:

1. `cli-reference.md`
2. `slash-commands.md`
3. `configuration.md`
4. `providers.md`
5. `troubleshooting.md`

Every localized file starts with a link to its translation. Paired files use
the same H2/H3 sequence and convey the same tables, examples, warnings, and
links. `docs/guide/README.md` becomes the bilingual table of contents and links
to `docs/model-probe.md`.

## Accuracy decisions

- Document all 37 visible flags. Mention, but do not count as public, the hidden
  `--completion-contract-json` implementation flag.
- Count `/exit` and its accepted `/quit` alias as two command names while making
  clear that the registry has 14 primary entries.
- Document all 12 preset keys accepted by the current parser. The early-stop
  completeness check covers 11 fields and omits `prompt_layout`; this is the
  current completeness trap and will be called out explicitly.
- Describe precedence per field. Five keys support the full CLI > preset >
  top-level > default chain; other fields only use the layers implemented for
  them.
- Preserve current `.anvil/` paths and list legacy `ANVIL_*` environment aliases
  alongside their preferred `COMMANDAGENT_*` names.
- Do not add runtime tests because behavior is unchanged. Use focused structural
  checks for CLI/command coverage, translation heading parity, links, and
  Markdown hygiene.

## Verification plan

- Compare the documented flag set with visible `#[arg]` fields in `src/cli.rs`.
- Compare documented slash names with `SLASH_COMMANDS` and `/quit`.
- Verify H2/H3 sequences match for every English/Japanese pair.
- Check required literal diagnostics, translation links, model-probe link, and
  trailing whitespace.
- Run `cargo test cli::tests` and the focused slash/config tests that protect the
  documented surfaces.
