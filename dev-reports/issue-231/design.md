# Issues #231 and #151 design

## Scope

Implement the approved combined REPL change without modifying `src/cli.rs` or
the live `.anvil/` namespace. The production changes are confined to the TUI
REPL/editor/slash registry plus a new workspace-history leaf module and minimal
module wiring. Documentation updates remain bilingual.

## Behavior

- Add `/model <id>`, `/provider <name>`, and `/profile <name>` as persistent
  settings for the current REPL session. `/model` and `/provider` mirror the
  executor-role CLI flags; planner settings remain unchanged. A provider change
  constructs the replacement client before committing the new configuration.
  Existing Gate 1 cards remain frozen, while the next plain-text request uses
  the new setting in its confirmation identity.
- Group the top-level `/help` output and support `/help <command>` details with
  usage and examples. Keep the command registry as the single completion,
  dispatch, and help source.
- Render `/status` with current execution information before session
  configuration, including an explicit idle state.
- Accept an exact Gate 1 hash as before. Unless
  `COMMANDAGENT_STRICT_CONFIRM=1`, also accept only a canonical `sha256:` prefix
  containing 8 through 63 lowercase hexadecimal digits that matches the latest
  pending card. Expand the prefix to the frozen full hash before persistence so
  stored confirmation records and emitted events remain schema-compatible.
- Add `/clear` to clear the terminal and `/last` to re-render the most recent
  assistant result without changing it.
- Store rustyline history at a state-directory leaf keyed by the SHA-256 of the
  canonical workspace path. Do not read, migrate, rewrite, or delete the legacy
  shared `<state_dir>/history.txt`. History hints require at least two entered
  characters and are clipped to the remaining terminal width.

## Safety and compatibility

- Session switches never mutate an already-rendered Gate 1 identity.
- Prefix confirmation is bounded, applies only to the single latest pending
  Gate 1 card, and preserves strict exact matching through the documented
  environment variable.
- No event names or schemas change. Confirmation persistence always receives
  the full hash.
- Workspace history names reveal only a digest, symlink aliases resolve to one
  canonical workspace identity, and the legacy shared file remains untouched.
- No CLI flag, historical evidence, or `.anvil/` state migration is introduced.

## Verification plan

1. Unit-test workspace path isolation, legacy-file preservation, hint threshold
   and clipping, session-setting validation, grouped and detailed help, status
   ordering, and strict/bounded confirmation matching.
2. Add PTY coverage using two workspaces with one state directory to prove that
   a unique history line from the first workspace is not rendered as a hint in
   the second. Cover model/provider/profile switching, `/clear`, `/last`, and
   ordered `/status` in the same focused PTY surface where practical.
3. Add a corpus contract fixture for the changed REPL command surface without
   changing event schemas.
4. Run focused TUI/editor/slash/history tests, focused ignored PTY tests, corpus
   and documentation drift checks, then formatting, Clippy, and the full Rust
   test suite.
