# Issue 43 implementation summary

## Root cause and terminal ownership

The REPL had four independent presentation paths: rustyline input echo, stdout
Markdown, stderr spinner updates, and the footer background painter. The footer
kept DECSTBM active while command output scrolled only inside the reduced body
region. Accepted input was written to history and events, but no durable
presentation projection existed. Footer repaint or resize could therefore
erase the wrapped prompt, and lines leaving a partial scroll region were not
reliably retained by terminal scrollback.

## Implementation

- Added `tui::command_receipt`, a leaf projection rendered immediately after
  slash parsing and before preflight/provider work. It preserves the full
  sanitized input and Goal with CJK display-width wrapping, and shows command,
  effective profile/style/prompt layout with explicitness, requested port, and
  Run ID.
- Added a footer scrollback write boundary. It freezes repainting, uses the
  latest resize-aware geometry, temporarily resets DECSTBM, clears reserved
  footer rows, writes the receipt through the normal screen region, restores
  the fixed region, and forces the footer to repaint. Footer-off remains a
  plain stdout write; no-color receipts contain no generated SGR.
- Stored the last accepted execution command in presentation state. `/status`
  now reprints active Goal, Run ID, profile/style/layout, requested port, phase,
  step, and scope. Provider, command, repair, phase, and step transitions remain
  driven by the existing status bus; interrupt/force-finalize takes priority.
- Updated the UltraPlan card to announce total phases and the first phase, and
  removed the generic assurance sentence. Promoted empty-response recovery,
  planning quality retries, plan-generation retries, and timeout recovery from
  documented ignores to concise user-visible breadcrumbs.
- Added action-aware terminal summary routing. `--setup-interaction-probe`,
  `--model-probe`, `--plan-steps`, and `--ultra-plan` keep their own focused
  output without a generic coding gate card. Lifecycle events, honest failure
  details, remediation, and exit behavior are unchanged.

## Direct-action audit

| Action | Result |
| --- | --- |
| setup interaction probe / model probe | Own focused output; generic coding summary suppressed |
| step-plan / UltraPlan generation | Generated plan output retained; generic coding summary suppressed |
| plan-run / UltraPlan-run / recovery execution | Generic terminal task summary retained |
| doctor / runs / offline UX demo | Already routed before the generic finalizer; safe |
| completions / man generation | Already routed as generated artifacts; safe |

## Tests and documentation

- Added receipt unit tests for long CJK wrapping and control, escape, and bidi
  sanitization; added footer transition, active status, retry projection, and
  direct-action classification tests.
- Added a gated PTY regression over footer on/off and color/no-color. It uses a
  narrow PTY plus resize, leaves `COMMANDAGENT_NO_SPINNER` and
  `COMMANDAGENT_NO_MARKDOWN` unset, and exercises a fake-provider
  `/ultra-plan-run`, failure/remediation, `/status`, and clean exit.
- Extended the standard Ultra event fixture with promoted retry/recovery events
  so their presentation classification remains audited.
- Updated both READMEs and the demo notes to distinguish the offline scripted
  `--ux-demo` and hand-authored SVG from a real provider-backed recording.
  `docs/assets/repl-ultra-plan-run.rec` was captured from the final locally
  built binary in a 24x120 PTY against local Ollama with footer, color, spinner,
  and Markdown enabled. It contains a long provider wait, plan/phase/step
  progress, interrupt, recovery artifacts, and `/status` state.

No event name/schema, `.anvil/` namespace, verification gate, acceptance rule,
or exit-code contract was changed. The planner and minimal-loop growth
tripwires were not modified.
