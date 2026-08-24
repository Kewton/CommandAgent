# Issues #186, #194, and #199 Implementation Summary

## Implemented

- Replaced the extension wizard's hard-coded profile list with the shared
  `trial-options` resource, including loading/error behavior and a safe fallback
  when the current profile is no longer advertised.
- Matched Trial's runtime authentication behavior: the wizard hides its token
  input and clears the tab-scoped token when runtime status explicitly disables
  Trial token authentication.
- Added an explicit `refresh` operation to `useResource` and wired successful
  pin and retirement operations to refresh the packs catalog immediately.
- Applied automatic-activation WAI-ARIA tab semantics with linked panels,
  roving tab stops, and ArrowLeft/ArrowRight/Home/End navigation.
- Added disclosure state/control relationships, hid decorative disclosure
  glyphs, and made expanded scrollable documents keyboard focusable.
- Consolidated pack warnings into one count status while keeping individual
  warnings as non-live notes.
- Replaced color-only assist/eval markers with check/minus glyphs plus explicit
  `あり` / `なし` text.

## Tests

- Extended the wizard-only Playwright smoke for root and proxied base paths.
  It now checks Trial profile parity, authentication-off behavior and token
  clearing, immediate post-pin catalog refresh, warning announcement behavior,
  file-presence text, keyboard tab navigation, disclosures, and targeted axe
  rules for all asset panels.
- Updated the Rust GUI source guard to pin the new data synchronization,
  lifecycle refresh, accessibility, and smoke-test contracts.

No API/event schema, extension lifecycle rule, historical evidence, or `.anvil`
runtime namespace was changed.
