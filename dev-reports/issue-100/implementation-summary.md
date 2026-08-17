# Issue 100 implementation summary

## Implemented behavior

- Integrated required predecessor commit `77be99d6` so the Trial token remains
  tab-scoped, base-path scoped, and removed only after a definitive server
  rejection.
- Changed the Trial history panel to retain its last successful index and show
  freshness independently from a current refresh error. Missing/incomplete
  tokens now show an explicit authentication-pending state rather than an empty
  authenticated list.
- Added lifecycle-driven revalidation for valid/restored tokens, accepted
  launches, Gate 3/4 transitions, successful reconnects, the shared runtime
  lease leaving `running`, focus, visibility restoration, and manual refresh.
  The panel has no independent periodic interval and consumes the Shell's
  existing runtime-status projection through React context instead of starting
  another poller.
- Merged the page's observed session into the file-backed index. A launch 202
  therefore adds its ID with `gate_2 / starting` immediately, while later
  monitor or index observations advance it without allowing a stale running
  observation to overwrite a terminal projection.
- Added a Terminal result link to the exact Trial history row. Existing
  `?session=<id>` deep links and GET-only reconnect requests remain unchanged.
- Renamed the repository view to **検証・運用レポート** and labeled its
  source as `repository / workspace/management/runs`. The GUI Trial history is
  separately labeled `execution root / .anvil/runs`, including distinct empty
  and unauthenticated copy.

## Tests and documentation

- Added `gui/scripts/session-index-smoke.mjs`. For both `/` and
  `/proxy/commandagent/`, it verifies initial index load, launch insertion,
  terminal update, stale-data retention on refresh failure, runtime/focus/
  visibility revalidation, absence of short-interval index polling, Terminal
  row navigation, and GET-only deep-link reconnect.
- The same smoke covers repository-only, Trial-only, both-present, and Trial-
  unauthenticated states with explicit source assertions.
- Updated the predecessor storage smoke for the now-earlier automatic session-
  index authentication check, and updated the established read-only smoke for
  the renamed repository-report view.
- Extended the Rust GUI source guard to prohibit an independent index interval
  or a second runtime-status hook and to pin lifecycle, freshness, source, and
  Playwright coverage.
- Updated the GUI user guide with the two history sources, event-driven refresh
  triggers, stale-while-error behavior, authentication-pending state, and the
  absence of an independent list poll.

## Unchanged contracts

- No Rust API payload, event schema, filesystem layout, `.anvil/` namespace,
  historical evidence, lease enforcement, or mutation surface changed.
