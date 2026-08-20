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

## Result

The focused rebuilt-candidate smoke passes for both base paths. Each markdown
card contains four complete SHA-256 values and reports `scrollWidth ==
clientWidth` at both viewports: 385px at 1440px and 316px at 390px. The separate
confirmation ID also fits (357px and 288px respectively), and visual inspection
of both fresh screenshots confirms that the values wrap rather than clipping.

Final verification remains blocked because the required unchanged full smoke
still times out in the root reconnect flow after the release binary was rebuilt
from the Issue 162-integrated candidate. No smoke assertion was removed or
weakened, and the proxy full case was not reached after the root failure.

## Scope

No Gate 1 data, API/event schema, acceptance semantics, historical evidence,
or existing tutorial screenshot was changed. Fresh browser evidence is stored
under `/private/tmp`; repository history and prior run records remain unchanged.
