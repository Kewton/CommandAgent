# Issues #231 and #151 implementation summary

## Implemented

- Added persistent REPL-session `/model`, `/provider`, and `/profile` controls.
  Model/provider changes target the executor role, preserve planner settings,
  update status sources to `repl`, and affect new Gate 1 cards only. Provider
  replacement is constructed before the active configuration/client is changed.
- Added five grouped help categories plus `/help <command>` detail pages with
  usage, aliases, and examples. The existing slash registry remains the source
  for dispatch, editor completion, help, and bilingual documentation drift.
- Changed slash `/status` presentation to put current phase, step, and scope
  before active-command details and session configuration, with explicit idle
  values when no command is active.
- Added bounded Gate 1 confirmation prefixes. A canonical matching `sha256:`
  prefix must contain 8–63 lowercase hexadecimal digits. The prefix is resolved
  only against the latest pending card and expanded to its full hash before the
  existing persistence and event path. `COMMANDAGENT_STRICT_CONFIRM=1` retains
  full-hash-only behavior.
- Added `/clear` and `/last`; the latter re-renders the most recent assistant
  result without replacing it.
- Replaced shared history loading with a new leaf helper that hashes the
  canonical workspace path and stores history below
  `<state-dir>/workspace-history/`. The former `<state-dir>/history.txt` is
  ignored and never modified. History hints require two characters and are
  clipped to the remaining prompt line width.

## Tests and fixtures

- Added unit coverage for workspace isolation, legacy-file preservation,
  command completion, history hint threshold/width, session-setting validation,
  detailed/grouped help, ordered status, result remembering, and strict/bounded
  confirmation matching through full-hash persistence.
- Updated PTY history assertions for the new leaf and added a two-workspace PTY
  scenario. It proves both legacy and foreign-workspace history are absent from
  the second workspace's first `/h` hint, then exercises model/provider/profile
  switching, detailed help, ordered status, `/clear`, and `/last`.
- Added `tests/corpus/apps/issue231-151-repl-controls/` to freeze the combined
  REPL contract without changing event schemas.
- Updated English/Japanese CLI and slash references, plus derived guide-index
  command counts required by documentation drift checks.

## Scope notes

- `src/cli.rs` is unchanged; no new top-level flag was introduced.
- Existing confirmation records, event names/schemas, historical evidence, and
  the live `.anvil/` namespace are unchanged.
- No legacy history file is migrated, rewritten, or deleted.
