# Issue #412 Implementation Summary

## Outcome

GUI Trial now distinguishes automatic execution-purpose ambiguity from other
Gate 1 request failures. The Issue reproduction shows a self-contained
Japanese retry message while retaining the registered route candidates as
technical detail.

## Changes

- Added the additive GUI API error code `trial_intent_ambiguous` with HTTP 422.
  The Gate 1 adapter emits it only when `intent` was omitted and the remaining
  deterministic candidates span more than one intent.
- Kept same-intent family ambiguity, typed unknown routes, invalid requests,
  authentication, Origin, workspace lease, and confirmation errors on their
  existing codes and mappings.
- Added Japanese GUI guidance that directs new-application work to `作成` and
  existing-application changes to `修正`, with the server message appended
  after `詳細`.
- Kept the existing `role="alert"` container and form controls unchanged, so
  live-region announcement, keyboard access, and text-based error semantics
  remain intact.
- Extended the Rust GUI-server integration test with the exact Issue goal,
  structured-code assertions, the same-intent negative case, and the existing
  Issue #409 `unknown` / `未計測` create proposal and delegation path.
- Extended the Playwright Gate 1 smoke with ambiguity and retry assertions.
  The existing case matrix runs and gates these checks at both `/` and
  `/proxy/commandagent/`.
- Updated the GUI Trial guide and changelog.

## Preserved Boundaries

The deterministic router, automatic-detection vocabulary, family catalog,
unmeasured-route selection, confirmation hash, CLI delegation,
verification/assurance, and event schemas were not changed.
