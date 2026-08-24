# Issue 157 design

## Problem

After a successful Gate 1 proposal, the compose form is unmounted and the card only offers confirmation and launch. A user therefore cannot edit the preserved request or intentionally replace the current proposal. Launch-time 412, 428, and 401 responses also leave the user on a card that is stale or no longer authorized.

## Design

- Add an explicit Gate 1 edit action that returns to compose with the current request fields intact while clearing the proposal, confirmation, and prior error.
- Add an explicit Gate 1 repropose action that requests a fresh workspace lease and proposal from the current request, replacing the old card and resetting confirmation.
- Treat launch responses 412 (stale confirmation), 428 (confirmation required), and 401 (rejected token) as recoverable Gate 1 invalidation: discard the proposal and confirmation, retain non-token inputs, return to compose, and retain the existing user-facing error guidance. Existing token rejection handling continues to clear only the rejected token.
- Extend the Gate 1 GUI smoke path to exercise editing, reproposal replacement, and 412/428/401 launch recovery for both root and proxied base paths.

## Scope

Change only the compose hook, the narrow `use-trial-run.ts` exposure needed by the component, the Gate 1 component, and the GUI smoke harness. Do not modify `trial-monitor.ts`, `errors.ts`, `trial-session-index.tsx`, `shell.tsx`, or `use-runtime-status.ts`; do not change API or event schemas.
