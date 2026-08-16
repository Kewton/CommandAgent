# Issue 76 implementation summary

## Implemented

- Fixed the GUI language policy to Japanese without adding i18n. Localized the
  shell, page-owned labels and guidance, loading/empty states, and Trial monitor
  guidance while preserving profile/provider values, event names, API resource
  identifiers, paths, and persisted status values.
- Replaced catch-copy/eyebrow intro blocks with a compact page name and one-line
  description. The description is hidden at the mobile breakpoint so the intro
  becomes one line.
- Added route metadata layouts so Overview, Trial, Run detail, Assets, and
  Measurements have distinct tab titles.
- Removed Assets from the four-item primary navigation and added a dedicated
  Assets entry on Overview.
- Replaced the decorative `CLI delegated` pill with a read-only status group.
  The new `GET /api/runtime-status` endpoint projects Trial availability and
  the existing workspace lease as idle, running, or recovery-required. The
  shell refreshes it every three seconds and never mutates runtime state.
- Recorded the language/information-design decision in the mechanism ledger and
  documented the additive runtime-status API.
- Preserved and integrated the completed Issue 63 monitoring recovery and Issue
  77 accessibility/mobile contracts before applying this change.

## Tests

- Extended GUI server integration coverage for disabled, idle, running, and
  completed runtime-status projections.
- Added source-contract coverage for Japanese copy removal, four-item primary
  navigation, Overview Assets routing, compact mobile intros, route titles, and
  runtime-status wiring.
- Updated the two-base-path Playwright smoke for Japanese headings, unique tab
  titles, navigation/Assets routing, and idle-to-running-to-idle header state.
  Existing monitor recovery, reconnect, mobile scroll, and run-ledger a11y
  assertions remain active.

## Compatibility

No existing API route, event schema/name, persisted runtime record, or `.anvil/`
namespace changed. `api/runtime-status` is additive and read-only.
