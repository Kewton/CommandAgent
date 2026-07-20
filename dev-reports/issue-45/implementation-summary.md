# Issue 45 implementation summary

## Implementation

- Fast-forwarded the worker branch to the passed Issue 43 predecessor so the
  accepted-command receipt and footer scrollback behavior remain the baseline.
- Added `tui::repl_output`, a leaf module for pre-execution input guidance,
  bounded nearest-command suggestions, interruption recovery cards, and a
  rendered command error transported back to the REPL.
- Moved unknown-slash and non-slash classification ahead of slash expansion,
  command receipts, presentation state, provider calls, and lifecycle guards.
  `/hepl` now receives a two-line `/help` suggestion, while Japanese and other
  plain text receives only `/ultra-plan-run` and `/plan-run` guidance.
- Removed the three-way failure presentation split from the slash handler.
  Started commands still finalize their existing event and summary contracts,
  but return one presentation: TASK FAILED for a real error or INTERRUPTED for
  user cancellation. The REPL sends that presentation through the Markdown
  renderer once and no longer writes a second `error:` line.
- The interruption card always gives the accepted command as a concrete rerun
  command and adds `/resume <recovery-yaml>` when a recovery plan exists.
- Reused the Markdown sanitizer for command receipts and input-error echoes,
  neutralizing terminal escape, C0/C1, and bidi controls while preserving CJK.

## Compatibility and documentation

No event names, keys, schemas, `.anvil/` paths, acceptance gates, or recovery
semantics changed. Unknown commands and plain text intentionally no longer emit
`tui_command_start`, `tui_command_stop`, or `loop_stop`, and do not generate a
command completion summary. Real failures and interruptions retain exactly one
honest stop event and the existing on-disk summary/recovery artifacts.

The English and Japanese slash-command guides now describe pre-execution input
errors. `/help` itself did not change. Added a corpus fixture recording the
no-event/no-summary input contract.

## Tests

- Unit coverage for typo distance, two-line guidance, Unicode/control/bidi
  sanitization, interruption rendering, receipt sanitization, and the REPL's
  one-call Markdown dispatch.
- Integration coverage proving invalid input does not call providers or create
  command lifecycle artifacts, a provider failure produces one final block,
  and an interrupted phase produces one distinct recovery card.
- Extended the real PTY screen-state matrix across footer on/off and
  color/NO_COLOR. It verifies the typo and Japanese plain-text exits plus one
  Markdown-rendered real failure with no Terminal summary or `error:` duplicate.
