# Changelog

All notable changes to CommandAgent will be documented in this file.

The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Define the GUI extension boundary as four dependent layers, with consistent
  source/status/hash/assurance metadata, safe registration routes, and
  synchronized English/Japanese guidance.
- Add explicit `--allow` tool-class policy, Git workspace warnings and exit
  diffs, and doctor-visible `--offline` scope.
- Add the repository's MIT License, contribution guide, and changelog.
- Add documented `just` development tasks and a reproducible Dev Container.
- Reorganize CLI, GUI, and extension documentation by reader, with stable GUI
  compatibility anchors, simultaneous EN/JA indexes, and an in-app help map.
- Split the GUI Trial into fixed instruction, live status, compact history, and
  terminal result-detail pages with state-aware reconnect links.

### Fixed

- Make the documented PTY commands execute the ignored integration suite.
- Repair maintained documentation links and GitHub-style heading anchors, and
  bind bilingual table, flag, and slash-command counts to the implementation.
- Keep `profiles/` out of legacy local-pack discovery when an extension root
  contains both draft profiles and `packs/`.
- Let `--packs` skip malformed local candidates with warnings so valid local
  packs remain listed.

## Historical note

This changelog was introduced while the project was at version 0.1.0
(2026-07). For changes made before this file began, consult the Git history and
[`docs/dev/mechanism-ledger.md`](docs/dev/mechanism-ledger.md).
