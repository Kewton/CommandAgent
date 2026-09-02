# Issue 390 design

## Scope

Add an optional GUI Trial working-directory selection while preserving the
current isolated `sessions/<session-id>` default. The request contract carries
one optional relative path. The server, not the browser, resolves that path
against the configured execution root.

## Server design

- Treat an omitted working directory as the existing managed session workspace.
- For an explicit selection, reject empty, absolute, parent-traversing, runtime
  metadata, and `sessions` paths. Require every selected path to already be a
  real directory whose canonical path remains below the canonical execution
  root and equals the lexical path (therefore no symlink component is accepted).
- Use the selected canonical directory as the Gate 1 workspace so the card and
  confirmation hash change with the selection. Re-resolve it when a confirmed
  launch is received.
- Persist a small versioned binding under the session state directory after
  confirmation. Record the relative path, canonical path, and filesystem
  identity so later dispatches reject deletion, symlink substitution, path
  substitution, and same-path directory replacement.
- Make `SessionPaths::existing` restore that binding. If no binding exists,
  retain the backward-compatible `sessions/<session-id>` fallback.
- Only create and roll back managed default workspaces. Explicitly selected
  existing directories are never created, emptied, or deleted by launch
  rollback.
- Resolve existing session paths for directive proposal and confirmation so
  Gate 3/Gate 4 continuation and post-restart reconnect use the persisted
  directory. Keep process `current_dir` and CLI `--cwd` sourced from the same
  canonical path.

## GUI design

Expose an optional relative working-directory field in the compose form. An
empty value is labelled as the isolated default. Editing it invalidates the
current proposal and confirmation like every other Gate 1 input. Send the
field unchanged to proposal and create endpoints; server errors remain the
authoritative validation feedback.

Project selected paths as `<execution-root>/<relative-path>` in Gate 1 and
session identity responses, without exposing the configured absolute root.

## Verification design

- Extend focused Rust integration coverage for default fallback, selected cwd
  and `--cwd`, stale hashes after selection changes, unsafe path rejection,
  deletion/replacement rejection, rollback preservation, directive reuse,
  and server-restart restoration.
- Extend GUI smoke/contract guards for the selection control, request payload,
  proposal invalidation, and history reconnect continuation surface.
- Run formatting, clippy, the focused GUI server/read-only tests, GUI typecheck
  and smoke coverage, then the full Rust suite because shared Trial session
  contracts are touched.
