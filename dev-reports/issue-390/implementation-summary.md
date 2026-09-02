# Issue 390 implementation summary

## Outcome

GUI Trial can now bind a session to an existing relative directory below
`--execution-root`. Omitting the field preserves the existing isolated
`sessions/<session-id>` behavior.

## Implementation

- Added an optional working-directory field to the Trial compose model and
  request payload. Editing it invalidates the current Gate 1 proposal, while an
  empty field is omitted from the API request for backward compatibility.
- Resolved explicit selections on the server and rejected absolute paths,
  traversal, runtime/session directories, missing paths, symlinks, and paths
  outside the execution root.
- Bound the selected canonical path into the Gate 1 identity, card, and hash.
  The same canonical path now supplies both the delegated process cwd and the
  CLI `--cwd` argument.
- Persisted a versioned per-session directory binding after confirmation. The
  record includes filesystem identity where available, allowing later requests
  to reject deletion, symlink substitution, and same-path replacement.
- Restored that binding for status, path inspection, additional directives,
  history reconnects, and server restarts. Sessions without a binding retain
  the legacy isolated-directory fallback.
- Kept selected directories outside managed workspace creation and rollback,
  so launch failures never remove an existing directory or its files.
- Updated GUI smoke coverage, Rust integration/guard tests, and user-facing
  documentation for the selection and recovery behavior.

## Compatibility and safety

No event name or event schema changed. Confirmation byte handling, credential
scrubbing, directive hash checks, and the existing Gate 1/Gate 3/Gate 4
boundaries remain in place. Absolute selected paths are stored only in the
private session binding and the authorized session-path response; public
identity projections use `<execution-root>/<relative-path>`.
