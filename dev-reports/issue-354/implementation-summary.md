# Issue #354 implementation summary

## Implemented

- Added an optional **Ollama thinking** selector to GUI Trial with the existing
  CLI values `true`, `false`, `low`, `medium`, and `high`.
- Disabled the selector when neither the executor nor planner uses Ollama and
  clear an existing selection when a provider edit removes the final Ollama
  role.
- Added optional `think` handling to the Trial request and frozen execution
  pins. The server returns HTTP 422 when a selected value has no Ollama role.
- Rendered selected thinking in the Gate 1 card and the frozen Gate 2/3/4 run
  identity, and delegated it as the required equals-form argument
  `--think=<value>`.
- Kept unspecified thinking absent from confirmation identity serialization,
  Gate 1 rendering, confirmation records, and delegated arguments. Requests
  that omit `think` and requests that send `think: null` produce identical
  proposals and card hashes.
- Integrated the required Issue #352 and #353 predecessor commits before the
  Issue #354 change.
- Documented the GUI selector, provider constraint, CLI mapping, and
  compatibility behavior in `docs/user/gui-trial.md`.

## Tests

- Added a legacy execution-pin JSON round-trip regression for the omitted field.
- Extended the existing delegated-session regression to compare omitted and
  explicit-null proposals, assert no `--think` argument, and assert the
  confirmation record has no `think` field.
- Added a GUI server integration regression for Gate 1 display, hash binding,
  HTTP 422 rejection without Ollama, and exact `--think=high` delegation.
- Extended the Trial source guard for selector disablement, automatic clearing,
  and frozen run-identity display.
- Extended the provider browser smoke for no-Ollama disabled/omitted behavior
  and end-to-end Ollama `high` propagation at root and proxied base paths.

## Compatibility

No event name/schema, `.anvil/` runtime namespace, or existing unspecified
confirmation identity changes. The optional execution pin uses serde defaulting
for historical records and is skipped during serialization when absent.
