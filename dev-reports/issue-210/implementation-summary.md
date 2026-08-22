# Implementation summary: Issues #210, #222, and #209

## Changes

- Added an explicit `InterruptCleared` runtime status transition. Every
  `InterruptMonitor::reset` now clears both monitor atomics and the shared
  footer/`/status` interrupt projection, including force-finalize state.
- Added footer-only redaction for the Playwright module-resolution and global
  npm-root availability probes. Their live command status is retained with the
  safe label `checking interaction probe`; workspace paths, environment
  normalization, and JavaScript probe text are not rendered.
- Added `tui::repair_display`, a leaf helper shared by footer, status, and
  activity rendering. Positive repair maxima cap the displayed attempt at the
  maximum, while the existing single-number form remains for an unknown zero
  maximum.
- Updated dependency lifecycle activity rendering to recognize Python from the
  event profile or setup kind and render `Python dependency setup` instead of
  npm installation text. Non-Python lifecycle rendering remains unchanged.
- Extended the opt-in PTY interrupt scenario to run `/status` after Esc and
  assert that completed interrupt state does not survive into the next command.
  Added focused footer, status-bus, interrupt, and activity rendering tests.

## Compatibility and exclusions

- No serialized event name or schema changed.
- No repair execution limit, verification gate, acceptance rule, or corpus
  contract changed; only TUI projection is normalized, so no corpus fixture
  update was required.
- Provider HTTP cancellation remains excluded for row #241.
