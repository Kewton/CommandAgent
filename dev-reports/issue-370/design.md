# Issue #370 design: split GUI Trial responsibilities into fixed routes

## Current behavior

The `/try/` page owns the compose form, Gate 1 confirmation, Gate 2 monitor,
terminal result, evidence viewer, and full Trial session index. A query-only
`/try/?session=<id>` reconnects into the same page. This keeps the underlying
workflow honest, but mixes a new instruction, a live run, historical summaries,
and failure diagnosis in one URL and one navigation state.

The existing session-index refresh behavior is already useful: it waits for a
complete token, revalidates after focus/visibility and runtime lease changes,
shows freshness, and keeps the last successful list after a failed refresh.
The existing session detail API already projects the terminal verdict,
diagnostics, acceptance sheet, identity, events, and artifacts without changing
runtime state. The session-index API does not currently expose the confirmed
profile or intent required by the compact history rows.

Issue #369 is a required predecessor. Its committed role-specific provider/model
fieldsets and responsive behavior will be integrated before editing the shared
compose surface, without changing their state or request contracts.

## Design

- Add four statically exportable pages, using fixed routes and a `session` query
  parameter rather than dynamic route segments:
  - `/try/`: new instruction and Gate 1 confirmation;
  - `/try/status/?session=<id>`: read-only Gate 2 monitoring and evidence reads;
  - `/try/history/`: compact Trial session summaries;
  - `/try/history/detail/?session=<id>`: terminal verdict, diagnostics,
    acceptance, events, artifacts, and the existing confirmed follow-up flow.
- Give every page its own metadata title, visible heading/description, and a
  four-item Trial subnavigation with an explicit `aria-current="page"` state.
  The main navigation continues to identify all four as Trial surfaces.
- Reuse the existing workflow hooks and stage components, but render only the
  surface owned by the current route. A small client-side routing leaf observes
  a successfully launched or reconnected session and moves it to status or
  detail according to its gate. It also preserves `/try/?session=<id>` as a
  legacy state-aware redirect.
- Keep Trial credentials only in the existing base-path-scoped
  `sessionStorage` helper. Status, history, and detail expose a password input
  when authentication is enabled so a direct reload or separately opened tab
  can reconnect without putting a token in a URL, log, or `localStorage`.
- Move the session index out of the compose surface. Its rows retain only start
  and update times, ID, gate/status, confirmed profile, intent, and pack. Each
  active row links to status and each terminal row links to detail; inline
  failure diagnostics are removed.
- Add optional `profile` and `intent` fields to the read-only session-index JSON,
  projected from the immutable Gate 1 confirmation record. Historical or
  unreadable confirmation records remain listable with unavailable labels. No
  existing field, event name, or schema is removed or rewritten.
- Point the shared runtime badge at status. Terminal transitions land on detail,
  and the result page links back to its compact history row.

## Focused verification

- Extend the session-index browser smoke for both `/` and
  `/proxy/commandagent/` to cover direct reloads, distinct titles/headings/nav
  states, launch-to-status, active/terminal row routing, status-to-detail,
  legacy deep links, runtime-badge routing, mobile layout, authentication,
  freshness, revalidation, stale-list retention, and GET-only reconnects.
- Update the Rust GUI read-only/source guard for the four-route ownership,
  compact rows, token storage boundary, and route smoke assertions.
- Update the GUI server session-index integration test to pin additive
  profile/intent projection and historical fallback behavior.
- Run GUI syntax checks, internal-path lint, typecheck, build, focused browser
  smoke, focused Rust tests, and then repository-wide formatting, Clippy, and
  tests because a shared read-only API contract and shared GUI smoke are touched.

## Compatibility and non-goals

Gate 1 hashes and explicit confirmation, active leases, delegated CLI behavior,
honest terminal projection, verification/acceptance semantics, event names and
schemas, and the live `.anvil/` namespace remain unchanged. This change does not
add cancellation, lease release, SSE, or any new mutating status/history action.
