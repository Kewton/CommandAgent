# Issue 100 design: live Trial history and explicit run sources

## Context and predecessor

- The repository-report APIs and pages read the server's configured repository
  root, including `workspace/management/runs`.
- The authenticated Trial session index already reads the configured execution
  root's `.anvil/runs`, projects `starting`, `running`, `completed`, and failed
  terminal states, and preserves the GET-only `?session=<id>` reconnect flow.
- Required predecessor Issue #81 is committed as `77be99d6` directly on this
  worktree's current parent but is not merged into the branch. Integrate that
  commit before Issue #100 implementation so tab-lifetime token restoration and
  definitive token-rejection handling remain the authentication contract.

## Design

- Keep the Rust API and event schemas unchanged. Extend the Trial session index
  client into a small stale-while-revalidate view: retain the last successful
  index when a later refresh fails and show last-success/freshness separately
  from the current refresh error. Do not add an always-on index interval.
- Revalidate only from the issue's lifecycle signals: a valid/restored token,
  launch acceptance, an observed Gate 3/4 terminal transition, reconnect
  success, the already-polled runtime lease moving from `running` to `idle` or
  `recovery_required`, window focus, document visibility becoming visible, and
  manual refresh. Share the Shell's existing runtime-status result through
  React context; do not instantiate another `useRuntimeStatus` poller.
- Pass the page's currently observed session into the index. Merge it ahead of
  the last server projection so a successful launch immediately shows the
  returned session ID and `gate_2 / starting`, reconnect success immediately
  restores the target row, and the existing status monitor can update the row
  to its Gate 3/4 terminal state without waiting for a manual refresh.
- Treat a missing or incomplete Trial token as an explicit authentication-wait
  state. Do not reuse the authenticated empty-list copy for that state. Token
  changes clear the prior token's projection; ordinary refresh failures do not.
- Add stable anchors to Trial rows and link the terminal result to its matching
  history row. Preserve UUID query deep links and make no new POST path.
- Rename and annotate repository history as verification/operations reports,
  with `repository / workspace/management/runs` shown as its source. Label GUI
  Trial history with `execution root / .anvil/runs`. Use base-path helpers for
  navigational URLs and plain in-page fragments only for row anchors.

## Verification strategy

- Add a focused Playwright smoke that runs against builds for both `/` and
  `/proxy/commandagent/`. Mock only API responses while using the exported GUI,
  and verify initial index load, launch, immediate optimistic insertion,
  automatic terminal update, refresh-error data retention, lifecycle/focus/
  visibility revalidation without constant index polling, reconnect GET-only
  behavior, and the terminal-to-history link.
- In the same smoke, cover repository-only, Trial-only, both-present, and
  Trial-unauthenticated display states and assert the two source labels remain
  distinct.
- Extend the Rust GUI source guard for the revalidation/freshness/source/deep-
  link contracts. Run GUI syntax, typecheck, lint, both base-path builds, the
  focused browser smoke, GUI source/server tests, then repository formatting,
  Clippy, and full tests because shared guard and predecessor files are touched.

## Non-goals

- No changes to filesystem layout, `.anvil/`, historical run evidence, event
  names, session API payloads, lease enforcement, or Trial authentication.
- No cancellation, deletion, lease reset, or other mutation in the history UI.
