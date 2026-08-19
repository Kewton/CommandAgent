# Issue 119 implementation summary

## Outcome

Implemented the remaining general GUI improvements from the Issue 100 review.
Runtime and read-only resource refresh now follow tab visibility, session
navigation works through both supported base paths, all displayed timestamps
share one formatter, and the repository-backed destination has an explicit
source-oriented name.

## Changes

- Fast-forwarded the branch through required predecessors Issue 122 and Issue
  115 before editing, preserving the reorganized GUI documentation and pack
  wizard contracts.
- Kept `Shell` as the sole `runtime-status` poll owner. Its sequential timeout
  loop now aborts/stops while hidden, resumes immediately when visible, keeps
  the last successful projection on failure, and never starts a concurrent
  request within one mounted document.
- Changed `useResource` to stale-while-revalidate behavior for Overview,
  repository run records, Measurements, and Extensions. Window focus and
  visible-tab transitions refresh data; a failed refresh leaves the prior value
  rendered with the new error.
- Added a base-path-safe runtime session badge link to
  `try/?session=<id>`. Terminal history links now mark their target with
  `data-session-id`, scroll through the existing fragment, and animate a
  temporary row highlight.
- Replaced the three date rendering paths with the single exported
  `dateTimeLabel` ja-JP formatter. Repository records, Trial monitor freshness,
  session-index freshness, and Trial start/update times now have the same
  display convention.
- Renamed the repository-backed page, navigation item, metadata, headings,
  empty state, docs, and smoke expectations to **リポジトリ実行記録**, while
  retaining `repository / workspace/management/runs` and
  `execution root / .anvil/runs` as distinct source disclosures.

## Verification coverage

- Extended the focused two-base-path browser smoke to count active
  runtime-status requests, hold responses long enough to expose overlap, verify
  zero hidden polling and immediate visible refresh, exercise stale resource
  retention/focus refresh, follow both badge and Terminal links, assert the row
  highlight, and compare Trial timestamps with the shared ja-JP convention.
- Updated the read-only browser smoke for the navigation, heading, and tab-title
  contract on both base paths.
- Extended the Rust GUI source guard for sole poll ownership, visibility/focus
  listeners, stale-data retention, shared formatter ownership, base-path query
  construction, link/highlight wiring, and the repository source name.
- Updated the reader-oriented GUI history and operations docs introduced by
  predecessor Issue 122.

## Compatibility and safety

No Rust production behavior, API or event schema, filesystem layout,
authentication rule, `.anvil/` namespace, historical run evidence, or write
boundary changed. Existing Trial session and resource failures remain visible;
the change does not convert any failed observation into success.
