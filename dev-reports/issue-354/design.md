# Issue #354 design: expose Ollama thinking in GUI Trial

## Context and predecessors

The worker branch starts at `origin/develop`. Issue #352 commit `8e4efda2`
isolates delegated Trial workspaces per session, and Issue #353 commit `6b81cde5`
separates executor and planner provider controls. Both predecessor verification
reports are passed. Their commits will be integrated in that order before this
Issue's implementation so delegation follows #352 and the compose UI follows
#353.

## Design

- Add an optional `think` value to the GUI session request and the frozen
  execution pins, using the CLI's existing `true`, `false`, `low`, `medium`,
  and `high` value domain.
- Keep the field absent from serialized confirmation identities when it is not
  selected. Render no additional Gate 1 or run-identity row in that case. This
  preserves the existing identity bytes, card hash, card text, and confirmation
  record shape for requests that omit `think` or send `null`.
- When selected, render the value in the Gate 1 card and frozen run identity,
  and delegate it as one equals-form argument: `--think=<value>`.
- Reject selected thinking with HTTP 422 unless the executor or planner provider
  is Ollama. In the compose form, disable the selector when neither role uses
  Ollama and clear an existing selection if provider edits remove the final
  Ollama role.
- Extend focused server and browser smoke coverage for selected delegation,
  Ollama-free rejection/UI disablement, and unspecified compatibility. Update
  the Trial user guide with the selector, validation, and exact CLI mapping.

## Verification

Run focused confirmation/server/static tests first, then the required Rust
format, Clippy, and full test suite plus GUI lint, typecheck, and provider smoke.
