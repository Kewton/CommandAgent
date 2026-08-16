# Issue 76 design: Japanese GUI language and navigation policy

## Decision

- The GUI language is fixed to Japanese. Do not add an i18n framework or a
  language switcher. User-facing navigation, headings, labels, guidance, empty
  states, and errors owned by the GUI are Japanese; opaque profile/provider
  values, filesystem paths, API resource names, event names, and persisted
  status identifiers remain unchanged.
- Every page intro contains the page name and one short explanatory sentence.
  Remove the numbered eyebrow/catch-copy composition. At the mobile breakpoint,
  hide the explanatory sentence so the intro is one line.
- Use Next metadata with a root title template and route layouts so Overview,
  Trial, Run detail, Assets, and Measurements produce distinct tab titles.
- Keep Assets routable, but remove it from the primary desktop/mobile
  navigation. Add a clear Assets link to Overview instead.

## Runtime status

The current green `CLI delegated` pill is decorative and must not imply live
state. Add a read-only `GET /api/runtime-status` projection backed by the
existing Trial workspace configuration and lease. It reports Trial availability
and an optional session with the existing lease state (`running` or
`recovery_required`). It does not accept credentials, dispatch work, mutate the
lease, inspect event contents, or change `.anvil/` state.

The shared shell polls this projection at a modest fixed interval and renders
neutral/loading/error, available/idle, running, and recovery-required states
with truthful color semantics. API/event identifiers and all existing Trial
write/confirmation boundaries stay intact.

## Predecessor integration

Issue 63 (`4313d7ef`) and Issue 77 (`e99547fa`) are complete but are not
ancestors of this worktree. Their committed Trial monitoring recovery, mobile
scroll/inset, run-ledger accessibility, and smoke assertions overlap the files
changed here. Preserve and integrate those contracts before applying the Issue
76 copy/layout changes; update their assertions only where Japanese visible
copy or the compact intro intentionally changes expectations.

## Tests and verification

- Add focused Rust coverage for runtime-status serialization and keep the GUI
  read-only/delegation guard authoritative.
- Extend the two-base-path browser smoke to assert Japanese headings, distinct
  document titles, the four-item primary navigation, the Overview Assets link,
  and truthful runtime-status transitions while retaining predecessor smoke
  checks.
- Run GUI lint/typecheck/build, focused GUI Rust tests, and `smoke.mjs` first.
  Because shared Rust server and smoke contracts change, also run repository
  formatting, Clippy, and the full Rust test suite.
