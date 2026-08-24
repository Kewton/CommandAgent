# Issue #370 implementation summary

## Outcome

The GUI Trial is split into four static-export-compatible pages with distinct
titles, headings, and Trial subnavigation states:

- `/try/` owns a new instruction and exact Gate 1 confirmation.
- `/try/status/?session=<id>` owns read-only progress and reconnect.
- `/try/history/` owns the compact session list.
- `/try/history/detail/?session=<id>` owns terminal result evidence.

Issue #369's committed provider/model field grouping was integrated first as
the required predecessor commit.

## Implementation

- Added fixed status, history, and detail routes plus one shared four-item Trial
  navigation. Launches, active history rows, terminal history rows, the runtime
  badge, and legacy `/try/?session=<id>` deep links route according to session
  state.
- Kept the existing monitor and terminal hooks as the lifecycle authority. New
  route wiring selects which existing stage surface is rendered; status,
  history, and detail reconnect through existing GET-only APIs.
- Added a shared access panel for direct reload and new-tab reconnect. It uses
  the existing base-path-scoped `sessionStorage` token helper and never places
  the token in a URL or `localStorage`.
- Removed the session index from `/try/`. History rows now show only start and
  update times, ID, gate/status, confirmed profile, intent, and pack, with no
  inline failure diagnosis. Terminal diagnosis, acceptance, events, and
  artifacts remain on result detail.
- Added optional `profile` and `intent` fields to the read-only session-index
  projection from the immutable confirmation identity. Old or unreadable
  confirmation records remain listable with null values; existing response
  fields and event schemas are unchanged.
- Preserved history authentication pending state, focus/visibility and runtime
  lease revalidation, freshness display, manual refresh, and last-successful
  list retention.
- Extended the dedicated browser smoke across root and proxy base paths for all
  four direct reloads, desktop/mobile layout, launch/status/detail transitions,
  old deep links, runtime-badge reconnect, GET-only behavior, authentication,
  freshness, and stale-list retention. The shared feedback smoke now preserves
  its notification observation across intentional page navigation.
- Updated the read-only Rust guard, GUI server integration assertions, user
  guides, design contract, mechanism ledger, indexes, and changelog.

## Preserved contracts

Gate 1 hash and explicit confirmation, active lease enforcement, delegated CLI
behavior, honest-failure projection, verification and acceptance semantics,
event names and schemas, and the live `.anvil/` namespace were not changed.
