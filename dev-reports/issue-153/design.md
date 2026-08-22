# Issue #153 design: residual GUI result projection

## Scope

This worktree combines open Issue #153 with only the explicitly assigned
residual GUI presentation from closed Issues #148 and #149. It does not change
the completed CLI presentation, sample goal, duration projection, reconnect
behavior, compose files, Issue lifecycle state, event schemas, or Gate 3/4
decision logic.

Production ownership is limited to the GUI session projection and terminal
presentation:

- `src/bin/gui_server/sessions.rs` adds nullable `stop_reason`, `next_action`,
  and `assurance_reason` response fields sourced from the current recorded
  terminal events.
- `gui/components/trial-terminal.tsx` renders the recorded result, reason, and
  typed next action as three Japanese rows before the frozen run identity. It
  keeps unrecognized diagnostics visible, folds the raw acceptance sheet, and
  orders the screen as result, optional additional request, then read-only
  session files.
- `gui/hooks/use-trial-terminal.ts` owns the terminal title and completion
  notification. Its duration uses the same server-owned session start epoch as
  the Gate 2 elapsed clock.
- `docs/user/gui-help-map.md` records the stable result guidance fixed by the
  browser smoke.

No historical evidence or `.anvil/` runtime namespace is rewritten.

## Projection and honest-failure boundary

The server reads only the current event segment after the latest confirmed
directive continuation. It prefers the latest `run_stop`, then
`tui_command_stop`, and exposes recorded strings without upgrading, translating,
or synthesizing a Gate. `next_action` may fall back to the current
`ultra_final_acceptance` or `plan_final_contract` event, matching the existing
terminal-report projection. Missing values remain JSON `null`.

The GUI translates known closed identifiers for readability and retains an
unknown identifier or diagnostic after a Japanese label. A missing field is
reported as unrecorded. Gate 3/4 headings, markers, and explanations continue
to consume the server-owned `gate`; no result field can promote Gate 4.

## Notification and title behavior

On terminal display, the title is `✔` for Gate 3 and `✗` for Gate 4 and uses
the same rendered Japanese heading. Completion notification is emitted once
per terminal event count only when all of these are true:

1. the previous screen was Gate 2, so a reload of an already terminal session
   is not treated as a fresh completion;
2. `document.hidden` is true;
3. the Notification API exists and permission is already `granted`.

The GUI never prompts for notification permission. Unsupported, default, or
denied permission is a no-op. Notification construction failure is also a
no-op and cannot affect the result screen.

## Focused tests and verification

- Extend GUI-server tests to pin the additive keys, recorded values, fallback
  behavior, and current-continuation boundary.
- Extend the GUI browser feedback smoke with a synthetic Gate 4 projection,
  Japanese summary, folded details, section ordering, truthful title, and a
  hidden-page granted notification containing Gate and duration.
- Run formatting, TypeScript/lint/build checks, focused GUI smoke, GUI-feature
  Rust tests, Clippy with GUI enabled, and the full Rust suite because a shared
  API response is extended.
