# Issue #420 Implementation Summary

## Outcome

CommandAgent now distinguishes its untouched deterministic Next.js page
templates from task-specific implementation. An `implement` step over the
engine-owned scaffold cannot satisfy the `implementation` obligation after a
Read; it continues until the page is rewritten and reports
`src/app/page.tsx` in `changed_paths`.

## Changes

- Added a Next.js profile predicate for the exact TypeScript and JavaScript
  page templates authored by scaffold completion. Matching tolerates platform
  line endings and surrounding whitespace without using broad content
  heuristics.
- Classified a matching engine-owned page as `scaffold` in runtime acceptance
  evidence. Once the page content is rewritten, the existing implementation
  classification applies.
- Added an integration test for the scaffold-to-implementation obligation
  transition and a minimal-loop regression for Read followed by Write.
- Updated the existing captured fallback-page corpus case to require and reject
  an unsatisfied `implementation` obligation.

## Compatibility

No event names or fields changed. The step short-circuit policy itself is
unchanged, so setup, inspect, verify, and already-supported non-scaffold flows
retain their previous behavior. The full suite also covers the prior
`session_01a06793` API-route regression.
