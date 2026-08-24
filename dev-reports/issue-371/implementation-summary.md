# Issue 371 implementation summary

## Outcome

The GUI extension screen now presents one four-layer extension model and keeps
the safety boundary visible from discovery through registration. Layer 1
capabilities and Layer 4 admission are repository-owned review boundaries;
Layer 2 lists external draft profiles with exact manifest hashes and a safe
registration-Issue path; Layer 3 retains the pack catalog, creation wizard,
and Trial handoff.

## Implementation

- Added the four-layer dependency map with consistent `layer`, `source`,
  `status`, `hash`, `assurance`, and registration/promotion metadata, plus
  explicit allowed and forbidden behavior.
- Added an additive, redacted extension-root status to `runtime-status` and
  surfaced unconfigured, invalid/incompatible, unpinned, and conformance
  reasons where they block use.
- Reused the existing Trial profile inventory for Layer 2 draft profiles and
  generated registration-Issue links containing only public profile identity
  and exact hash information.
- Preserved the Layer 3 pack wizard and Trial selection flow, while requiring
  both exact pinning and pack conformance for local Trial eligibility.
- Reframed Contract and Suite catalogs as read-only reference material rather
  than extension kinds.
- Extended static guards, GUI server integration coverage, and browser smoke
  coverage for both supported base paths and desktop/mobile layouts.
- Synchronized GUI help and the English/Japanese extension guides with the
  same layer names, dependency order, and assurance boundaries.

## Compatibility and safety

The runtime response change is additive. Gate 1 and delegated CLI flows still
use exact pack selectors and hashes, existing event schemas are unchanged,
and the GUI exposes no control for capability-vocabulary mutation or profile
self-admission.
