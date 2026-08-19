# Issue #113 Implementation Summary

## Outcome

The legacy read-only Assets pack view is now a read-only **Extensions**
catalog. It resolves repository and configured extension-root supplies, labels
the source and approval state of every displayed row, recomputes the exact-byte
hash, and surfaces missing or stale pins inline. Eligible admitted repository
rows can hand their selector to Trial, where the registered profile and pack
are preselected together.

## Implementation

- Added a leaf GUI-server catalog module that performs bounded
  `<root>/packs/<id>/<version>` discovery, uses the shared strict pack loader,
  classifies the three compiled tuples as admitted, and applies local-over-repo
  display precedence without changing either source.
- Kept `GET /api/packs` additive and read-only. Existing `pin` consumers remain
  compatible while the response now includes source labels, expected and
  observed hashes, mismatch state, retirement/shadowing state, and Trial
  eligibility.
- Promoted `/assets/` to the Japanese **拡張** navigation and page label. Rows
  display source, selector, profile/intent, both hashes, and accessible warning
  text. `Trial で使う` is shown only for admitted, hash-matched, non-retired
  rows.
- Added base-path-safe `?pack=<id>@<version>` handoff. Trial validates the query
  against its admitted option response before applying the selector and its
  registered profile; unknown selectors are ignored.
- Updated user documentation, static guard coverage, GUI-server integration
  coverage, and browser smoke coverage for source classification, stale local
  pins, local shadowing, and Trial preselection.

## Scope and compatibility

Required predecessor commits were integrated before implementation so this
branch contains the authoritative extension-root, pack catalog/selection,
Trial, Next.js pack, and GUI preflight contracts. No mutation endpoint,
approval operation, event-schema change, runtime-state migration, corpus
contract change, or historical evidence rewrite was introduced.
