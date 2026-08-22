# Issues #172 and #167 design

## Scope

This combined change restores an existing GUI Trial session after a page load and
uses an honest terminal marker in the browser title. It keeps the API schema,
Gate/assurance decisions, and all session mutations unchanged. Closed Issue #162
is not reimplemented; its existing elapsed-time and identity projections are
preserved.

Production changes stay within the approved GUI ownership:

- `gui/hooks/use-trial-monitor.ts` owns URL-session restoration, query cleanup,
  and the terminal-title projection.
- `gui/components/trial-session-index.tsx` and the existing runtime-session link
  in `gui/components/shell.tsx` already produce the shared `?session=<id>` route;
  their navigation contract is retained so both links inherit automatic
  reconnect without acquiring mutation behavior.
- Row #159 files (`gui/lib/trial-monitor.ts` and `gui/lib/errors.ts`) are excluded.

## Behavior

1. On initial layout, when `session` is present, remove `sample` from the current
   URL before the compose hook's passive initialization can inject the sample
   goal. Keep the session id and unrelated query parameters.
2. Once Trial access is ready, automatically pass the URL session id through the
   existing reconnect operation. That operation calls `fetchSession`, whose
   request is GET-only. Guard each session/token pair against duplicate automatic
   attempts while still allowing a corrected token or an explicit retry.
3. On launch or successful reconnect, canonicalize the URL to contain the session
   id and no consumed sample. When the user starts a new run, remove the stale
   session id so compose mode cannot reconnect the old result.
4. When a terminal session is rendered, project the visible result heading into
   the document title with `✔` for Gate 3, `✗` for Gate 4, and the metadata
   separator `|`. Schedule this after the existing terminal effect so the
   approved monitor-owned correction wins without editing the terminal hook.

## Verification strategy

- Extend the focused browser feedback smoke to enter through `?sample=`, replace
  the sample goal, launch, reload, and observe automatic restoration without
  clicking the reconnect button. Record that `sample` is gone, the server-backed
  goal remains visible, and every API request after reload is GET.
- Make the synthetic terminal result Gate 4 and assert that its title starts with
  `✗`, contains no `✔`, and uses `| CommandAgent`.
- Update the Rust source-contract test for the new reconnect/title smoke evidence.
- Run GUI typecheck, lint, the focused feedback smoke, the focused Rust guard
  test, then the repository formatting, Clippy, and full test checks required for
  a shared GUI contract change.
