# Issue #249 implementation summary

## Outcome

A registered external draft profile can now use an exact-byte pinned local
pack whose `pack.profile` is that draft profile's exact ID. The same selection
is available through CLI listing/direct selection and the GUI Gate 1 flow.
Delegated execution receives the frozen pack directory, ID, version, and hash.

The assurance and promotion boundaries are unchanged: external profiles remain
`draft`, have no measured band or compiled admission entry, and completed
execution is capped at `static` with reason `profile_not_admitted`.

## Implementation

- Added a typed process-lifetime draft `PackProfile` identity. It can be
  decoded only after an exact external profile ID has been registered; unknown
  IDs, compiled aliases, whitespace variants, and compiled profiles without a
  pack vocabulary remain invalid.
- Centralized runtime and pack-byte resolution in `profile_descriptor.rs`.
  Compiled descriptors retain their explicit `PackProfile`; draft resolution
  does not mutate `PROFILE_DESCRIPTORS`, admission, bands, or promotion state.
- Added the local-supply rule to `pack/catalog.rs`. Draft pack identities are
  rejected from admitted/repository sources and must share the registered
  profile extension root. Exact pins, retirement, source freezing,
  conformance, and local shadowing checks remain in force.
- Routed CLI pack listing/verification/selection, runtime reload, locator-aware
  Gate 1 confirmation, card rendering, GUI proposal, and delegated execution
  through the same dynamic identity and source checks. Locator-free draft Gate
  1 construction still accepts no pack, so unverified serialized pins do not
  gain a bypass.
- Kept pack behavior declarative and compiled: draft create packs have no
  invented compiled contract floor, while injection sources, phase points,
  parameters, materials, checks, scrubbing, and conformance stay closed and
  strict.

## Tests and fixture

- Added `tests/issue249_draft_local_pack.rs` for registered/unknown identities,
  local-only compatibility, exact CLI listing and selection, locator-aware
  Gate 1 rendering, and the non-full terminal sheet.
- Extended the GUI integration to propose and confirm the local draft pack,
  delegate it to a fixture CLI with the exact ID/hash environment, observe a
  completed `static / profile_not_admitted` event, and reject a cross-profile
  repository pack.
- Added `tests/corpus/apps/issue249-draft-local-pack/` with a compact v2 draft
  manifest, local material pack, exact pin, frozen Gate 1 card, and completed
  static terminal evidence.
- Updated maintainer and GUI extension documentation for the exact-ID,
  same-root, local-only rule.

The required Issue #239 predecessor commit was fast-forwarded before Issue
#249 implementation. No event schema, historical evidence, live `.anvil/`
namespace, pack admission tuple, or guardrail baseline changed.
