# Issues #211 and #212 implementation summary

## Implemented behavior

- REPL `/resume` now prepares the requested recovery before the D-3c Gate 1
  check. Missing default, named-run, and YAML targets explain that no resumable
  recovery exists and point to `/runs`; valid recoveries still receive the
  unchanged Gate 1 confirmation guidance.
- A non-TTY REPL invocation with `--resume <session-id>` now attempts to load
  the named saved minimal-loop session before returning the generic TTY error.
  Missing or unreadable sessions are identified as such. `--fresh-session`
  continues to override `--resume`.
- Interactive `/runs` passes the configured event stream to a session-aware
  renderer and marks its matching row `(current)`. The existing slash-command
  receipt remains intact. Direct `--runs` stays read-only and has no fabricated
  current session.
- Run timestamps are rendered in local `YYYY/MM/DD HH:MM` form. Completely
  absent phase counts render as `-`, and STOP detail is reduced to its leading
  category such as `model_stagnation`.
- Run rows use Unicode display widths and fixed column budgets; every header
  and data row is bounded to 100 terminal columns.

## Tests added

- Missing default and named recovery messages, plus `/runs` pointers.
- REPL missing-recovery preflight and valid-recovery fallthrough to Gate 1.
- Missing and existing saved-session validation for non-TTY resume.
- Local timestamp shape, `-` phase fallback, concise stop category, explicit
  current-session marker, Unicode-aware row width, and the 100-column bound.

## Compatibility

- No event name, event schema, recovery artifact, `.anvil/` path, provider
  behavior, acceptance gate, or verification requirement changed.
- Production changes are confined to `src/tui/repl.rs` and `src/runs.rs`.
- Tests remain inside the two owned source files; no corpus fixture changed
  because serialization and recovery/event contracts are unchanged.
