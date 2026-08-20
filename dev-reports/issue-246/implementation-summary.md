# Issue 246 implementation summary

## Outcome

`--packs` now keeps valid local packs visible when the same extension root also
contains malformed pack candidates or nested memo directories. Each malformed
candidate is skipped with an actionable stderr warning instead of aborting the
whole listing.

## Implementation

- Fast-forwarded this branch through committed predecessor Issue 245 before
  editing the shared `pack_actions` surface. Its reserved `profiles/` namespace
  and co-located `packs/` behavior remain intact.
- Inspected committed predecessor Issue 161 and kept its GUI warning projection
  separate; no GUI schema or warning behavior needed to change for this CLI
  defect.
- Updated local catalog rendering to handle pack conformance per candidate.
  Successful compatible reports retain the existing stdout row format, while
  failures emit `warning: skipping invalid local pack ...` with the candidate
  path and conformance error, then traversal continues.
- Added an integration regression with a valid pinned pack, a malformed pack,
  and a memo-style nested directory in one extension root. It verifies success,
  the valid local row, two warnings, and both skipped paths.
- Added an Unreleased changelog entry for the user-visible fix.

Strict failure semantics remain unchanged for pack selection, `--pack-verify`,
`--pack-pin`, extension-root traversal errors, and the conformance rules
themselves. No event, schema, corpus, or live `.anvil/` contract changed.
