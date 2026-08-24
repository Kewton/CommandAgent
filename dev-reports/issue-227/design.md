# Issue #227 design

## Scope and constraints

Issue #227 owns the summary/headless output leaf code for the Wave 4 E lane.
The approved row decision is narrower than the original Issue proposal:

- own the human-first `summary.md` writer;
- extend `commandagent.headless-summary/v1` additively with terminal status,
  gate, stop reason, next action, changed files, verification commands, and
  process exit code;
- provide the `ja` / `en` language projection leaf;
- report changed files and verification results from existing evidence; and
- add focused schema coverage.

Do not edit `src/lib.rs` or `docs/user/headless.md`. Issue #221 owns those
consumer surfaces after this additive API lands. Preserve existing event names,
field types, acceptance semantics, and the live `.anvil/` namespace.

The worktree starts clean at `origin/develop` commit `494d49b4`; there are no
required predecessor commits for this row.

## Design

Add small leaf modules below `eval_events` instead of growing the existing
summary functions:

1. A terminal-report projector reads the persisted event stream and produces
   one neutral report used by both human and JSON writers. It derives the
   normalized process status (`completed`, `failed`, or `interrupted`), release
   gate, stop reason, next action, changed paths, verification observations,
   and exit code. Explicit status/exit overrides remain available for the
   Issue #221 SIGINT/exit-code consumer.
2. Changed paths come from `git status --porcelain=v1 -z` with no shell and are
   sorted/deduplicated. A missing workspace, non-Git workspace, or failed
   read-only probe honestly produces an empty list.
3. Verification reporting consumes only existing event evidence. Declarative
   command-check results, build-verifier observations, and pre-satisfied step
   summaries retain their recorded pass/fail status. Declared commands with no
   recorded result remain `not_recorded`; they are never upgraded from a
   terminal success alone.
4. A language leaf parses `ja` / `en`, resolves the default from `LC_ALL`,
   `LC_MESSAGES`, then `LANG`, and projects human labels plus a closed set of
   status/gate/next-action phrases. Unknown evidence values remain unchanged so
   localization cannot erase diagnostic meaning.
5. The human writer keeps the existing build banner as the first line for the
   established compatibility checks. The first ten lines then expose the human
   result, assurance, gate, stop reason, next action, changed-file count, and
   verification result; the exit code immediately follows. Detailed
   changed-file and verification lists follow, while the previous verbose
   summary is retained under `## Machine details`.
6. `HeadlessSummary` keeps schema version v1 and every existing key/value
   contract. The seven new keys are always serialized: nullable scalar
   `status`, `gate`, `stop_reason`, `next_action`, and `exit_code`, plus array
   `changed_files` and `verify_commands`. `Source` keeps its existing
   constructor and gains typed terminal override methods for Issue #221.

`append_run_summary` will unwrap an existing human-first document back to its
machine body, append the new machine detail, and render one refreshed human
header. This prevents stale `Status: running` headers and avoids nesting a new
summary header on every append.

## Compatibility and honest failure

- `commandagent.headless-summary/v1` is not renamed.
- Existing headless keys remain present and retain their current meanings.
- No event schema changes are required.
- No verification, acceptance, assurance, or release gate is weakened.
- Verification commands without direct result evidence are explicitly
  `not_recorded` in the human projection.
- Existing summary machine detail remains available for current parsers during
  the staged migration, below a stable heading.

## Tests and verification

Add focused tests for locale resolution and translation, porcelain parsing and
Git changed-path collection, honest verification projection, the first-ten-line
human contract, append refresh behavior, and all additive headless v1 fields.
Update the existing headless corpus fixtures to freeze the new event-derived
schema values without changing their old evidence.

Run the focused summary tests first, then `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test` because shared CLI
summary behavior changes.
