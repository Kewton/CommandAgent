# CommandAgent User Guide

[English](../../README.md) | [日本語](../../README.ja.md)

This directory is the landing page for the full user guide. The dedicated guide
is being expanded separately; until then, use the links and built-in references
below.

## CLI and REPL

- [English usage overview](../../README.md#usage)
- [日本語の使い方](../../README.ja.md#使い方)
- Run `commandagent --help` for every CLI flag supported by the installed binary.
- Run `/help` inside the REPL for every slash command supported by the installed
  binary.

## Configuration

- [English configuration overview](../../README.md#configuration)
- [日本語の設定概要](../../README.ja.md#設定)

CommandAgent reads workspace and user presets from `.commandagent/config.toml`,
with matching `.anvil/config.toml` files retained as legacy fallbacks. It does
not create those files automatically.

## Safety

Read the [security model](../../SECURITY.md) before running CommandAgent in a new
workspace or enabling `--yes`.
