# Issue #374 design: expose the delegated session workspace safely

## Current behavior

Each GUI Trial launch creates two distinct directory trees below the configured
execution root:

- `sessions/<session-id>` is the delegated CLI working directory. Both the
  child process `current_dir` and its explicit `--cwd` argument use the
  canonical form of this directory.
- `.commandagent/runs/<session-id>` is runtime state and evidence storage. It
  owns `events.jsonl`, `summary.md`, confirmations, plans, and other run
  records. Existing legacy sessions below `.anvil/runs` remain readable.

The status, artifact, event, and session-index APIs are read-only and Trial
access-controlled, but their public projections intentionally redact the
absolute execution root. Consequently the four-page Trial GUI added by the
required Issue #370 predecessor cannot show or copy the working directory.

## Design

- Add a dedicated GET-only endpoint at
  `api/sessions/{id}/paths`. It returns one absolute working-directory path,
  its explicit availability (`available` or `missing`), and the distinct
  absolute run-record directory plus `events.jsonl` and `summary.md` locations.
- Make this endpoint stricter than the other Trial read APIs: it is available
  only when Trial token authentication is enabled and the request supplies the
  valid token. No absolute path is added to create, status, session-index,
  public artifact, event, runtime-status, or static projections.
- Derive the working directory from the same `SessionPaths` value used by CLI
  delegation. Canonicalize existing directories and require every resolved
  path to remain a real, non-symlink descendant of the configured execution
  root. A missing working directory is a successful historical state; a
  symlink, non-directory, invalid session ID, traversal attempt, or resolved
  path outside the execution root is rejected.
- Add one reusable workspace panel to the status and result-detail surfaces.
  It fetches as soon as a session ID and authenticated token are available, so
  the same path remains visible immediately after launch, while running, at a
  terminal result, and after opening result detail from history.
- Render the working directory separately from the run-record directory and
  the two record files. For an available directory, a native button copies the
  path with `navigator.clipboard.writeText`; its keyboard semantics are native,
  and a polite live region announces success or failure. For a missing
  directory, preserve the historical path but show an explicit deleted state
  and do not imply that generated artifacts still exist.

## Focused verification

- Extend the GUI server integration suite to prove the projected working path
  equals both delegated `current_dir` and `--cwd`, while paths remain absent
  from all existing projections.
- Add endpoint cases for missing workspaces, disabled or invalid
  authentication, invalid IDs and traversal forms, symlinked workspaces and
  run roots, and references that resolve outside the execution root.
- Extend the Trial route browser smoke for both `/` and
  `/proxy/commandagent/`, desktop and mobile, covering launch/running/terminal/
  history-detail consistency, copy-button keyboard activation and live
  notification, record-path separation, and deleted-workspace rendering.
- Run focused GUI syntax/type/lint/build and smoke checks, focused Rust tests,
  then repository formatting, Clippy, and test suites because the shared GUI
  server/API surface changes.

## Compatibility and non-goals

The endpoint is read-only and additive. It does not change delegated CLI
arguments, event schemas, existing public projections, Trial token/origin
validation, the read-only artifact guard, verification/acceptance semantics,
or the live `.anvil/` namespace. It does not recreate, delete, archive, or
otherwise mutate a missing session workspace.
