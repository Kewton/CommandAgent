# Issue #147 implementation summary

## Outcome

New users can now follow the complete interactive path directly from either
README: start the REPL, enter a plain-text request, review the Gate 1 card, and
enter `/confirm <hash>` to begin execution.

## Changes

- Replaced the startup banner's discovery-only hint with a stable first-run
  sequence and made the D-3c guard explain how to create and confirm Gate 1.
- Added `/confirm <hash>` to the shared slash-command registry and `/help`.
  Generic slash dispatch remains fail-closed because confirmation is owned by
  the boundary REPL and requires its active Gate 1 card.
- Updated the English and Japanese READMEs, slash-command references, guide
  counts, and tutorial excerpts. The registry now has 18 primary entries and
  19 accepted names including `/quit`.
- Added exact-copy unit assertions, an actual PTY assertion for the banner and
  D-3c response, `/help` integration coverage, and doc-drift checks for the
  README flow and documented command counts.

## Compatibility and scope

Gate 1 identity, confirmation persistence, dispatch semantics, event schemas,
acceptance evidence, and the `.anvil/` runtime namespace are unchanged. No
historical run evidence or migration records were modified.
