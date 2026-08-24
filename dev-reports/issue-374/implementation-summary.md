# Issue #374 implementation summary

## Outcome

GUI Trial status and result-detail pages now show the exact absolute working
directory used by the delegated CLI. The path is stable across launch,
running, terminal, and history-detail navigation, can be copied with a native
keyboard-operable button, and has polite screen-reader feedback.

The UI presents the CLI working directory separately from the run-record
directory and its `events.jsonl` and `summary.md` locations. When the working
directory no longer exists, it retains the historical path but explicitly
labels the directory deleted and says generated code or execution targets are
not present.

## Implementation

- Added GET-only `api/sessions/{id}/paths`. It projects the working-directory
  path/state and distinct run-record paths with `private, no-store` caching.
- Restricted the absolute-path endpoint to servers with Trial token
  authentication enabled and requests carrying the valid token. Existing
  create, status, session-index, public artifact/event, runtime-status, and
  static projections remain free of absolute execution-root paths.
- Reused `SessionPaths`, the same owner used to build delegated `current_dir`
  and `--cwd`. Existing directories must be real canonical directories;
  invalid IDs, traversal, symlinked roots/workspaces, and out-of-root symlink
  targets are rejected. A genuinely absent workspace is projected as
  `missing` without filesystem mutation.
- Added the reusable `TrialSessionPaths` component to the Issue #370 status
  and detail surfaces. It loads from the base-path-aware API helper, shares
  Trial token rejection handling, copies with `navigator.clipboard.writeText`,
  and exposes an atomic polite status announcement.
- Added responsive desktop/mobile styling and documentation for the workspace,
  record boundary, authentication requirement, copy behavior, and deleted
  state.

## Tests

- Extended the delegated CLI integration test to record the actual process
  working directory and prove it, `--cwd`, and the API projection are equal.
- Added authenticated endpoint tests for missing workspaces, invalid and
  traversal session IDs, symlinked workspaces/run roots, out-of-root targets,
  disabled authentication, wrong/missing tokens, and non-leaking failures.
- Extended the read-only/source guard for GET-only routing, token enforcement,
  canonical confinement, copy accessibility, path separation, and public
  projection isolation.
- Extended the root and `/proxy/commandagent/` browser lifecycle smoke across
  launch, running, terminal, history detail, keyboard copy/live feedback,
  desktop/mobile fit, record separation, and deleted-workspace display.

The first highly parallel full Rust run timed out in two pre-existing
three-second child-process fixtures; both passed immediately in exact focused
runs, and the unchanged full `cargo test` then passed. The first parallel
`gui_server` suite likewise timed out in one unrelated draft-pack fixture; its
exact rerun passed, followed by all 41 tests passing sequentially. No timeout,
acceptance, or verification threshold was weakened.

## Preserved contracts

Delegated arguments and event bytes, Gate 1 confirmation, active leases,
Origin validation, artifact/event read-only guards, public projection
redaction, honest-failure and acceptance semantics, event schemas, and the
live `.anvil/` namespace are unchanged.
