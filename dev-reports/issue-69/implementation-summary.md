# Issue 69 Implementation Summary

## Outcome

Gate 2 now gives a mobile-friendly indication of continuing activity without
changing the server contract. After a confirmed session is accepted, the Trial
page advances an `HH:MM:SS` browser-observed elapsed value once per second and
shows it next to the measured mean duration from Gate 1. The interval stops at
terminal so the final observed duration remains visible.

The progress summary selects the projected `running` phase when one exists,
otherwise the latest projected phase, and renders `Phase x / N` from the
existing `PhaseStatus.index` and `PhaseStatus.total` fields. Before any phase
event it displays a neutral placeholder rather than inventing a phase count.

Reaching Gate 3 or Gate 4 changes the browser tab title to include the terminal
gate and outcome. Leaving terminal restores the prior title. The three feedback
values use stable test IDs and collapse from three columns to a single mobile
column below 720 pixels.

## Tests and documentation

- Added a deterministic Playwright feedback probe to `gui/scripts/smoke.mjs`.
  It intercepts only the probe page's Trial API calls, returns a mocked
  `PolledSession`, waits 2.2 seconds, verifies the elapsed display changes,
  asserts `Phase 2 / 5` and `10.2 min mean`, advances the mock to Gate 4, and
  verifies the tab title changes.
- Added `--feedback-only` for fast focused execution. The same probe also runs
  inside the normal real-delegation smoke for `/` and
  `/proxy/commandagent/`.
- Extended the GUI source guard to pin the timer, payload-total phase display,
  terminal title behavior, and browser assertions.
- Updated the GUI operator guide to distinguish the browser-observed clock
  from a server timestamp and describe the completion title notification.

## Compatibility and scope

No Rust production source, server route, response field, event schema,
polling/retry interval, cache validator, confirmation gate, CLI delegation
path, corpus fixture, or `.anvil/` runtime namespace changed. The patch consumes
the existing `PhaseStatus.total` field and remains independent of the sibling
predecessor commits reviewed in the design note.
