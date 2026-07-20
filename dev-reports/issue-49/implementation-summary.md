# Issue 49 implementation summary

## Implementation

- Added shared Unicode terminal-width helpers in `src/util.rs`:
  `char_display_width`, `display_width`, `display_width_ansi`, and
  `fit_display_width`. The implementation uses `unicode-width`, preserves the
  footer's zero-column ANSI CSI behavior, stops at UTF-8 character boundaries,
  retains accepted zero-width combining characters, and closes an accepted SGR
  prefix before the truncation marker.
- Replaced the byte-counting `presentation::fit` path with a one-line wrapper
  over `fit_display_width` at every presentation call site. The content budget
  stays unchanged for ASCII: a 120-column truncated value retains the same 120
  ASCII characters before `...`; Japanese now retains 60 full-width characters
  instead of about 40 UTF-8-byte-limited characters.
- Routed footer truncation and padding, Markdown table measurement, and the
  Issue 43 accepted-command receipt through the shared width implementation.
  Footer fitting still reserves marker columns so rendered rows stay within the
  terminal width, and colored footer/table text continues to ignore ANSI CSI
  width.
- Added `unicode-width` as a direct dependency. It was already present in the
  lockfile through `rustyline`; the lockfile's `commandagent` dependency list is
  the only resolved-package change.

## Display truncation audit

| Surface | Decision |
| --- | --- |
| `input_queue::preview` | Migrated from 40 Unicode scalar values to a 40-column `fit_display_width` budget because the preview is rendered in queue confirmations and the footer. |
| spinner label | Left unchanged because it has no truncation or length budget; it only neutralizes controls, escape, and bidi characters before rendering. |
| `status_bus::sanitize_command_excerpt` | Migrated from the byte-oriented excerpt helper to a 120-column display budget because it feeds live footer/status presentation. |

## Compatibility

`excerpt_with_marker` remains byte-based and unchanged for record/protocol
callers such as provider parsing and run summaries. `eval_events` snippet and
summary functions were not changed, and their focused tests plus the full
conformance/golden suite pass. No event name/schema, `.anvil/` namespace,
verification gate, acceptance policy, or planner/minimal-loop tripwire changed.

Focused tests cover CJK and ASCII Plan Goal budgets, emoji, combining marks,
ANSI CSI, incomplete escape input, tiny column budgets, queue preview fitting,
command excerpt fitting, footer bounds, Markdown table padding, and the Issue 43
receipt wrapper.
