# Issue 373 implementation summary: Overview product landing page

## Outcome

The GUI Overview is now a product landing page instead of an operational
dashboard. It starts with the CommandAgent promise and a direct Trial action,
explains the safety model and Goal-to-result lifecycle in plain Japanese, keeps
first-use guidance visible, introduces the four extension layers, and projects
only live readiness and active-session facts returned by `gui_server`.

The detailed capability map, measurement bands, run counts, and recent-run
table were removed from Overview. Concise links now send users to Extensions,
Repository run records, and Measurements, where those details are owned.

## Predecessor integration

- Integrated the Issue 369 provider/model grouping as commit `0a9d2a0a`.
- Integrated the Issue 370 four-route Trial flow as commit `032ed840` and used
  `/try/`, `/try/status/`, `/try/history/`, and `/try/history/detail/` from the
  shared route helper throughout the landing guidance.
- Integrated the Issue 371 extension model as commit `8a2916fb` and presented
  capability vocabulary, draft profiles, pack supply, and admission as four
  ordered layers with their distinct trust boundaries.

## Implementation

- Rebuilt `gui/app/page.tsx` around a hero, Trial CTA, safety principles,
  plain-language terminology, the five-stage workflow, first-use guidance,
  extension boundaries, live state, and owner-page links.
- Made the first-use component persistent and aligned its prerequisite list,
  sample preset, and status/history/result links with the Issue 370 routes.
- Suppressed stale readiness and active-session actions when runtime status is
  unavailable. Loading, action-required, recovery, idle, and unavailable states
  remain visibly distinct.
- Added responsive single-column behavior, touch-sized actions, semantic
  headings and lists, labelled sections, visible keyboard focus, polite live
  status announcements, reduced-motion compatibility, and accessible color
  contrast.
- Expanded the browser smoke to verify both supported base paths, direct
  reloads, fixed routes, the absence of Overview dashboard fetches, synthetic
  active and failed runtime states, mobile fit, focus, heading order,
  reduced-motion behavior, and Axe WCAG A/AA results.
- Updated Rust source guards so the new landing-page and first-use contracts
  remain pinned without relaxing existing runtime or safety protections.

## Documentation

The landing-page story and fixed Trial route guidance are synchronized across
`README.md`, `README.ja.md`, the English and Japanese tutorials, the GUI
getting-started and operations guides, the compatibility index, and the
single-owner help-copy map.

## Preserved contracts

No runtime endpoint, write authority, admission rule, Gate hash, confirmation
boundary, assurance ceiling, acceptance rule, event name/schema, delegated CLI
argument, or `.anvil/` runtime path was changed. Overview now reads only
`runtime-status`; all operational detail remains on its existing owning page.
Failed or unavailable observations are never presented as ready or verified.

## Verification result

All required GUI lint, TypeScript, production build, browser smoke, focused
Rust guard, formatting, Clippy, GUI-server, and full Rust test checks passed.
The exact commands are recorded in `verification.md`.
