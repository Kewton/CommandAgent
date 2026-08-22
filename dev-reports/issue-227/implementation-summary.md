# Issue #227 implementation summary

## Outcome

Implemented the Issue #227 producer-side summary contract without changing
`src/lib.rs` or `docs/user/headless.md`:

- `summary.md` now starts with a compact human result covering status,
  assurance, release gate, stop reason, next action, changed-file count,
  verification result, and exit code. Existing detailed output remains under
  `## Machine details`.
- `commandagent.headless-summary/v1` retains every existing field and adds the
  always-present `status`, `gate`, `stop_reason`, `next_action`,
  `changed_files`, `verify_commands`, and `exit_code` fields.
- A shared terminal-report projector supplies both writers, keeping the human
  and JSON views aligned.
- A language-projection leaf accepts the closed `en` / `ja` set, chooses a
  default from the process locale, translates known human-facing values, and
  preserves unknown diagnostics verbatim. The follow-up consumer can wire the
  explicit language choice without changing the writer.
- Typed status and exit-code overrides are available for the Issue #221
  interrupted-run consumer.

## Evidence projection

Changed files are collected from a bounded, shell-free
`git status --porcelain=v1 -z` probe and returned in stable sorted order. A
missing or unreadable Git workspace yields an empty list.

Verification reporting uses only persisted event evidence. Commands with an
explicit result retain their recorded pass, failure, timeout, or not-run state;
commands that were merely declared remain `not_recorded`. Terminal success is
never used to manufacture verification success.

## Tests and compatibility

Added focused unit coverage for locale projection, the first-ten-line human
contract, summary append refresh, Git porcelain parsing, changed-file
collection, honest verification projection, terminal precedence, and typed
interrupt overrides. Updated the existing headless corpus fixtures and schema
tests to pin all seven additive v1 fields.

No event was renamed, no existing headless key was removed, no acceptance or
release gate was weakened, and the live `.anvil/` namespace was unchanged.
