# Issue 373 design: make Overview the product landing page

## Current behavior

The Overview mixes a dismissible first-use card with repository-run counts, the
score/time capability map, measurement-band rows, an extension teaser, and the
recent repository-run table. Operational detail therefore appears before the
product purpose, safety model, and path from a goal to an honestly verified
result. Readiness and an active Trial are visible only as compact shell badges,
and the page does not explain where Issue 370's separated Trial surfaces or
Issue 371's extension layers are owned.

## Predecessor integration

- `feature/issue-369-gui-trial-provider-model` groups executor and planner
  provider/model fields without changing Gate 1 or request contracts.
- `feature/issue-370-gui-trial` contains the same Issue 369 tree plus the fixed
  `/try/`, `/try/status/`, `/try/history/`, and `/try/history/detail/` routes.
  Overview links will use those route helpers, including the live session ID
  only on the status route.
- `feature/issue-371-gui-extensions` defines the dependency order as compiled
  capability vocabulary, draft profile, pack supply, and reviewed admission.
  It also adds a redacted `extension_root` prerequisite to runtime status.

The committed Issue 370 and Issue 371 work will be integrated before Overview
implementation edits. The duplicate Issue 369 patch will be incorporated once
through Issue 370's equivalent predecessor commit.

## Landing-page structure

1. Lead with one product promise, a plain-language safety statement, a primary
   Trial CTA, and a secondary active-session CTA only when runtime data reports
   a running or recovery-required session.
2. Explain four design principles: local-first execution, explicit pre-run
   confirmation, a dedicated write boundary, and verification/repair/evidence
   with honest failure. No readiness state is inferred from decoration.
3. Present the workflow as Goal -> pre-run confirmation -> plan/implement ->
   verify/repair -> verified result or honest failure. Define Gate, profile,
   pack, and assurance in plain Japanese before or where each term first
   appears.
4. Keep Issue 120's prerequisite checks, sample goal, and term help, but make
   first use a persistent part of the page rather than a dismissible dashboard
   overlay. Link new instructions, live status, history, and result reading to
   the four Issue 370 routes.
5. Summarize the four Issue 371 layers and distinguish local registration,
   code/review boundaries, and evidence-backed promotion. Link to the owning
   Extensions page for exact identities and lifecycle actions.
6. Show only actionable live state: Trial availability, each real runtime
   prerequisite (including extension root), and the active/recovery session.
   Loading, unavailable, unconfigured, and action-required states remain
   explicit and are never rendered as green success.
7. Replace the capability map, band rows, run counts, and recent-run table with
   concise owner links to Measurements and Repository run records.

## Accessibility and responsive behavior

- Preserve the Shell's single page `h1` and main landmark; each landing section
  receives a unique `h2`, and ordered processes use semantic lists.
- Keep visible link names, `aria-live` runtime projection, `aria-current`
  navigation, focus-visible outlines, and 44-pixel-class touch targets.
- At mobile width, hero actions, workflow steps, status rows, and layer cards
  become a single readable column with no horizontal overflow. Existing global
  `prefers-reduced-motion` behavior remains authoritative; the landing page adds
  no essential animation.

## Documentation and verification

Synchronize the Overview story and routes in `README.md`, `README.ja.md`, GUI
help, and the English/Japanese user guides. Extend the existing source guard
and provider-free Overview smoke so both `/` and `/proxy/commandagent/` assert
the CTA targets, direct reload, real readiness/session states, semantic heading
order, focus, contrast/accessibility scan, reduced-motion compatibility, and
desktop/mobile fit.

Run GUI internal-path lint, TypeScript checking, production builds for both
base paths, focused Overview smoke, focused GUI Rust guards, formatting,
Clippy, and the full Rust test suite. Record only exact successful commands in
the machine-readable verification report.

## Preserved contracts

This change adds no runtime endpoint, metric, admission rule, release gate, or
write operation. Gate hashes and confirmation, assurance ceilings, verification
and acceptance semantics, event names and schemas, delegated CLI arguments,
and the live `.anvil/` namespace remain unchanged. A failed or unavailable
check is shown and reported honestly rather than converted to success.
