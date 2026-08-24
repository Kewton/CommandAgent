# Issue 150 design: one-command GUI initialization

## Scope

Implement Epic #260 Lane F within `src/bin/gui_server.rs`,
`src/bin/gui_server/preflight.rs`, and `scripts/setup.sh`. Do not change the GUI
API, event schemas, runtime `.anvil/` namespace, root-overlap policy, extension
root privacy policy, or Trial Origin/authentication policy.

## Startup design

- Add `gui_server --init` as the mutating convenience path. `--check` remains a
  read-only preflight and conflicts with `--init` so its existing meaning does
  not change.
- When `--init` is used, fill only omitted `--execution-root` and
  `--extension-root` values with sibling directories below the user data root:
  `${XDG_DATA_HOME}/commandagent/{trial-workspace,extensions}` when
  `XDG_DATA_HOME` is set, otherwise
  `${HOME}/.local/share/commandagent/{trial-workspace,extensions}`. Create the
  two default roots with mode `0700` on Unix and make initialization
  idempotent.
- Never create, chmod, or replace an explicitly supplied execution or extension
  root. Existing canonicalization, ownership/privacy validation, and pairwise
  disjointness checks remain authoritative for explicit and default roots.
- When `--commandagent-bin` is omitted under `--init`, resolve `commandagent`
  from the `gui_server` executable's directory, then
  `target/release/commandagent` below the repository, then `PATH`. Preserve the
  existing `target/debug/commandagent` default outside `--init`.
- Run the complete existing preflight after initialization and before binding a
  listener. A failed check prints its detail and remediation and prevents
  startup. Successful startup retains the existing single-line URL and uses
  the unchanged Trial Origin policy.
- Make `scripts/setup.sh --gui` print one `--init` startup command instead of a
  separate preflight command plus a start command. The setup command remains
  the build half of the fresh-clone two-command path; `--init` performs the
  startup preflight.

## Remediation design

- Store remediation as owned text so messages can name the failing path, mode,
  environment variable, and expected value.
- Distinguish missing/unreadable static exports, invalid base paths, missing or
  non-directory roots, symlinked extension roots, wrong owners, missing owner
  access, non-private extension permissions, overlapping roots, binary probe
  failures, invalid tokens, and invalid Origin allowlists.
- For an extension root with group/other permissions (for example `0755`), say
  to remove those permissions with `chmod 700`; do not suggest granting owner
  access when the owner already has it.

## Verification strategy

- Extend `tests/gui_server.rs` to pin the `--init` help/argument contract,
  automatic root creation and `0700` modes, preflight-before-listen behavior,
  explicit-root non-mutation, binary auto-detection, and exact cause-specific
  remediation text.
- Extend `tests/setup_script.rs` to pin the single `--init` startup command and
  removal of the redundant separately printed preflight command.
- Run the focused GUI server and setup-script integration targets first, then
  formatting, Clippy with GUI/all targets, and the full Rust suite because
  startup and shared setup behavior are CI-sensitive.
