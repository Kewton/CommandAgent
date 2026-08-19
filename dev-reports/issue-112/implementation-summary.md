# Issue #112 Implementation Summary

## Implemented

- Integrated the committed Issue #107–#111 pack stack, Issue #106 GUI Trial
  delegation split, and Issue #118 Trial frontend split required by this
  downstream issue.
- Added `GET /api/pack-options` as a read-only projection of the shared admitted
  catalog. It exposes exact identity, profile/intent compatibility, injection
  point, closed source value, and Japanese source label. For
  `python-cli × create`, it returns `cli-assist@1.0.0` and `1.1.0` as
  `承認済み`.
- Added an optional exact `id@version` pack selector to GUI Trial proposals and
  launches. The server resolves it through the shared catalog after
  deterministic routing and freezes the resulting hash, point, and source in
  `ConfirmationIdentity`; the client cannot supply those derived fields.
- Reused the shared Gate 1 and acceptance-sheet renderers, so a pinned pack
  shows both the pack row and supply-source row. Changing the selector changes
  `card_hash`, and launch with the prior hash retains the existing 412
  `trial_confirmation_stale` contract.
- Extended the sole delegated process builder to validate the confirmed pack's
  current bytes, pass exact `--pack`/`--pack-hash` arguments, and set all four
  `COMMANDAGENT_PACK_*` values from the frozen identity plus its admitted
  locator. The parent environment remains cleared, and ambient GUI-server pack
  values are neither allowlisted nor copied. Initial and continuation children
  share this builder.
- Added the persisted pack projection to GUI Trial session summaries and a pack
  column to the history panel. Real rows read the immutable confirmation record;
  optimistic rows explicitly show no selected pack until the file projection
  arrives.
- Added the admitted pack selector to the extracted Trial hook/component/API
  modules, including source labeling and proposal invalidation when the pack or
  profile changes.
- Updated GUI documentation, structural guards, server integration coverage,
  and deterministic browser smoke fixtures for the additive API/history
  contract.

## Compatibility and safety

- Existing requests may omit `pack` and retain `PackSelection::None` behavior.
- Existing confirmation/event schemas and the live `.anvil/` namespace were
  not migrated. The session-index response adds only the new nullable `pack`
  field.
- Invalid or cross-profile selectors fail before confirmation; changed pack
  bytes fail before delegation rather than silently rebinding the confirmed
  identity.
- No historical run or migration evidence was rewritten.
