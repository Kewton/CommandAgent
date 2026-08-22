# Issue 157 implementation summary

## Implemented

- Added Gate 1 actions for returning to the compose form and requesting a fresh proposal.
- Preserved the current goal, profile, pack, provider, and model inputs when returning to compose while clearing the proposal and confirmation state.
- Invalidated the Gate 1 card and returned to compose after launch-time HTTP 412, 428, or 401 responses. Existing authentication handling clears the rejected token on 401 without clearing the remaining request.
- Extended the focused Gate 1 browser smoke for root and proxied base paths. It verifies edit preservation, successful reproposal with confirmation reset, and actionable 412/428/401 recovery.
- Kept smoke evidence token-safe by reporting only whether the Trial token is present or cleared.

## Scope

Production changes are limited to `gui/hooks/use-trial-compose.ts`, the small `use-trial-run.ts` exposure, and `gui/components/trial-gate-one.tsx`. Test coverage is in `gui/scripts/smoke.mjs`. No excluded Issue files, API schemas, event schemas, or runtime namespaces were changed.
