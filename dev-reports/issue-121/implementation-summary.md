# Issue 121 implementation summary

## Delivered

- Integrated the completed Issue 107-109 config/pack chain and Issue 106 GUI
  delegate extraction. The one merge conflict retained Issue 106's extracted
  `sessions.rs` and reapplied Issue 108's `PackLocator` adaptation in the new
  `gate_one.rs` owner.
- Extended `scripts/setup.sh` with combinable GUI/config options while retaining
  the original no-argument, `--yes`, and `--check-only` flow. GUI mode enforces
  Node 20.9+, installs the locked frontend graph, builds for one normalized base
  path, builds `gui_server`, runs doctor JSON, and prints separate preflight and
  start commands.
- Added private extension-root scaffolding (`packs/`, `profiles/`, and
  `journal.jsonl`), a non-overwriting `.commandagent/config.toml` example, and
  private 0600 Trial-token creation without displaying the value.
  Existing config/token paths are preserved; config differences are displayed.
- Added `gui_server --check`, `--json`, and `--extension-root`. The new leaf
  preflight module validates the exported base path, root existence/private
  permissions/pairwise disjointness, `commandagent --version`, and the existing
  Trial token/origin contract without binding a socket. Human and JSON reports
  use stable check IDs; green exits 0, any `ng` exits 1, and Clap argument errors
  exit 2.
- Kept all child-process construction in `gui_server/delegate.rs`. The binary
  probe clears the environment and restores the existing allowlist, while Trial
  delegation now passes the validated extension root explicitly instead of
  admitting ambient pack selectors.
- Added the startup configuration summary (auth mode, execution/extension
  roots, and approved/local pack counts), focused setup/process tests, guard
  markers, and English/Japanese Quickstart plus GUI-guide documentation.

## Compatibility and boundaries

- No event or HTTP response schema changed. The preflight JSON schema is new
  and opt-in.
- The live `.anvil/` namespace and historical evidence were not changed.
- `setup.sh --check-only` retains its no-mutation early exit and its existing
  regression tests pass.
- The dashboard's Japanese `はじめに` content remains owned by Issue 120. This
  Issue builds and serves the current static export correctly at both `/` and a
  proxy base path; once Issue 120 lands, the same setup/start path serves that
  first-use card without further setup changes.
