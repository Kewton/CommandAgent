# Issue #107 Design: unified profile descriptors

## Scope

Introduce one `PROFILE_DESCRIPTORS` slice as the registration point for the
six currently registered runtime profiles. Preserve public function
signatures, legacy aliases, runtime selection, admission results, route
contracts, pack compatibility, profile ordering, and GUI response bytes.
Runner chokepoints, event schemas, manifests, and historical evidence remain
unchanged.

## Descriptor boundary

- Add `src/planner/profile_descriptor.rs` with the descriptor shape approved
  by the Issue: typed and canonical identity, aliases, Japanese Trial metadata,
  admission resolver, runtime/domain implementations, create-contract
  reference, formal band key, and optional pack profile.
- Move the six runtime/domain singleton values beside the descriptor slice so
  `ProfileRuntimeRegistry`, `domain_profile`, and `profile_names` enumerate or
  resolve through that slice instead of parallel arrays and matches.
- Keep `ProfileId` closed. Preserve the legacy distinction where the `cli`
  route/admission alias maps to `python-cli`, while runtime dispatch for the
  typed legacy `ProfileId::Cli` continues to use `GenericProfile`.
- Treat `generic` and `community-mini-app` as admitted but unbanded runtime
  profiles, as they are today. The four formal Gate 1 profiles retain create
  contracts, bands, task families, and pack identities; the completeness test
  verifies those linked catalogs plus the identity/alias invariants for every
  descriptor.

## Derived consumers

- Derive runtime enumeration/resolution, admission, Gate 1 admitted profiles,
  route canonical names/create contracts, GUI Trial options, pack profile
  parsing/rendering/runtime mapping, and editor profile names from descriptors.
- Replace catalog identity literals with constants owned by the descriptor
  module. Formal band rows and task-family definitions remain the authoritative
  behavioral data, but their profile keys no longer re-register identities.
- Update the Next.js literal guardrail to reflect the identity literals removed
  from the boundary-shell catalogs and exclude the reviewed descriptor boundary
  itself from the out-of-profile dispatch scan.

## Tests and verification

Add descriptor-local tests for unique canonical names and aliases, typed parse
round trips, runtime/domain coherence, admission metadata, formal contract/band
and task-family coverage, and pack round trips. Run the descriptor and affected
integration tests first, then formatting, Clippy, the full default Rust suite,
the GUI-feature Rust suite, and GUI typecheck/lint/build because Trial option
generation changes internally.
