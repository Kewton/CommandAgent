# Issue #430 implementation summary

- Replaced the arbitrary byte endpoint in
  `src/planner/profiles/nextjs/api_contract.rs` with the existing
  `crate::util::floor_char_boundary` helper. This is the only production-code
  change. It preserves the saturated 4,096-byte limit and every existing
  response-recognition predicate.
- Added three focused unit tests: all UTF-8 interior bytes and adjacent
  boundaries for Japanese, box drawing, emoji, and ASCII with/without checks;
  response tokens ending at bytes 4,095/4,096/4,097 after mixed UTF-8 or long
  ASCII padding; and short/empty suffixes, response names, whitespace, returned
  fetches, and inline `.ok` checks.
- Added `tests/corpus/apps/issue430-nextjs-utf8-response-window`, a synthetic
  source-only Next.js shift-management app with Japanese UI, emoji, and a
  deliberately long box-drawing comment. Its fetch window ends at byte 4202,
  inside `─` at bytes 4201..4204. The source intentionally accepts a POST
  response without checking `Response.ok`.
- Added a corpus regression that first asserts the invalid UTF-8 cutoff and
  then requires both final and invariant profile verification to return the
  exact existing `api_contract_failure` message for the unchecked POST.
- No event/schema, runtime-state, oracle-threshold, guardrail, or historical
  evidence changes. No panic-catching or skipped inspection. The fixture's
  build is not checked and the original S1 generated-app build defect remains
  outside this issue's scope.

See `verification.md` for reproduction evidence and final check results.
