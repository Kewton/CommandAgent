# Issue 45 design

## Observed mechanism

`tui::slash::handle_command` currently classifies an unknown command only after
rendering the accepted-command receipt, publishing active-command state,
emitting `tui_command_start`, and installing the completion guard. The same
path is used for non-slash text, so both input mistakes produce stop events and
a run summary. On execution errors, the handler prints a TASK FAILED block,
renders a separate Terminal summary, and returns the error; the REPL then
prints that returned error again with an `error:` prefix. Interruptions are
classified honestly in events, but still use those generic failure renderers.

Issue 43 is the required baseline. Its command receipt and footer scrollback
boundary remain the presentation path for accepted executable commands; input
errors must exit before that receipt so a typo remains a short response.

## Change

- Add a leaf REPL-output module that renders sanitized pre-execution guidance,
  typo suggestions, one interruption card with concrete resume/rerun commands,
  and a display error containing one final failure presentation.
- Classify the first token before slash parsing or lifecycle setup. Non-slash
  text returns only guidance to `/ultra-plan-run` and `/plan-run`; an unknown
  slash command returns a bounded nearest-command suggestion. Neither path
  emits command events, creates a summary, calls a provider, or publishes an
  accepted command.
- Keep real execution failure semantics unchanged: started commands still emit
  exactly one compatible `tui_command_stop`, write the run summary, and return
  an error. Remove the handler's direct stderr/stdout failure writes and make
  the returned error carry either the TASK FAILED block or the distinct
  INTERRUPTED card. The REPL sends both success and error presentations through
  its Markdown renderer exactly once and does not add an `error:` line.
- Reuse the repository Markdown sanitizer for echoed input after flattening
  line breaks, covering controls, C1 bytes, bidi controls, and terminal escape.

## Compatibility and verification

No event name, key, or schema changes. The only lifecycle change is that
pre-execution input errors intentionally emit no `tui_command_start`,
`tui_command_stop`, `loop_stop`, or summary. Existing execution failures and
interruptions retain honest stop status and recovery artifacts. `/help` text is
unchanged, so the doc-drift snapshot does not need a semantic update; the
English and Japanese slash-command guides will be updated because they
currently describe unknown input as an execution failure.

Focused tests will cover `/hepl`, Japanese free text, provider failure,
interruption recovery guidance, single-render dispatch, and control/bidi/escape
sanitization. A corpus fixture will record the no-event input-error contract,
and the existing PTY matrix will exercise invalid inputs under footer on/off
and color/NO_COLOR. Shared TUI and event behavior requires formatting, Clippy,
doc drift, corpus regression, and the full Rust test suite.
