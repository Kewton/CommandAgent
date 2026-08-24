# Issue #16 Design

## Scope

Migrate supported environment-variable entry points from `ANVIL_*` to
`COMMANDAGENT_*` and migrate configuration discovery from `.anvil/config` and
`.anvil/config.toml` to `.commandagent/config` and
`.commandagent/config.toml`. Preserve existing users through read-only legacy
fallbacks. This phase does not migrate the live `.anvil/` runtime-state tree,
event schemas, or historical evidence.

## Environment Compatibility

- Add a small `src/env_compat.rs` leaf module. Callers pass a canonical
  `COMMANDAGENT_*` name; the helper reads it first and derives the matching
  legacy name only when the canonical name is absent.
- Presence, including an empty value, determines precedence. Therefore a set
  canonical variable always wins over a legacy variable, while callers remain
  responsible for interpreting empty strings.
- When a legacy-only value is consumed, emit a deprecation warning naming both
  variables. Track warned legacy names process-wide so repeated reads warn once
  per variable.
- Keep lookup and warning sinks injectable inside the helper so the four-value
  precedence matrix and the warn-once contract can be tested without mutating
  the process environment.
- Route Rust environment reads through the helper and change child-process,
  test, build, documentation, eval, and script-facing canonical names to
  `COMMANDAGENT_*`. Small Python and shell compatibility helpers retain legacy
  fallback for script-only variables without spreading legacy spellings back
  through call sites.

## Configuration Paths

- TOML search order is workspace `.commandagent/config.toml`, workspace
  `.anvil/config.toml`, home `.commandagent/config.toml`, then home
  `.anvil/config.toml`. This preserves workspace-over-home precedence while
  making the new namespace authoritative at each scope.
- The extensionless compatibility parser checks workspace
  `.commandagent/config` before workspace `.anvil/config`.
- Existing per-field merge behavior remains unchanged: a higher-priority file
  wins for fields it defines, and lower-priority files may fill missing fields.
- Add focused tests for new-only, old-only, and both-present path cases. Update
  existing config tests and user documentation to make `.commandagent` the
  primary path while retaining explicit legacy fallback coverage.

## Verification

Run the focused environment/config tests first, then formatting, all-target
Clippy, the acceptance scan, and `cargo build && cargo test --quiet`. Because
Python harness files change, also run Ruff and the focused eval Python tests.
Record only checks that actually complete successfully in the verification
report.
