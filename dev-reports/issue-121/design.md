# Issue 121 design

## Scope

Add an explicit GUI/setup path without changing the existing no-argument,
`--yes`, or `--check-only` setup behavior. The setup path builds the static GUI
with one normalized base path, builds the optional server binary, can create a
private extension-root skeleton and config example, can create a private Trial
token file, and prints a directly reusable preflight/start command.

Issue 121 depends on the committed Issue 106 GUI delegate boundary and the
Issue 107-109 pack/config chain. Integrate those predecessor commits first;
keep this Issue's new child-process use in `gui_server/delegate.rs`, and consume
the `extension_root` config contract introduced by Issue 109.

## GUI preflight

- Put preflight behavior in a new `src/bin/gui_server/preflight.rs` leaf module;
  keep `gui_server.rs` limited to argument wiring, preflight dispatch, and the
  additive startup summary.
- Add `--check`, `--json`, and `--extension-root`. `--json` is valid only with
  `--check`. Clap remains responsible for malformed arguments and exit 2.
- Report stable checks for the static export/base path, the configured roots,
  the delegated binary/version probe, and Trial token/origin configuration.
  Human output uses `ok`/`ng`, a reason, and one remediation line; JSON carries
  the same check identifiers, status, detail, and remediation. Any `ng` exits 1
  and a fully green preflight exits 0 without binding a socket.
- Detect the export base path from `index.html` references to `/_next/` assets
  and compare its canonical prefix with `--base-path`. Require repository,
  execution, and extension roots (when configured) to be existing directories,
  canonicalize them, and reject any ancestor/descendant or symlink-alias pair.
- Reuse a public pairwise-disjoint helper in `workspace_policy.rs`. Add
  `delegate::check_binary()` as the only `commandagent --version` process probe;
  it clears the environment and restores the existing delegate allowlist.
- Reuse Trial token/origin parsing from `trial_access.rs` so preflight and
  startup accept exactly the same values. Normal startup retains current
  failure behavior.

## Setup and templates

- Replace the one-positional-option parser with a loop while preserving the
  legacy modes exactly. `--gui` enables Node >= 20.9, `npm ci`, GUI export, and
  GUI-server build steps. Normalize a trailing slash from `--base-path` before
  printing the server command.
- `--extension-root` creates a private root with `packs/`, `profiles/`, and an
  empty `journal.jsonl`, rejecting overlap with the repository. `--write-config`
  creates `.commandagent/config.toml` only when absent and otherwise prints a
  diff against a temporary candidate without overwriting user content. The
  example uses Issue 109's top-level `extension_root` and a documented business
  preset containing profile/provider/model/planner/plan-preset/pack fields.
- `--gui-token-file` creates a 0600 token file without printing the token and
  leaves an existing path untouched. Keep `--yes`'s existing no-secret-write
  policy: an explicitly requested token file is reported as skipped in that
  mode.
- `--profile-set` narrows optional prerequisite checks (`nextjs` checks Node and
  Playwright setup; `python-cli` checks Python) without changing required Rust
  and Git checks.
- The printed command loads the token from the file into the process
  environment, supplies the normalized build/server base path and extension
  root, and includes `--check`; a second displayed form starts the server.
  Run `commandagent --doctor --json` at the end of the GUI setup and summarize
  its result without exposing credentials.

## Tests and documentation

Add GUI process tests for a green check and independent base-path, root-overlap,
missing-binary, and invalid-token failures, including JSON and no-bind behavior.
Extend setup fixtures for GUI/config/root/token generation and retain the
existing `--check-only` assertions. Update the English/Japanese Quickstarts and
the existing GUI guide sections only; leave the later guide split to Issue 122.
Run the focused setup/GUI/guard tests first, then GUI frontend checks and the
repository-wide Rust format, Clippy, default test, and GUI-feature test gates.
