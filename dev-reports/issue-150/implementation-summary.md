# Issue 150 implementation summary

## Implemented behavior

- Added `gui_server --init` as the build output's convenience startup path.
  It creates omitted default roots below
  `${XDG_DATA_HOME}/commandagent` or
  `${HOME}/.local/share/commandagent`, gives both default roots mode `0700` on
  Unix, runs the complete preflight, and binds only after every check passes.
- Kept `--check` read-only by making it conflict with `--init`. Explicit
  `--execution-root`, `--extension-root`, and `--commandagent-bin` values retain
  their existing behavior. In particular, `--init` never creates or chmods an
  explicitly supplied root, and the existing canonical pairwise-disjointness
  and private extension-root checks remain authoritative.
- Added `commandagent` discovery for `--init`: first beside `gui_server`, then
  `target/release/commandagent` below the repository, then `PATH`. Non-init
  startup retains the previous `target/debug/commandagent` fallback.
- Preserved the existing listener URL output and Trial authentication/Origin
  implementation. No route, API schema, event schema, `.anvil/` runtime path,
  or `src/bin/gui_server/api.rs` code changed.
- Changed `scripts/setup.sh --gui` to print one `--init` start command. Together
  with `./scripts/setup.sh --gui`, that command is the requested fresh-clone
  build-and-start path; `--init` incorporates the previously separate
  preflight command.

## Preflight remediation

- Made remediations path- and cause-specific for malformed base paths, missing,
  unreadable, or mismatched static exports, root path/type/symlink failures,
  ownership and permission failures, overlapping roots, binary probe failures,
  invalid Trial tokens, and invalid Origin allowlists.
- Fixed the `0755` extension-root case to identify group/other access as the
  cause and print the exact `chmod 700 <path>` remedy. It no longer tells the
  operator to grant owner access that is already present.

## Tests and guardrails

- Added GUI server integration coverage for default root creation, exact `0700`
  modes, automatic binary discovery, preflight-before-listen ordering,
  `--init`/`--check` separation, explicit-root non-mutation, missing-export
  guidance, and exact remediation text.
- Updated the setup-script fixture to require one `--init` startup command and
  reject the old separately printed `--check` command.
- Narrowed the GUI read-only guard to permit exactly one startup-root creation
  and chmod site in `gui_server.rs`; filesystem creation remains forbidden in
  every API and leaf module, and CLI execution remains confined to the
  confirmed delegate.
