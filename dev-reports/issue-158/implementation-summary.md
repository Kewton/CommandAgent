# Issue 158 implementation summary

## Implemented

- Added a shrink/wrap rule to `.gate-card-markdown`, allowing unbroken hashes,
  paths, and identifiers to wrap inside the clipped Gate 1 panel.
- Added `probeGateOneHashLayout` to the GUI smoke. It finds every displayed
  SHA-256-bearing Gate 1 surface, records `clientWidth` and `scrollWidth`, and
  fails unless both the markdown card and the separate confirmation ID fit at
  the exact 1440px and 390px viewports.
- Added `--gate-one-only` as additive root-and-proxy browser coverage. It keeps
  the existing real proposal, desktop/mobile screenshots, 428 confirmation
  guard, and new width assertions while stopping before CLI delegation.
- Integrated the user-required Issue 162 commit `551fa209` intact as
  `64a52d1c` before the definitive full-smoke attempt.
- Semantically integrated the verified Issue 162 auth-retry follow-up
  `ea8f8fbd` as `ceaa6d36`. The follow-up defers automatic session-index
  revalidation while the compose screen is reconnecting, invalidates stale
  requests, preserves explicit retry, and adds root/proxy wrong-token coverage.

## Result

Final verification passes on the rebuilt `ceaa6d36` candidate for both root and
proxy base paths. Each markdown card contains four complete SHA-256 values and
reports `scrollWidth == clientWidth` at both viewports: 385px at 1440px and
316px at 390px. The separate confirmation ID also fits at 357px and 288px.

The focused session-index smoke proves that a rejected token is removed and
the explicit retry remains enabled without a non-GET reconnect request. The
unchanged full smoke completes both base paths with `qwen3:8b`, retains the
Gate 1 width assertions, preserves reconnect timing, and enforces the seven
launch identity controls as absent at Gate 2, terminal, and closed states and
enabled again only after starting a new run. No assertion was removed or
weakened.

## Scope

No Gate 1 data, API/event schema, acceptance semantics, historical evidence,
or existing tutorial screenshot was changed. Fresh browser evidence is stored
under `/private/tmp`; repository history and prior run records remain unchanged.
The earlier pre-integration, stale-binary, and pre-follow-up failures remain
documented as diagnostics and are not used for the final verdict.
